$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
$fixture=Join-Path $project 'data/scoped-fixture'
New-Item -ItemType Directory -Path $fixture -Force|Out-Null
$sample=Join-Path $fixture 'sample.txt'
Set-Content -LiteralPath $sample -Value 'PCScope fixture, not user data'
Set-Content -LiteralPath (Join-Path $fixture 'unlisted.txt') -Value 'Must not be collected'
$optionsPath=Join-Path $project 'target/debug/collector-options.json'
$old=if(Test-Path -LiteralPath $optionsPath){[IO.File]::ReadAllText($optionsPath)}else{$null}
function Run-Scoped($files){
    $request=ConvertTo-Json -InputObject @{files=@($files)} -Compress
    $lines=@($request | & (Join-Path $project 'target/debug/pcscope-powershell-adapter.exe') scoped)
    if($LASTEXITCODE -ne 0){throw 'Scoped plugin failed'}
    @($lines|ForEach-Object {ConvertFrom-Json $_ -AsHashtable})
}
try{
    @{file_roots=@($fixture);portable_roots=@();module_process_names=@();hash_files=$true;max_files_per_root=20;max_hash_bytes=1024}|ConvertTo-Json|Set-Content $optionsPath -Encoding utf8
    $messages=@(Run-Scoped @())
    if(@($messages|Where-Object source -eq pcscope.files.metadata-security).Count){throw 'Empty list must ignore legacy file_roots'}
    $messages=@(Run-Scoped @($sample))
    if(@($messages|Where-Object type -eq gap).Count){throw (ConvertTo-Json -InputObject @($messages|Where-Object type -eq gap) -Compress)}
    $items=@($messages|Where-Object source -eq pcscope.files.metadata-security|ForEach-Object {$_.items})
    if($items.Count -ne 1 -or $items[0].data.sha256.Length -ne 64 -or -not $items[0].data.dacl_sddl -or $items[0].id -ne $sample.ToLowerInvariant()){throw 'Allowlist isolation or metadata/hash/ACL failed'}
    $messages=@(Run-Scoped @((Join-Path $fixture 'missing.txt')))
    $items=@($messages|Where-Object source -eq pcscope.files.metadata-security|ForEach-Object {$_.items})
    if($items.Count -ne 1 -or $items[0].data.exists -ne $false){throw 'Missing watched file must have explicit absent observation'}
    $messages=@(Run-Scoped @($fixture))
    if(-not @($messages|Where-Object {$_.source -eq 'pcscope.files.metadata-security' -and $_.type -eq 'gap'}).Count){throw 'Directory selection must fail closed'}
    'PASS: empty allowlist, ignored legacy directory roots, exact selected file, ACL/hash, absent file and directory rejection'
}finally{if($null -ne $old){[IO.File]::WriteAllText($optionsPath,$old)}else{Copy-Item (Join-Path $project 'plugins/powershell-adapter/collector-options.json') $optionsPath -Force}}