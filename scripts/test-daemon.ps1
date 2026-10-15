$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
Push-Location $project
$daemon=$null
$previousDataDir=$env:PCSCOPE_DATA_DIR
try {
    & .\build.ps1 -CargoArgs @('build','--workspace','--examples')
    & .\build.ps1 -CargoArgs @('build','--workspace')
    Copy-Item plugins/eventlog/collect-eventlog.ps1 target/debug/collect-eventlog.ps1 -Force
    Copy-Item plugins/powershell-adapter/collect-system.ps1 target/debug/collect-system.ps1 -Force
    Copy-Item plugins/powershell-adapter/evidence.ps1 target/debug/evidence.ps1 -Force
    Copy-Item plugins/eventlog/event-schema.ps1 target/debug/event-schema.ps1 -Force
    Copy-Item plugins/eventlog/channels.json target/debug/channels.json -Force
    Copy-Item plugins/powershell-adapter/collect-domain.ps1 target/debug/collect-domain.ps1 -Force
    Copy-Item plugins/powershell-adapter/domains target/debug/ -Recurse -Force
    if(-not (Test-Path target/debug/collector-options.json)){Copy-Item plugins/powershell-adapter/collector-options.json target/debug/collector-options.json}
    $data=Join-Path $project ('target\daemon-smoke\'+[guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $data -Force | Out-Null
    $env:PCSCOPE_DATA_DIR=$data
    $daemon=Start-Process -FilePath (Join-Path $project 'target\debug\pcscope-service.exe') -ArgumentList ('"'+$data+'"') -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $data 'daemon.stderr.log')
    Start-Sleep -Milliseconds 800
    if($daemon.HasExited){throw 'Test daemon failed to start; check stderr log'}
    & .\target\debug\examples\probe.exe
    if($LASTEXITCODE -ne 0){throw 'IPC probe failed'}
    $duplicate=Start-Process -FilePath (Join-Path $project 'target\debug\pcscope-service.exe') -ArgumentList ('"'+$data+'"') -WindowStyle Hidden -PassThru -Wait -RedirectStandardError (Join-Path $data 'duplicate.stderr.log')
    if($duplicate.ExitCode -eq 0){throw 'Duplicate daemon was not rejected'}
    Write-Output 'PASS: duplicate daemon rejected'
    & .\target\debug\examples\probe.exe shutdown
    if($LASTEXITCODE -ne 0){throw 'Graceful shutdown request failed'}
    if(-not $daemon.WaitForExit(180000)){throw 'Graceful shutdown timed out'}
    if($daemon.ExitCode -ne 0){throw 'Daemon exited unsuccessfully'}
    Write-Output 'PASS: graceful shutdown'
} finally {
    $env:PCSCOPE_DATA_DIR=$previousDataDir
    if($daemon -and -not $daemon.HasExited){Stop-Process -Id $daemon.Id -ErrorAction SilentlyContinue}
    Pop-Location
}


