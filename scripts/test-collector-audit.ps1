# Run from PowerShell 7. Read-only sources; summaries never print collected values.
param([string[]]$Domains=@('system','security','network','storage','startup','desktop','virtualization','applications','scoped'))
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
$destination=Join-Path $project 'target/debug'
foreach($name in @('collect-domain.ps1','collect-system.ps1','evidence.ps1')){Copy-Item -LiteralPath (Join-Path $project "plugins/powershell-adapter/$name") -Destination $destination -Force}
Copy-Item -LiteralPath (Join-Path $project 'plugins/powershell-adapter/domains') -Destination $destination -Recurse -Force
foreach($name in @('collect-eventlog.ps1','event-schema.ps1','channels.json')){Copy-Item -LiteralPath (Join-Path $project "plugins/eventlog/$name") -Destination $destination -Force}
if(-not (Test-Path -LiteralPath (Join-Path $destination 'collector-options.json'))){Copy-Item -LiteralPath (Join-Path $project 'plugins/powershell-adapter/collector-options.json') -Destination $destination}
foreach($domain in $Domains){
    $clock=[Diagnostics.Stopwatch]::StartNew()
    $lines=@(& (Join-Path $destination 'pcscope-powershell-adapter.exe') $domain)
    if($LASTEXITCODE -ne 0){[pscustomobject]@{domain=$domain;status='process_failed';exit_code=$LASTEXITCODE}|ConvertTo-Json -Compress;continue}
    $sources=@{}
    foreach($line in $lines){
        if([Text.Encoding]::UTF8.GetByteCount($line)+1 -gt 1048576){throw 'Output exceeded protocol size'}
        $m=ConvertFrom-Json -InputObject $line -AsHashtable
        if(-not $sources.ContainsKey($m.source)){$sources[$m.source]=@{source=$m.source;items=0;gaps=@();max_bytes=0}}
        $entry=$sources[$m.source];$entry.max_bytes=[Math]::Max($entry.max_bytes,[Text.Encoding]::UTF8.GetByteCount($line))
        if($m.type -eq 'gap'){$entry.gaps+=@($m.reason)}else{$entry.items+=@($m.items).Count}
    }
    [pscustomobject]@{domain=$domain;seconds=[Math]::Round($clock.Elapsed.TotalSeconds,1);sources=@($sources.Values|Sort-Object source)}|ConvertTo-Json -Depth 5 -Compress
}
