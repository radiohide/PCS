# PowerShell 7. Export a read-only fixed Application-log fixture, then drain both backends.
param([Parameter(Mandatory)][string]$OutputDirectory,[int]$MaxBatches=200)
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent;$utf8=[Text.UTF8Encoding]::new($false)
$output=Join-Path $OutputDirectory ('eventlog-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $output|Out-Null
$archive=Join-Path $output 'application.evtx'
& (Join-Path $env:SystemRoot 'System32/wevtutil.exe') epl Application $archive
if($LASTEXITCODE -ne 0){throw 'Could not export Application log without changing it'}
$results=@()
foreach($backend in @('powershell','native')){
    $events=[Collections.Generic.List[object]]::new();$batches=@();$resume=$null;$hasMore=$true
    $destination=Join-Path $output ($backend+'.jsonl');$writer=[IO.StreamWriter]::new($destination,$false,$utf8)
    try{
        for($batch=0;$batch -lt $MaxBatches -and $hasMore;$batch++){
            $start=[Diagnostics.ProcessStartInfo]::new();$start.UseShellExecute=$false;$start.CreateNoWindow=$true
            $start.RedirectStandardInput=$true;$start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true;$start.StandardOutputEncoding=$utf8
            if($backend -eq 'powershell'){
                $start.FileName=Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe'
                foreach($arg in @('-NoLogo','-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',(Join-Path $project 'plugins/eventlog/collect-eventlog.ps1'))){$start.ArgumentList.Add($arg)}
                [void]$start.Environment.Remove('PSModulePath');[void]$start.Environment.Remove('PSExecutionPolicyPreference')
            }else{$start.FileName=Join-Path $project 'dist/n04-source-gaps/pcscope-eventlog-native.exe'}
            $clock=[Diagnostics.Stopwatch]::StartNew();$process=[Diagnostics.Process]::Start($start)
            $stdout=$process.StandardOutput.ReadToEndAsync();$stderr=$process.StandardError.ReadToEndAsync()
            $process.StandardInput.Write((@{channel='Application';archive_path=$archive;resume=$resume}|ConvertTo-Json -Depth 12 -Compress));$process.StandardInput.Close()
            if(-not $process.WaitForExit(30000)){$process.Kill($true);throw "$backend timed out"}
            $clock.Stop();$text=$stdout.GetAwaiter().GetResult();$diagnostic=$stderr.GetAwaiter().GetResult()
            if($process.ExitCode -ne 0){throw "$backend failed ($($process.ExitCode)); diagnostic bytes=$($utf8.GetByteCount($diagnostic))"}
            $messages=@($text -split '\r?\n'|Where-Object {$_}|ForEach-Object {ConvertFrom-Json -AsHashtable $_})
            if($messages.Count -ne 1 -or $messages[0].type -ne 'event_batch'){throw "$backend did not return one event batch"}
            $message=$messages[0];foreach($event in $message.events){$events.Add($event)}
            $batches+=@{seconds=$clock.Elapsed.TotalSeconds;events=@($message.events).Count;bytes=$utf8.GetByteCount($text)}
            $writer.Write($text)
            $resume=@{epoch=$message.epoch;batch_seq=$message.batch_seq;checkpoint=$message.checkpoint}
            $hasMore=($message.checkpoint|ConvertFrom-Json).has_more
            $process.Dispose()
        }
    }finally{$writer.Dispose()}
    if($hasMore){throw "$backend reached the bounded test batch limit before catching up"}
    $result=@{backend=$backend;events=$events.Count;batches=$batches;seconds=($batches.seconds|Measure-Object -Sum).Sum;file=$destination;has_more=$hasMore}
    $results+=,$result
    @{backend=$backend;events=$events.Count;seconds=$result.seconds;batches=$batches.Count}|ConvertTo-Json -Compress|Write-Host
}
$result=@{output=$output;archive_bytes=(Get-Item -LiteralPath $archive).Length;channel='Application';mode='fixed EVTX complete drain; direct backend processes; excludes IPC/UI';results=$results}
[IO.File]::WriteAllText((Join-Path $output 'result.json'),($result|ConvertTo-Json -Depth 10),$utf8)
Write-Output ('Event-log result: '+(Join-Path $output 'result.json'))
