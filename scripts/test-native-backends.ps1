# Production wrappers, no daemon or formal database. PowerShell 7 harness only.
param([string]$OutputRoot,[string]$BinaryRoot)
$ErrorActionPreference='Stop';$project=Split-Path $PSScriptRoot -Parent
if(-not $OutputRoot){$OutputRoot=Join-Path $project 'target/native-backend-tests'}
$output=Join-Path $OutputRoot ([Guid]::NewGuid().ToString('N'));New-Item -ItemType Directory -Path $output -Force|Out-Null
$bin=if($BinaryRoot){[IO.Path]::GetFullPath($BinaryRoot)}else{Join-Path $project 'target/debug'};$utf8=[Text.UTF8Encoding]::new($false)
$fixture=Join-Path $output 'fixture';New-Item -ItemType Directory -Path $fixture|Out-Null
$file=Join-Path $fixture 'sample.txt';[IO.File]::WriteAllText($file,'native metadata fixture',$utf8)
$child=Join-Path $fixture 'child';New-Item -ItemType Directory -Path $child|Out-Null
[IO.File]::WriteAllText((Join-Path $child 'not-recursed.txt'),'nested fixture',$utf8)
$inventory=Get-Content -Raw (Join-Path $project 'model/sources.json')|ConvertFrom-Json
$ownership=Get-Content -Raw (Join-Path $project 'plugins/platform-native/ownership.json')|ConvertFrom-Json -AsHashtable
$timings=[Collections.Generic.List[object]]::new()
function Run([string]$exe,[string[]]$arguments,$request,[string]$name,[string]$executionRoot=$bin){
    $clock=[Diagnostics.Stopwatch]::StartNew()
    $start=[Diagnostics.ProcessStartInfo]::new();$start.FileName=Join-Path $executionRoot $exe;$start.UseShellExecute=$false;$start.CreateNoWindow=$true
    $start.RedirectStandardInput=$true;$start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true;$start.StandardOutputEncoding=$utf8
    foreach($arg in $arguments){$start.ArgumentList.Add($arg)}
    $process=[Diagnostics.Process]::Start($start);$stdout=$process.StandardOutput.ReadToEndAsync();$stderr=$process.StandardError.ReadToEndAsync()
    $process.StandardInput.Write(($request|ConvertTo-Json -Depth 20 -Compress));$process.StandardInput.Close()
    if(-not $process.WaitForExit(120000)){$process.Kill($true);throw "Native wrapper timeout: $name"}
    $text=$stdout.GetAwaiter().GetResult();$diagnostic=$stderr.GetAwaiter().GetResult()
    [IO.File]::WriteAllText((Join-Path $output ($name+'.stderr')),$diagnostic,$utf8)
    if($process.ExitCode -ne 0){throw "Wrapper failed: $name; diagnostic bytes=$($utf8.GetByteCount($diagnostic))"}
    [IO.File]::WriteAllText((Join-Path $output ($name+'.jsonl')),$text,$utf8)
    $messages=@($text -split '\r?\n'|Where-Object {$_}|ForEach-Object {ConvertFrom-Json -AsHashtable $_})
    $clock.Stop();$timings.Add(@{name=$name;elapsed_ms=$clock.Elapsed.TotalMilliseconds;stdout_bytes=$utf8.GetByteCount($text);messages=$messages.Count})
    $process.Dispose();return $messages
}
foreach($unit in @('applications','system','security','storage','scoped')){
    $selected=@($ownership[$unit]);$request=@{selection=@{disabled_sources=@($inventory.sources|Where-Object {$_.source -notin $selected}|ForEach-Object source)}}
    if($unit -eq 'scoped'){$request.files=@($file);$request.directories=@($fixture)}
    $messages=@(Run 'pcscope-powershell-adapter.exe' @($unit) $request $unit)
    if(-not $messages.Count){throw "No native result: $unit"}
    if(@($messages|Where-Object {$_.source -notin $selected}).Count){throw "Unexpected compatibility result: $unit"}
    if($unit -eq 'scoped'){
        $directory=@($messages|Where-Object source -eq 'pcscope.files.directory-security'|ForEach-Object items|Where-Object {$_})
        if(@($directory).Count -ne 1 -or @($directory[0].data.entries).Count -ne 2){throw 'Native directory did not return direct children only'}
        $request.internal_data_dir=$fixture
        $blocked=@(Run 'pcscope-powershell-adapter.exe' @($unit) $request 'self-watch')
        if(@($blocked|Where-Object { $_.source -in @('pcscope.files.directory-security','pcscope.files.metadata-security') -and $_.type -eq 'snapshot'}).Count){throw 'Internal-data self-watch was accepted'}
        if(@($blocked|Where-Object type -eq 'gap').Count -ne 2){throw 'Self-watch did not produce explicit gaps'}
    }
    @{unit=$unit;snapshots=@($messages|Where-Object type -eq 'snapshot').Count;gaps=@($messages|Where-Object type -eq 'gap').Count;items=($messages|Where-Object type -eq 'snapshot'|ForEach-Object {@($_.items).Count}|Measure-Object -Sum).Sum}|ConvertTo-Json -Compress|Write-Host
}
# Missing native helper cannot silently start a PowerShell fallback. Missing compatibility
# scripts must not discard successfully emitted native evidence.
$isolated=Join-Path $output 'isolated';New-Item -ItemType Directory -Path $isolated|Out-Null
Copy-Item -LiteralPath (Join-Path $bin 'pcscope-powershell-adapter.exe') -Destination $isolated
$selected=@($ownership.applications);$request=@{selection=@{disabled_sources=@($inventory.sources|Where-Object {$_.source -notin $selected}|ForEach-Object source)}}
$missing=@(Run 'pcscope-powershell-adapter.exe' @('applications') $request 'missing-native' $isolated)
if($missing.Count -ne 1 -or $missing[0].type -ne 'gap' -or $missing[0].reason -notmatch 'no PowerShell fallback'){throw 'Missing native helper did not fail explicitly'}
foreach($asset in @('pcscope-platform-native.exe','collector-options.json','channels.json')){Copy-Item -LiteralPath (Join-Path $bin $asset) -Destination $isolated}
$partial=@(Run 'pcscope-powershell-adapter.exe' @('applications') @{} 'missing-compatibility' $isolated)
if(-not @($partial|Where-Object {$_.type -eq 'snapshot' -and $_.source -eq $selected[0]}).Count -or -not @($partial|Where-Object type -eq 'gap').Count){throw 'Compatibility failure discarded native evidence'}
$hybrid=@(Run 'pcscope-powershell-adapter.exe' @('applications') @{} 'hybrid-applications')
if(-not @($hybrid|Where-Object {$_.type -eq 'snapshot' -and $_.source -eq $selected[0]}).Count){throw 'Packaged hybrid route lost native runtime evidence'}
@{unit='hybrid-applications';native_items=($hybrid|Where-Object {$_.type -eq 'snapshot' -and $_.source -eq $selected[0]}|ForEach-Object {@($_.items).Count}|Measure-Object -Sum).Sum;compatibility_gaps=@($hybrid|Where-Object type -eq 'gap').Count}|ConvertTo-Json -Compress|Write-Host
# Default wrapper must successfully read through the native backend, without a backend override.
if(Test-Path -LiteralPath (Join-Path $bin 'eventlog-backend.json')){throw 'Default-backend test requires no local override'}
$first=@(Run 'pcscope-eventlog.exe' @() @{channel='Application'} 'event-first')[0]
if($first.type -ne 'event_batch'){throw 'Default event backend did not read Application'}
$second=@(Run 'pcscope-eventlog.exe' @() @{channel='Application';resume=@{epoch=$first.epoch;batch_seq=$first.batch_seq;checkpoint=$first.checkpoint}} 'event-resume')[0]
if($second.type -ne 'event_batch' -or $second.epoch -ne $first.epoch -or $second.batch_seq -ne $first.batch_seq+1){throw 'Native resume contract failed'}
if(@($first.events.id|Where-Object {$_ -in $second.events.id}).Count){throw 'Native resume duplicated event IDs'}
[IO.File]::WriteAllText((Join-Path $output 'timings.json'),(ConvertTo-Json -InputObject @($timings) -Depth 10),$utf8)
@{output=$output;default_event_backend='native';resume='passed';self_watch='explicitly blocked';sources='native-owned only';daemon_started=$false}|ConvertTo-Json -Compress
