$ErrorActionPreference='Stop'
$taskProjectRoot=Split-Path $PSScriptRoot -Parent
$taskRunRoot=Join-Path $taskProjectRoot ('target/native-owned-performance/'+[Guid]::NewGuid().ToString('N'))
$taskBin=Join-Path $taskRunRoot 'bin'
[void](New-Item -ItemType Directory -Path $taskBin -Force)
foreach($taskFile in @('pcscope-platform-native.exe','pcscope-powershell-adapter.exe')){
    $taskTarget=Join-Path $taskBin $taskFile
    Copy-Item -LiteralPath (Join-Path $taskProjectRoot ('target/debug/'+$taskFile)) -Destination $taskTarget
    & icacls.exe $taskTarget /setintegritylevel M | Out-Null
    if($LASTEXITCODE -ne 0){throw 'Isolated executable integrity setup failed'}
}
Copy-Item -LiteralPath (Join-Path $taskProjectRoot 'plugins/powershell-adapter/collector-options.json') -Destination (Join-Path $taskBin 'collector-options.json')
$taskOwnership=Get-Content (Join-Path $taskProjectRoot 'plugins/platform-native/ownership.json') -Raw|ConvertFrom-Json
$taskInventory=Get-Content (Join-Path $taskProjectRoot 'model/sources.json') -Raw|ConvertFrom-Json
$taskAllOwned=@($taskOwnership.PSObject.Properties|Where-Object {$_.Name -notin @('version','note')}|ForEach-Object Value)
$taskDisabled=@($taskInventory.sources|Where-Object {$_.source -notin $taskAllOwned}|ForEach-Object source)
$taskRequest=@{selection=@{disabled_sources=$taskDisabled};files=@();directories=@()}|ConvertTo-Json -Depth 6 -Compress
$taskReports=@()
foreach($taskUnit in @('pcscope-powershell-adapter.exe','system','security','storage','network','startup','desktop','applications','virtualization','scoped')){
    $taskInfo=[Diagnostics.ProcessStartInfo]::new()
    $taskInfo.FileName=Join-Path $taskBin 'pcscope-powershell-adapter.exe'
    if($taskUnit -ne 'pcscope-powershell-adapter.exe'){$taskInfo.ArgumentList.Add($taskUnit)}
    $taskInfo.UseShellExecute=$false;$taskInfo.CreateNoWindow=$true
    $taskInfo.RedirectStandardInput=$true;$taskInfo.RedirectStandardOutput=$true;$taskInfo.RedirectStandardError=$true
    $taskInfo.StandardOutputEncoding=[Text.UTF8Encoding]::new($false)
    $taskProcess=[Diagnostics.Process]::new();$taskProcess.StartInfo=$taskInfo
    $taskWatch=[Diagnostics.Stopwatch]::StartNew()
    try{
        [void]$taskProcess.Start();$taskProcess.StandardInput.Write($taskRequest);$taskProcess.StandardInput.Close()
        $taskOut=$taskProcess.StandardOutput.ReadToEndAsync();$taskErr=$taskProcess.StandardError.ReadToEndAsync()
        $taskNative=@{};$taskPeak=[long]0;$taskCpu=[double]0
        while(-not $taskProcess.WaitForExit(40)){
            if($taskWatch.Elapsed.TotalSeconds -gt 65){$taskProcess.Kill($true);throw ('Native performance unit timed out: '+$taskUnit)}
            foreach($taskChild in [Diagnostics.Process]::GetProcessesByName('pcscope-platform-native')){
                try{
                    if($taskChild.MainModule.FileName -ne (Join-Path $taskBin 'pcscope-platform-native.exe')){continue}
                    $taskNative[$taskChild.Id]=@{peak=$taskChild.PeakWorkingSet64;cpu_ms=$taskChild.TotalProcessorTime.TotalMilliseconds}
                }catch{}finally{$taskChild.Dispose()}
            }
        }
        $taskWatch.Stop()
        $taskText=$taskOut.GetAwaiter().GetResult();$taskError=$taskErr.GetAwaiter().GetResult()
        if($taskProcess.ExitCode -ne 0){throw ($taskUnit+' failed: '+$taskError)}
        $taskMessages=@($taskText -split '\r?\n'|Where-Object {$_}|ForEach-Object {$_|ConvertFrom-Json -AsHashtable})
        if(@($taskMessages|Where-Object {$_.source -in $taskDisabled}).Count){throw 'Disabled compatibility source was executed'}
        foreach($taskMetric in $taskNative.Values){$taskPeak=[Math]::Max($taskPeak,$taskMetric.peak);$taskCpu+=$taskMetric.cpu_ms}
        $taskSources=@($taskMessages|Group-Object source|ForEach-Object {
            [pscustomobject]@{source=$_.Name;snapshots=@($_.Group|Where-Object type -eq snapshot).Count;gaps=@($_.Group|Where-Object type -eq gap).Count;items=(@($_.Group|Where-Object type -eq snapshot|ForEach-Object items)).Count}
        })
        $taskReport=[pscustomobject]@{unit=$taskUnit;elapsed_ms=$taskWatch.ElapsedMilliseconds;native_observed_peak_mb=if($taskNative.Count){[Math]::Round($taskPeak/1MB,1)}else{$null};native_observed_cpu_ms=if($taskNative.Count){$taskCpu}else{$null};native_processes_observed=$taskNative.Count;output_bytes=[Text.Encoding]::UTF8.GetByteCount($taskText);sources=$taskSources}
        $taskReports+=@($taskReport)
        [pscustomobject]@{unit=$taskUnit;elapsed_ms=$taskReport.elapsed_ms;observed_mb=$taskReport.native_observed_peak_mb;source_count=$taskSources.Count}|ConvertTo-Json -Compress
    }finally{$taskProcess.Dispose()}
}
$taskSummary=[ordered]@{output=$taskRunRoot;run='one bounded ordinary-permission pass';units=$taskReports;legacy_scripts_present=$false;compatibility_sources_disabled=$taskDisabled;metrics_limit='CPU and peak values sampled while child was live; missing observations stay null; short processes can be missed';coverage_limit='state wrapper ownership only; optional roots/HTTP/Docker and standalone eventlog/USN/software execution units require separate gates';formal_backend_changed=$false;history_database_accessed=$false}
$taskSummary|ConvertTo-Json -Depth 10|Set-Content (Join-Path $taskRunRoot 'summary.json') -Encoding utf8
Write-Output ('SUMMARY '+(Join-Path $taskRunRoot 'summary.json'))

