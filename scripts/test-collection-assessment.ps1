# PowerShell 7; reads source APIs with the current token, never starts the daemon.
param([string]$OutputRoot,[ValidateRange(1,10)][int]$Repeat=1,[ValidateSet('RemoteSigned','Bypass')][string]$CollectorPolicy='RemoteSigned',[string[]]$OnlyJobs=@())
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
if(-not $OutputRoot){$OutputRoot=Join-Path $project 'target/collection-assessment'}
$output=Join-Path $OutputRoot ([Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $output -Force|Out-Null
$utf8=[Text.UTF8Encoding]::new($false)
$fixture=Join-Path $output 'fixture';New-Item -ItemType Directory -Path $fixture|Out-Null
$file=Join-Path $fixture 'sample.txt';[IO.File]::WriteAllText($file,'assessment fixture',$utf8)
$os=Get-CimInstance Win32_OperatingSystem;$cpu=Get-CimInstance Win32_Processor;$computer=Get-CimInstance Win32_ComputerSystem
$identity=[Security.Principal.WindowsIdentity]::GetCurrent()
$environment=@{date=[DateTime]::UtcNow.ToString('o');os=$os.Caption;version=$os.Version;build=$os.BuildNumber;architecture=$os.OSArchitecture;cpu=@($cpu|ForEach-Object Name);logical_processors=$computer.NumberOfLogicalProcessors;ram_bytes=$computer.TotalPhysicalMemory;administrator=([Security.Principal.WindowsPrincipal]::new($identity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator);runner_powershell=$PSVersionTable.PSVersion.ToString();collector_powershell='Windows PowerShell 5.1';collector_process_policy=$CollectorPolicy;volume=@(Get-CimInstance Win32_LogicalDisk|Where-Object DriveType -eq 3|Select-Object FileSystem,Size,FreeSpace)}
[IO.File]::WriteAllText((Join-Path $output 'environment.json'),($environment|ConvertTo-Json -Depth 8),$utf8)
$ps5=Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe'
$jobs=[Collections.Generic.List[object]]::new()
function PsJob($name,$script,$arguments,$inputText=''){@{name=$name;exe=$ps5;args=@('-NoLogo','-NoProfile','-NonInteractive','-ExecutionPolicy',$CollectorPolicy,'-File',$script)+$arguments;input=$inputText;timeout=115;measurement='direct collector process; excludes Rust wrapper'}}
$jobs.Add((PsJob 'base' (Join-Path $project 'plugins/powershell-adapter/collect-system.ps1') @()))
foreach($domain in @('system','security','network','storage','startup','desktop','virtualization','applications','scoped')){
    $arguments=@('-Domain',$domain)
    if($domain -eq 'scoped'){$arguments+=@('-WatchListJson',(ConvertTo-Json -InputObject @($file) -Compress),'-DirectoryListJson',(ConvertTo-Json -InputObject @($fixture) -Compress))}
    $jobs.Add((PsJob $domain (Join-Path $project 'plugins/powershell-adapter/collect-domain.ps1') $arguments))
}
$jobs.Add(@{name='software';exe=(Join-Path $project 'dist/n04-source-gaps/pcscope-software.exe');args=@();input='';timeout=115;measurement='packaged debug native collector'})
$jobs.Add(@{name='usn';exe=(Join-Path $project 'dist/n04-source-gaps/pcscope-usn.exe');args=@();input=(@{watch=@{files=@($file);revision=1}}|ConvertTo-Json -Depth 5 -Compress);timeout=115;measurement='packaged debug native collector'})
foreach($channel in (Get-Content -Raw (Join-Path $project 'plugins/eventlog/channels.json')|ConvertFrom-Json)){
    $jobs.Add((PsJob ('event-'+$jobs.Count) (Join-Path $project 'plugins/eventlog/collect-eventlog.ps1') @() (@{channel=$channel}|ConvertTo-Json -Compress)))
}
$results=[Collections.Generic.List[object]]::new();$sources=@{}
function RunJob($job,$round){
    $start=[Diagnostics.ProcessStartInfo]::new();$start.FileName=$job.exe;$start.UseShellExecute=$false;$start.CreateNoWindow=$true
    $start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true;$start.RedirectStandardInput=$true;$start.StandardOutputEncoding=$utf8;$start.StandardErrorEncoding=$utf8
    # ProcessStartInfo does not apply pwsh's native powershell.exe environment cleanup.
    # PS5 must discover its own modules, rather than trying to load the PS7 Security module.
    if($job.exe -eq $ps5){[void]$start.Environment.Remove('PSModulePath');[void]$start.Environment.Remove('PSExecutionPolicyPreference')}
    foreach($arg in $job.args){$start.ArgumentList.Add([string]$arg)}
    $process=[Diagnostics.Process]::new();$process.StartInfo=$start
    $clock=[Diagnostics.Stopwatch]::StartNew();[void]$process.Start();$stdout=$process.StandardOutput.ReadToEndAsync();$stderr=$process.StandardError.ReadToEndAsync()
    $process.StandardInput.Write($job.input);$process.StandardInput.Close();$peak=0L;$timeout=$false
    while(-not $process.WaitForExit(50)){
        $process.Refresh();try{$peak=[Math]::Max($peak,$process.WorkingSet64)}catch{}
        if($clock.Elapsed.TotalSeconds -gt $job.timeout){$timeout=$true;$process.Kill($true);$process.WaitForExit();break}
    }
    $clock.Stop();$text=$stdout.GetAwaiter().GetResult();$diagnostic=$stderr.GetAwaiter().GetResult()
    $rawPath=Join-Path $output ($job.name+'-'+$round+'.jsonl');[IO.File]::WriteAllText($rawPath,$text,$utf8)
    [IO.File]::WriteAllText(($rawPath+'.stderr'),$diagnostic,$utf8)
    $messages=@();$parseErrors=0;$oversize=0;$maxLine=0
    foreach($line in ($text -split '\r?\n'|Where-Object {$_})){
        $length=$utf8.GetByteCount($line)+1;$maxLine=[Math]::Max($maxLine,$length);if($length -gt 1048576){$oversize++}
        try{$messages+=ConvertFrom-Json -AsHashtable $line}catch{$parseErrors++}
    }
    foreach($m in $messages){
        if(-not $sources.ContainsKey($m.source)){$sources[$m.source]=@{source=$m.source;jobs=@();snapshots=0;batches=0;items=0;events=0;gaps=0;incomplete=0;reasons=@();max_line_bytes=0;duplicate_ids=0;message_status=@{};raw_xml_missing=0;field_issues=0;shards=@{}}}
        $s=$sources[$m.source];if($job.name -notin $s.jobs){$s.jobs+=@($job.name)}
        $s.max_line_bytes=[Math]::Max($s.max_line_bytes,$utf8.GetByteCount(($m|ConvertTo-Json -Depth 100 -Compress))+1)
        switch($m.type){
            'gap'{$s.gaps++;$s.reasons+=@($m.reason)}
            'snapshot'{
                $s.snapshots++;$s.items+=@($m.items).Count;if(-not $m.complete){$s.incomplete++}
                $ids=@{};foreach($item in $m.items){if($ids.ContainsKey($item.id)){$s.duplicate_ids++};$ids[$item.id]=$true}
                if($m.scope -match '^(.*)/shard:([0-9a-f]+)$'){$key=$m.run_id+'/'+$Matches[1];if(-not $s.shards.ContainsKey($key)){$s.shards[$key]=@()};$s.shards[$key]+=$Matches[2]}
            }
            'event_batch'{
                $s.batches++;$s.events+=@($m.events).Count
                foreach($event in $m.events){if($event.kind -eq 'coverage.log_reset'){continue};if(-not $event.data.raw_xml){$s.raw_xml_missing++};$status=[string]$event.data.message_status;if(-not $s.message_status.ContainsKey($status)){$s.message_status[$status]=0};$s.message_status[$status]++;$s.field_issues+=@($event.data.collection.field_issues).Count}
            }
        }
    }
    $result=@{job=$job.name;round=$round;seconds=[Math]::Round($clock.Elapsed.TotalSeconds,4);cpu_seconds=[Math]::Round($process.TotalProcessorTime.TotalSeconds,4);peak_working_set_bytes=if($peak){$peak}else{$null};bytes=$utf8.GetByteCount($text);messages=$messages.Count;exit_code=$process.ExitCode;timeout=$timeout;parse_errors=$parseErrors;oversized_lines=$oversize;max_line_bytes=$maxLine;diagnostic_bytes=$utf8.GetByteCount($diagnostic);measurement=$job.measurement;file=$rawPath}
    $results.Add($result);$process.Dispose();$result|ConvertTo-Json -Compress|Write-Host
    return $messages
}
for($round=1;$round -le $Repeat;$round++){
    foreach($job in $jobs){if($OnlyJobs.Count -and $job.name -notin $OnlyJobs){continue};$messages=@(RunJob $job $round);$messages=@($messages|Where-Object {$_ -is [hashtable]}|ForEach-Object {$_})
        # One additional Application batch checks real cursor continuation; not a history drain.
        if($round -eq 1 -and $job.input -and ($job.input|ConvertFrom-Json).channel -eq 'Application'){
            $batch=@($messages|Where-Object type -eq 'event_batch')|Select-Object -Last 1
            if($batch){$resumeJob=PsJob 'application-resume' (Join-Path $project 'plugins/eventlog/collect-eventlog.ps1') @() (@{channel='Application';resume=@{epoch=$batch.epoch;batch_seq=$batch.batch_seq;checkpoint=$batch.checkpoint}}|ConvertTo-Json -Depth 10 -Compress);$null=RunJob $resumeJob 1}
        }
    }
}
$inventory=Get-Content -Raw (Join-Path $project 'model/sources.json')|ConvertFrom-Json
$missing=@($inventory.sources|Where-Object {$_.source -notlike 'pcscope.runtime.*' -and -not $sources.ContainsKey($_.source)}|ForEach-Object source)
foreach($s in $sources.Values){$s['incomplete_shard_groups']=@($s.shards.Values|Where-Object {@($_|Select-Object -Unique).Count -ne 16}).Count;$s.Remove('shards')}
$summary=@{output=$output;repeat=$Repeat;only_jobs=$OnlyJobs;ps5_module_path_policy='removed inherited PS7 PSModulePath/PSExecutionPolicyPreference for direct PS5 processes';memory_sampling_ms=50;environment=$environment;jobs=@($results);sources=@($sources.Values|Sort-Object source);declared_sources_without_output=$missing}
[IO.File]::WriteAllText((Join-Path $output 'summary.json'),($summary|ConvertTo-Json -Depth 30),$utf8)
Write-Output ('Assessment saved: '+(Join-Path $output 'summary.json'))
