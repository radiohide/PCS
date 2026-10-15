# PowerShell 7. Elevates a fixed read-only native assessment, never Codex or the product daemon.
param([switch]$Elevate,[string]$OutputRoot)
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
if(-not $OutputRoot){$OutputRoot=Join-Path $project 'target/native-permission-assessment'}
$output=Join-Path $OutputRoot ([Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $output -Force|Out-Null
$executable=Join-Path $project 'target/debug/pcscope-platform-native.exe'
if(-not (Test-Path -LiteralPath $executable)){throw 'Build plugins/platform-native/build.ps1 first'}
# Freeze the reviewed binary and manifests for this run; later builds cannot replace the launched file.
foreach($name in @('pcscope-platform-native.exe','channels.json','collector-options.json')){Copy-Item -LiteralPath (Join-Path $project ('target/debug/'+$name)) -Destination $output}
$executable=Join-Path $output 'pcscope-platform-native.exe'
$sid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value
function Quote-Windows([string]$value){if($value.Contains('"')){throw 'Unexpected quote in argument'};return '"'+$value.TrimEnd('\')+'"'}
$arguments='assess '+(Quote-Windows $output)+' '+(Quote-Windows $sid)
@{executable_sha256=(Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash.ToLowerInvariant();requested_elevation=[bool]$Elevate;operation='read-only capabilities, driver-store inventory and CNG metadata; no repair or daemon';output=$output}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $output 'request.json') -Encoding utf8
$parameters=@{FilePath=$executable;ArgumentList=$arguments;WindowStyle='Hidden';PassThru=$true}
if($Elevate){$parameters.Verb='RunAs'}
try{$process=Start-Process @parameters}catch{throw ('Windows did not start the assessment (including cancelled UAC): '+$_.Exception.Message)}
$clock=[Diagnostics.Stopwatch]::StartNew()
while(-not $process.WaitForExit(1000)){if($clock.Elapsed.TotalSeconds -gt 180){throw 'Native assessment exceeded 180 seconds; output remains isolated. Inspect the owned process before retrying.'}}
if($process.ExitCode -ne 0){throw "Native assessment exited $($process.ExitCode); no success is claimed"}
$complete=Get-Content -Raw -LiteralPath (Join-Path $output 'complete.json')|ConvertFrom-Json
if(-not $complete.same_user -or -not $complete.complete -or $complete.repair){throw 'Assessment contract failed'}
if($Elevate -and -not $complete.elevated){throw 'Windows did not grant an elevated token'}
$capabilities=Get-Content -Raw -LiteralPath (Join-Path $output 'capabilities.json')|ConvertFrom-Json
$sources=@(Get-Content -LiteralPath (Join-Path $output 'native-sources.jsonl')|ForEach-Object {ConvertFrom-Json -AsHashtable $_})
@{output=$output;elevated=$complete.elevated;same_user=$complete.same_user;repair=$complete.repair;logs_readable=@($capabilities.logs|Where-Object readable).Count;logs_total=@($capabilities.logs).Count;driver_items=($sources|Where-Object { $_.source -eq 'pcscope.hardware.driver-packages' -and $_.type -eq 'snapshot'}|ForEach-Object {@($_.items).Count}|Measure-Object -Sum).Sum;driver_gaps=@($sources|Where-Object { $_.source -eq 'pcscope.hardware.driver-packages' -and $_.type -eq 'gap'}).Count;cng_gaps=@($sources|Where-Object { $_.source -eq 'pcscope.security.key-metadata' -and $_.type -eq 'gap'}).Count}|ConvertTo-Json -Compress
