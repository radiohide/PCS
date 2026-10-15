<#
.SYNOPSIS
Runs the verification set for one workstream, in its own work tree.

.DESCRIPTION
Each workstream verifies its own area, so two workstreams can run at the same time without sharing
state. Every result is printed with the commit, branch, catalog version and rule version it was
produced against, so a result can always be traced back to the contract that produced it.

The verification sets follow model/ownership.json: a workstream runs the checks for the files it
owns, not the whole suite. The full suite is the integration gate, not the per-workstream loop.

.PARAMETER Workstream
base-data | type-adapters | query-ui | migration

.PARAMETER SkipCargo
Runs only the script checks. Useful when a full compile is not wanted.
#>
param(
    [Parameter(Mandatory=$true)][ValidateSet('base-data','type-adapters','query-ui','migration')][string]$Workstream,
    [switch]$SkipCargo
)
$ErrorActionPreference='Stop'
$project=$PSScriptRoot | Split-Path -Parent

function GitOutput([string[]]$arguments){
    $git=Get-Command git -ErrorAction SilentlyContinue
    if(-not $git){
        foreach($candidate in @(
            (Join-Path $env:LOCALAPPDATA 'Programs/PortableGit/cmd/git.exe'),
            (Join-Path $env:ProgramFiles 'Git/cmd/git.exe'))){
            if(Test-Path -LiteralPath $candidate){ $git=$candidate; break }
        }
    }
    if(-not $git){ return '(git not found)' }
    $executable=if($git -is [string]){$git}else{$git.Source}
    return (& $executable -C $project @arguments) -join ' '
}

# Read as UTF-8 explicitly: PowerShell 5.1's Get-Content defaults to the system code page and
# mangles the Chinese labels in these files badly enough to break JSON parsing.
function Read-Json([string]$path){
    return [System.IO.File]::ReadAllText($path, [System.Text.Encoding]::UTF8) | ConvertFrom-Json
}
# Hash the content, not the checkout. With core.autocrlf the same commit can be checked out with
# CRLF in one work tree and LF in another, and a rule version that changes with that would make two
# work streams report the same version for different rules — or different versions for the same one.
function Get-ContentDigest([string]$path){
    $text=[System.IO.File]::ReadAllText($path, [System.Text.Encoding]::UTF8).Replace("`r`n","`n")
    $sha=[System.Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($text)))).Replace('-','').ToLowerInvariant() }
    finally { $sha.Dispose() }
}
$catalog=Read-Json (Join-Path $project 'model/catalog.json')
$ownership=Read-Json (Join-Path $project 'model/ownership.json')
# PowerShell variable names are case-insensitive, so this must not be called $workstream: that would
# be the parameter itself, and assigning to it re-runs the validation.
$workstreamInfo=$ownership.workstreams | Where-Object { $_.id -eq $Workstream }

Write-Output "workstream      $Workstream ($($workstreamInfo.name))"
Write-Output "work tree       $project"
Write-Output "branch          $(GitOutput @('rev-parse','--abbrev-ref','HEAD'))"
Write-Output "commit          $(GitOutput @('rev-parse','--short','HEAD'))"
Write-Output "catalog version $($catalog.version)"
Write-Output "rule version    $(Get-ContentDigest (Join-Path $project 'model/catalog.json'))"
Write-Output ""

$sets=@{
    'base-data'=@(
        @{kind='script';path='scripts/check-model-catalog.cjs'},
        @{kind='script';path='scripts/check-ownership.cjs'},
        @{kind='cargo';args=@('test','--offline','--locked','-p','pcscope-contracts','-p','pcscope-storage')}
    )
    'type-adapters'=@(
        @{kind='script';path='scripts/check-source-inventory.cjs'},
        @{kind='script';path='scripts/check-contract-cases.cjs'},
        @{kind='cargo';args=@('test','--offline','--locked','-p','pcscope-contracts')}
    )
    'query-ui'=@(
        @{kind='script';path='scripts/check-query-contract.cjs'},
        @{kind='script';path='scripts/check-collection-view.cjs'},
        @{kind='script';path='scripts/check-readable-details.cjs'},
        @{kind='script';path='scripts/check-snapshot-diff.cjs'},
        @{kind='script';path='scripts/check-front-refresh.cjs'},
        @{kind='script';path='scripts/check-monitoring-ui.cjs'}
    )
    'migration'=@(
        @{kind='script';path='scripts/check-model-catalog.cjs'},
        @{kind='cargo';args=@('test','--offline','--locked','-p','pcscope-cli')}
    )
}

$failures=0
foreach($step in $sets[$Workstream]){
    $label=if($step.kind -eq 'script'){$step.path}else{$step.args -join ' '}
    $clock=[Diagnostics.Stopwatch]::StartNew()
    # PowerShell 5.1 wraps a native command's stderr in an ErrorRecord, which under
    # $ErrorActionPreference='Stop' aborts the whole script the moment cargo prints progress. The
    # exit code is the thing that decides success here, so the preference is relaxed for the call.
    $previousPreference=$ErrorActionPreference
    $ErrorActionPreference='Continue'
    try{
        if($step.kind -eq 'script'){
            $output=& node (Join-Path $project $step.path) 2>&1
            $code=$LASTEXITCODE
        }else{
            if($SkipCargo){ $ErrorActionPreference=$previousPreference; Write-Output ("  skipped (SkipCargo)  {0}" -f $label); continue }
            # build.ps1 resolves the external toolchain; a work tree never assumes one on PATH.
            $output=& (Join-Path $project 'build.ps1') -CargoArgs $step.args 2>&1
            $code=$LASTEXITCODE
        }
    }finally{
        $ErrorActionPreference=$previousPreference
    }
    $clock.Stop()
    $status=if($code -eq 0){'ok  '}else{'FAIL'}
    if($code -ne 0){ $failures++ }
    Write-Output ("  {0} {1,6:N1}s  {2}" -f $status,$clock.Elapsed.TotalSeconds,$label)
    if($code -ne 0){
        $output | Select-Object -Last 12 | ForEach-Object { "        $_" }
    }
}

Write-Output ""
if($failures -gt 0){
    Write-Output "verify $Workstream FAILED ($failures step(s)) at commit $(GitOutput @('rev-parse','--short','HEAD'))"
    exit 1
}
Write-Output "verify $Workstream passed against catalog $($catalog.version) at commit $(GitOutput @('rev-parse','--short','HEAD'))"
