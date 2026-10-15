param([string]$ToolsRoot)
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
Push-Location $project
try {
    & ./build.ps1 -ToolsRoot $ToolsRoot -CargoArgs @('build','--workspace','--bins','--examples')
    & ./plugins/platform-native/build.ps1
    $fixture=Join-Path $project 'target/sihost-fixtures'
    New-Item -ItemType Directory -Force -Path $fixture | Out-Null
    $source=Join-Path $fixture 'host.cs'
    [IO.File]::WriteAllText($source,'using System; using System.Runtime.InteropServices; class Host { [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr LoadLibraryW(string path); static int Main(string[] args) { if(args.Length!=1||LoadLibraryW(args[0])==IntPtr.Zero)return 1;Console.WriteLine("ready");Console.ReadLine();return 0; } }')
    $csc=Join-Path $env:SystemRoot 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
    & $csc /nologo /platform:x64 /target:exe ('/out:'+ (Join-Path $fixture 'pcscope-module-fixture.exe')) $source
    if($LASTEXITCODE -ne 0){throw 'Fixture host compile failed'}
    $fixtureTools=if($ToolsRoot){$ToolsRoot}else{$env:PCSCOPE_TOOLS}
    if(-not $fixtureTools){throw 'Pass -ToolsRoot or configure PCSCOPE_TOOLS for the native fixture compiler'}
    $compiler=Join-Path $fixtureTools 'mingw64/bin/gcc.exe'
    if(-not (Test-Path -LiteralPath $compiler)){throw 'Resolved native fixture compiler missing'}
    foreach($version in 1,2){
        $code=Join-Path $fixture ('module'+$version+'.c')
        [IO.File]::WriteAllText($code,('__declspec(dllexport) int pcs_fixture_version(void) { return '+$version+'; }'))
        & $compiler -shared -o (Join-Path $fixture ('module'+$version+'.dll')) $code
        if($LASTEXITCODE -ne 0){throw 'Fixture DLL compile failed'}
    }
    & ./target/debug/examples/sihost-file-monitor.exe $fixture
    if($LASTEXITCODE -ne 0){throw 'SIHost file monitor acceptance failed'}
} finally {Pop-Location}
