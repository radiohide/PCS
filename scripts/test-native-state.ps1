# Isolated production routing checks. Does not start a daemon, alter devices or open history.db.
param([string]$BinaryRoot,[string]$OutputRoot)
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
if(-not $BinaryRoot){$BinaryRoot=Join-Path $project 'target/debug'}
if(-not $OutputRoot){$OutputRoot=Join-Path $project 'target/native-state-tests'}
$output=Join-Path $OutputRoot ([guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $output -Force | Out-Null
$inventory=Get-Content -Raw (Join-Path $project 'model/sources.json') | ConvertFrom-Json
$units=@{
    'base'=@('system.version','system.boot-state','startup.services','startup.tasks','storage.volumes','network.adapters','network.addresses','hardware.devices','system.hotfixes','virtualization.platform')
    'system'=@('execution.processes','identity.profiles','identity.logon-sessions','identity.machine','system.restore-points')
    'storage'=@('hardware.driver-services','hardware.driver-bindings','hardware.firmware','hardware.battery','power.configuration')
    'security'=@('security.certificates.currentuser','security.certificates.localmachine')
    'network'=@('network.routes','network.tcp','network.udp')
    'startup'=@('startup.task-details')
}
$utf8=[Text.UTF8Encoding]::new($false)
function Run-State([string]$exe,[string[]]$arguments,[string[]]$selected,[string]$name){
    $request=@{selection=@{disabled_sources=@($inventory.sources | Where-Object {$_.source -notin $selected} | ForEach-Object source)}}
    $start=[Diagnostics.ProcessStartInfo]::new();$start.FileName=$exe;$start.UseShellExecute=$false;$start.CreateNoWindow=$true
    $start.RedirectStandardInput=$true;$start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true;$start.StandardOutputEncoding=$utf8
    foreach($argument in $arguments){$start.ArgumentList.Add($argument)}
    $clock=[Diagnostics.Stopwatch]::StartNew();$process=[Diagnostics.Process]::Start($start)
    $stdout=$process.StandardOutput.ReadToEndAsync();$stderr=$process.StandardError.ReadToEndAsync()
    $process.StandardInput.Write(($request|ConvertTo-Json -Depth 10 -Compress));$process.StandardInput.Close()
    if(-not $process.WaitForExit(60000)){$process.Kill($true);throw "Timeout: $name"}
    $text=$stdout.GetAwaiter().GetResult();$diagnostic=$stderr.GetAwaiter().GetResult()
    if($process.ExitCode -ne 0){throw "Process failed: $name $diagnostic"}
    [IO.File]::WriteAllText((Join-Path $output ($name+'.jsonl')),$text,$utf8)
    $rows=@($text -split '\r?\n' | Where-Object {$_} | ForEach-Object {ConvertFrom-Json -AsHashtable $_})
    if(@($rows|Where-Object {$_.source -notin $selected}).Count){throw "Unexpected source: $name"}
    $process.Dispose();return @{rows=$rows;elapsed_ms=[math]::Round($clock.Elapsed.TotalMilliseconds);bytes=$utf8.GetByteCount($text)}
}
# No legacy scripts are copied: successful owned queries must be entirely native.
$stage=Join-Path $output 'isolated';New-Item -ItemType Directory -Path $stage | Out-Null
foreach($name in @('pcscope-powershell-adapter.exe','pcscope-platform-native.exe')){
    $file=Join-Path $stage $name;Copy-Item -LiteralPath (Join-Path $BinaryRoot $name) -Destination $file
    & icacls.exe $file /setintegritylevel M | Out-Null
    if($LASTEXITCODE -ne 0){throw 'Unable to set ordinary application integrity for isolated binary'}
}
$native=Join-Path $stage 'pcscope-platform-native.exe'
$contract=& $native state-test
if($LASTEXITCODE -ne 0){throw 'Native state contract failed'}
$summary=[Collections.Generic.List[object]]::new()
foreach($unit in @('base','system','storage','security','network','startup')){
    $selected=@($units[$unit] | ForEach-Object {'pcscope.'+$_})
    $arguments=if($unit -eq 'base'){@()}else{@($unit)}
    $run=Run-State (Join-Path $stage 'pcscope-powershell-adapter.exe') $arguments $selected $unit
    foreach($source in $selected){
        $messages=@($run.rows|Where-Object source -eq $source);$snapshots=@($messages|Where-Object type -eq 'snapshot');$gaps=@($messages|Where-Object type -eq 'gap')
        if(-not $messages.Count){throw "No output: $source"}
        if($gaps.Count -and $source -notin @('pcscope.system.restore-points','pcscope.power.configuration')){throw "Unexpected gap: $source $($gaps[0].reason)"}
        if($snapshots.Count){
            $expected=if($unit -eq 'base'){1}else{16};if($snapshots.Count -ne $expected -or $gaps.Count){throw "Atomic scope count: $source"}
            foreach($snapshot in $snapshots){
                if(-not $snapshot.complete){throw "Incomplete snapshot: $source"}
                if($unit -eq 'base' -and -not $snapshot.scope.EndsWith('/schema:1/')){throw "Changed base scope: $source"}
                if($unit -ne 'base' -and $snapshot.scope -notmatch '/context:/shard:[0-9a-f]$'){throw "Changed shard scope: $source"}
            }
        }
        $summary.Add(@{source=$source;snapshots=$snapshots.Count;items=@($snapshots|ForEach-Object items).Count;gaps=@($gaps|ForEach-Object reason);unit_ms=$run.elapsed_ms})
    }
}
# A low-integrity helper must reject filtered visibility, never infer deletions.
$low=Join-Path $output 'low-native.exe';Copy-Item -LiteralPath $native -Destination $low
& icacls.exe $low /setintegritylevel L | Out-Null
if($LASTEXITCODE -ne 0){throw 'Unable to prepare isolated low-integrity fixture'}
$blocked=Run-State $low @('base') @('pcscope.hardware.devices') 'low-integrity'
if($blocked.rows.Count -ne 1 -or $blocked.rows[0].type -ne 'gap' -or $blocked.rows[0].reason -notmatch 'integrity below'){throw 'Low integrity committed a misleading successful snapshot'}
# Missing helper has no legacy script to fall back to and must retain state.
$missing=Join-Path $output 'missing';New-Item -ItemType Directory -Path $missing | Out-Null
Copy-Item -LiteralPath (Join-Path $stage 'pcscope-powershell-adapter.exe') -Destination $missing
$missingRun=Run-State (Join-Path $missing 'pcscope-powershell-adapter.exe') @() @('pcscope.hardware.devices') 'missing-helper'
if($missingRun.rows.Count -ne 1 -or $missingRun.rows[0].type -ne 'gap' -or $missingRun.rows[0].reason -notmatch 'no PowerShell fallback'){throw 'Missing helper did not explicitly retain state'}
[IO.File]::WriteAllText((Join-Path $output 'summary.json'),($summary|ConvertTo-Json -Depth 8),$utf8)
@{output=$output;source_count=$summary.Count;no_legacy_scripts=$true;low_integrity='rejected';missing_helper='no fallback';contract=($contract|ConvertFrom-Json);sources=@($summary)} | ConvertTo-Json -Depth 8 -Compress
