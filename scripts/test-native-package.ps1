param([Parameter(Mandatory=$true)][string]$Bundle,[string]$ToolsRoot,[switch]$AbsenceReplay)
$ErrorActionPreference='Stop'
$taskProject=Split-Path $PSScriptRoot -Parent
$taskBundle=(Resolve-Path -LiteralPath $Bundle).Path
$taskArguments=@{CargoArgs=@('build','-p','pcscope-service','--example','native-package-smoke')}
if($ToolsRoot){$taskArguments.ToolsRoot=$ToolsRoot}
& (Join-Path $taskProject 'build.ps1') @taskArguments
# Compiler executables/outputs may carry low integrity. Do not change the toolchain or workspace.
# Launch only a detached acceptance copy from an ordinary shell; the executable validates its token.
$taskRunnerDirectory=Join-Path $taskProject ('target/native-package-smoke-runner/'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $taskRunnerDirectory|Out-Null
$taskRunner=Join-Path $taskRunnerDirectory 'native-package-smoke.exe'
Copy-Item -LiteralPath (Join-Path $taskProject 'target/debug/examples/native-package-smoke.exe') -Destination $taskRunner
& icacls.exe $taskRunner /setintegritylevel M | Out-Null
if($LASTEXITCODE -ne 0){throw 'Acceptance executable integrity setup failed'}
Push-Location $taskProject
try{
    $taskRunnerArgs=@($taskBundle)
    if($AbsenceReplay){$taskRunnerArgs+='--absence-replay'}
    & $taskRunner @taskRunnerArgs
    if($LASTEXITCODE -ne 0){throw 'Native package acceptance failed; isolated diagnostic files retained'}
}finally{Pop-Location}
