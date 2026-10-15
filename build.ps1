<#
.SYNOPSIS
Builds or tests the PCScope Rust workspace.

.DESCRIPTION
The Rust toolchain is an external dependency: it is deliberately not committed, so this script
resolves it instead of assuming it sits in the repository. Search order for the tools root, which
must contain cargo\bin\cargo.exe:

  1. -ToolsRoot <path>
  2. $env:PCSCOPE_TOOLS
  3. <repository>\.tools
  4. <ancestor>\.tools and <ancestor>\*\.tools, for up to four ancestor levels

When nothing is found the script fails with the paths it searched, rather than letting cargo report
a confusing error or silently pointing CARGO_HOME at a directory that does not exist.

.PARAMETER Check
Reports the resolved toolchain paths and versions without building. Use this to verify a fresh
terminal can locate the external dependencies before any build is attempted.

.PARAMETER Project
Directory of the Cargo workspace to build. Defaults to this script's directory, the main workspace.
The desktop application is a separate workspace and is built with
-Project apps/desktop/src-tauri; running it through this script keeps one toolchain resolution and
one linker selection for both workspaces instead of a second, hand-assembled environment.
#>
param(
    [string[]]$CargoArgs = @('test','--workspace'),
    [string]$ToolsRoot,
    [string]$Project,
    [switch]$Check
)
$ErrorActionPreference='Stop'
$repo=$PSScriptRoot

function Test-ToolsRoot([string]$path){
    if([string]::IsNullOrWhiteSpace($path)){return $false}
    return (Test-Path -LiteralPath (Join-Path $path 'cargo\bin\cargo.exe'))
}
function Find-ToolsRoot(){
    $searched=New-Object System.Collections.Generic.List[string]
    # An explicit request is authoritative: falling through to a different toolchain would build
    # with something the caller did not ask for, which is worse than failing.
    if($ToolsRoot){
        if(Test-ToolsRoot $ToolsRoot){return @{path=$ToolsRoot;searched=$searched}}
        return @{path=$null;searched=$searched;rejected=$ToolsRoot;rejectedBy='-ToolsRoot'}
    }
    if($env:PCSCOPE_TOOLS){
        if(Test-ToolsRoot $env:PCSCOPE_TOOLS){return @{path=$env:PCSCOPE_TOOLS;searched=$searched}}
        return @{path=$null;searched=$searched;rejected="$env:PCSCOPE_TOOLS";rejectedBy='PCSCOPE_TOOLS'}
    }
    $inRepo=Join-Path $repo '.tools'
    $searched.Add("repository $inRepo")
    if(Test-ToolsRoot $inRepo){return @{path=$inRepo;searched=$searched}}
    $ancestor=$repo
    for($level=1;$level -le 4;$level++){
        $parent=Split-Path $ancestor -Parent
        if(-not $parent -or $parent -eq $ancestor){break}
        $ancestor=$parent
        $near=Join-Path $ancestor '.tools'
        $searched.Add("ancestor $near")
        if(Test-ToolsRoot $near){return @{path=$near;searched=$searched}}
        $siblings=@(Get-ChildItem -LiteralPath $ancestor -Directory -ErrorAction SilentlyContinue)
        foreach($dir in $siblings){
            $candidate=Join-Path $dir.FullName '.tools'
            if($candidate -eq $inRepo){continue}
            if(Test-ToolsRoot $candidate){
                $searched.Add("sibling $candidate")
                return @{path=$candidate;searched=$searched}
            }
        }
        $searched.Add("  no .tools under $ancestor (checked $($siblings.Count) sibling directories)")
    }
    return @{path=$null;searched=$searched}
}
function Fail-NoToolchain($found){
    Write-Host 'PCScope build: Rust toolchain not found.' -ForegroundColor Red
    if($found.rejectedBy){
        Write-Host "$($found.rejectedBy) was set to $($found.rejected), which does not contain cargo\bin\cargo.exe."
        Write-Host 'An explicit toolchain request is not silently replaced by another one.'
    } else {
        Write-Host 'It is an external dependency and is not committed to this repository.'
        Write-Host 'Pass -ToolsRoot <path> or set $env:PCSCOPE_TOOLS to a directory containing cargo\bin\cargo.exe.'
        Write-Host 'Searched:'
        foreach($item in $found.searched){ Write-Host "  $item" }
    }
    exit 2
}
function Add-PathOnce([string]$dir){
    if([string]::IsNullOrWhiteSpace($dir)){return $false}
    if(-not (Test-Path -LiteralPath $dir)){return $false}
    if(($env:PATH -split ';') -contains $dir){return $true}
    $env:PATH="$dir;$env:PATH"
    return $true
}
function Find-BinDirectory([string]$tools,[string[]]$patterns){
    foreach($pattern in $patterns){
        $candidate=Join-Path $tools $pattern
        # A literal path has to be tested as a directory itself: Get-ChildItem -Path <dir>
        # -Directory lists the directory's children, so a pattern without a wildcard would
        # never match the bin directory it names (that is how the 'mingw64\bin' entry stayed
        # dead code while llvm-mingw was picked instead).
        if($pattern -notmatch '[*?\[]'){
            if(Test-Path -LiteralPath $candidate -PathType Container){ return (Resolve-Path -LiteralPath $candidate).Path }
            continue
        }
        # -Path, not -LiteralPath: the pattern contains a wildcard for the MinGW flavour directory.
        $match=Get-ChildItem -Path $candidate -Directory -ErrorAction SilentlyContinue | Select-Object -First 1
        if($match){return $match.FullName}
    }
    return $null
}
function Report([string]$label,[string]$value){ "  {0,-22} {1}" -f $label,$value }

$found=Find-ToolsRoot
if(-not $found.path){ Fail-NoToolchain $found }
$cargoExe=Join-Path $found.path 'cargo\bin\cargo.exe'
$env:CARGO_HOME=Join-Path $found.path 'cargo'
$rustup=Join-Path $found.path 'rustup'
if(Test-Path -LiteralPath $rustup){ $env:RUSTUP_HOME=$rustup } else { Write-Warning "rustup home not found at $rustup; relying on the existing RUSTUP_HOME" }
Add-PathOnce (Join-Path $found.path 'cargo\bin') | Out-Null
# Prefer a GNU MinGW distribution. Rust's x86_64-pc-windows-gnu std is linked against
# libgcc/libgcc_eh, which LLVM-MinGW does not ship (it uses compiler-rt): with an
# llvm-mingw bin directory first on PATH, rustc resolves the linker name
# `x86_64-w64-mingw32-gcc` to the clang wrapper and every link fails with
# "lld: error: unable to find library -lgcc_eh". A GNU mingw64 directory is therefore
# searched before llvm-mingw, and the linker is named explicitly below.
$compilerBin=Find-BinDirectory $found.path @('mingw\*\bin','mingw64\bin','llvm-mingw\*\bin')
if($compilerBin){ Add-PathOnce $compilerBin | Out-Null }
# Name the linker instead of letting PATH order decide. A caller that already set the
# variable asked for a specific linker, so it is left alone.
$gnuLinker=$null
if($compilerBin){
    foreach($candidate in 'x86_64-w64-mingw32-gcc.exe','gcc.exe'){
        $candidatePath=Join-Path $compilerBin $candidate
        if(Test-Path -LiteralPath $candidatePath){ $gnuLinker=$candidatePath; break }
    }
    if($gnuLinker -and -not $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER){
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=$gnuLinker
    }
}

$buildDir=if($Project){(Resolve-Path -LiteralPath $Project).Path}else{$repo}
if(-not (Test-Path -LiteralPath (Join-Path $buildDir 'Cargo.toml'))){ throw "No Cargo.toml in $buildDir; -Project must name a Cargo workspace directory." }

if($Check){
    Write-Host 'PCScope build preflight'
    Report 'project' $buildDir
    Report 'tools root' $found.path
    Report 'cargo home' $env:CARGO_HOME
    Report 'rustup home' $(if($env:RUSTUP_HOME){$env:RUSTUP_HOME}else{'(not set)'})
    Report 'cargo' (& $cargoExe --version)
    Report 'rustc' (& (Join-Path $found.path 'cargo\bin\rustc.exe') --version)
    Report 'host target' (& (Join-Path $found.path 'cargo\bin\rustc.exe') -vV | Where-Object { $_ -match '^host:' })
    Report 'linker bin' $(if($compilerBin){$compilerBin}else{'NOT FOUND - the GNU target needs a MinGW or LLVM-MinGW bin directory'})
    Report 'target linker' $(if($env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER){$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER}else{'not set - rustc resolves x86_64-w64-mingw32-gcc from PATH'})
    $libgcc=if($compilerBin){ Get-ChildItem -Path (Join-Path (Split-Path $compilerBin -Parent) 'lib\gcc\*\*\libgcc.a') -ErrorAction SilentlyContinue | Select-Object -First 1 }
    Report 'libgcc.a' $(if($libgcc){$libgcc.FullName}elseif($compilerBin){'NOT FOUND - the GNU target needs libgcc; LLVM-MinGW ships compiler-rt instead'}else{'NOT FOUND'})
    $csc=Join-Path $env:SystemRoot 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
    Report 'csc (native reader)' $(if(Test-Path -LiteralPath $csc){"$csc"}else{'NOT FOUND'})
    Report 'node' $(if(Get-Command node -ErrorAction SilentlyContinue){(node --version)}else{'NOT FOUND'})
    Report 'powershell' $PSVersionTable.PSVersion.ToString()
    $git=Get-Command git -ErrorAction SilentlyContinue
    Report 'git' $(if($git){"$($git.Source)"}else{'not on PATH; a portable install may still exist'})
    exit 0
}

Push-Location $buildDir
try {
    # Cargo writes progress ("Compiling …") to stderr. Under $ErrorActionPreference='Stop' PowerShell
    # 5.1 turns each such line into a terminating error, so a caller that redirects streams — a job,
    # a log capture — would abort the build on its first line of progress. The exit code decides
    # success here, so the preference is relaxed for the call and restored afterwards.
    $previousPreference=$ErrorActionPreference
    $ErrorActionPreference='Continue'
    try { & $cargoExe @CargoArgs } finally { $ErrorActionPreference=$previousPreference }
    if ($LASTEXITCODE -ne 0) { throw "Cargo failed: $LASTEXITCODE" }
} finally { Pop-Location }
