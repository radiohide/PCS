param([string]$OutputRoot)
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
if(-not $OutputRoot){$OutputRoot=Join-Path $project 'target/n04-live-sources'}
$output=Join-Path $OutputRoot ([Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $output -Force|Out-Null
$inventory=Get-Content -Raw -LiteralPath (Join-Path $project 'model/sources.json')|ConvertFrom-Json
$script=Join-Path $project 'plugins/powershell-adapter/collect-domain.ps1'
$results=@()
foreach($spec in @(
    @{domain='applications';sources=@('pcscope.apps.dotnet-runtimes')},
    @{domain='system';sources=@('pcscope.system.update-history','pcscope.execution.resources')},
    @{domain='security';sources=@('pcscope.security.key-metadata','pcscope.collector.event-channel-coverage')},
    @{domain='storage';sources=@('pcscope.hardware.driver-packages')}
)){
    $disabled=@($inventory.sources|Where-Object {$_.source -notin $spec.sources}|ForEach-Object source)
    $selection=ConvertTo-Json -InputObject @{disabled_sources=$disabled} -Compress
    $start=[Diagnostics.ProcessStartInfo]::new()
    $start.FileName=Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe'
    $start.UseShellExecute=$false;$start.CreateNoWindow=$true;$start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
    $start.StandardOutputEncoding=[Text.UTF8Encoding]::new($false)
    foreach($argument in @('-NoLogo','-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',$script,'-Domain',$spec.domain,'-SelectionJson',$selection)){$start.ArgumentList.Add($argument)}
    $process=[Diagnostics.Process]::new();$process.StartInfo=$start
    [void]$process.Start();$stdout=$process.StandardOutput.ReadToEndAsync();$stderr=$process.StandardError.ReadToEndAsync()
    if(-not $process.WaitForExit(120000)){$process.Kill($true);throw "Collector timed out: $($spec.domain)"}
    $text=$stdout.GetAwaiter().GetResult();$diagnostic=$stderr.GetAwaiter().GetResult()
    if($process.ExitCode -ne 0){throw "Collector failed: $($spec.domain); exit=$($process.ExitCode); $diagnostic"}
    $destination=Join-Path $output ($spec.domain+'.jsonl')
    [IO.File]::WriteAllText($destination,$text,[Text.UTF8Encoding]::new($false))
    $messages=@($text -split '\r?\n'|Where-Object {$_}|ForEach-Object {ConvertFrom-Json -AsHashtable $_})
    foreach($source in $spec.sources){
        $matching=@($messages|Where-Object source -eq $source)
        if(-not $matching.Count){throw "Source emitted no result: $source"}
        $items=0;foreach($message in $matching){$items+=@($message.items|Where-Object {$_}).Count}
        $gaps=@($matching|Where-Object type -eq 'gap')
        $results+=@{source=$source;items=$items;gaps=$gaps.Count;reasons=@($gaps|ForEach-Object reason);file=$destination}
    }
    $process.Dispose()
}
@{output=$output;sources=$results}|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding UTF8
# Print only counts and availability; original material stays in the ignored fixture directory.
@{output=$output;sources=$results}|ConvertTo-Json -Depth 8
