param([string]$OutputRoot)
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
if(-not $OutputRoot){$OutputRoot=Join-Path $project 'target/n04-source-fixtures'}
$fixture=Join-Path $OutputRoot ([Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture -Force | Out-Null
$domains=Join-Path $project 'plugins/powershell-adapter/domains'
Add-Type -Path (Join-Path $domains 'MetadataNative.cs')
function Assert($condition,[string]$message){if(-not $condition){throw $message}}
function Item([string]$id,$data){[pscustomobject]@{id=$id;data=$data}}
function Iso($value){if($null -ne $value){([datetime]$value).ToUniversalTime().ToString('o')}}
function Hash-Text([string]$text){$sha=[Security.Cryptography.SHA256]::Create();try{([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($text)))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}}
$script:captured=[Collections.Generic.List[object]]::new()
$script:disabled=@()
function Emit([string]$source,[scriptblock]$query){
    if($source -in $script:disabled){return}
    try{$items=@(& $query);$script:captured.Add([pscustomobject]@{source='pcscope.'+$source;scope=$scopeContext;items=$items;error=$null})}
    catch{$script:captured.Add([pscustomobject]@{source='pcscope.'+$source;scope=$scopeContext;items=@();error=$_.Exception.Message})}
}
function Fields($object,[string[]]$names){$data=@{};foreach($name in $names){$data[$name]=$object.$name};$data}
# Dot-source the actual scoped emitter with an isolated explicit list and default project options.
$watched=Join-Path $fixture 'watched.txt'
[IO.File]::WriteAllText($watched,'first')
$WatchListJson=ConvertTo-Json -InputObject @($watched) -Compress
$DirectoryListJson=ConvertTo-Json -InputObject @($fixture) -Compress
$scopeContext=''
. (Join-Path $domains 'scoped.ps1')
$file=($script:captured|Where-Object source -eq 'pcscope.files.metadata-security')[0]
$directory=($script:captured|Where-Object source -eq 'pcscope.files.directory-security')[0]
Assert (-not $file.error) ('File collection failed: '+$file.error)
Assert (-not $directory.error) ('Directory collection failed: '+$directory.error)
Assert ($file.items[0].data.file_id) 'File identity was not collected'
Assert ($directory.items[0].data.file_id) 'Directory identity was not collected'
Assert ($directory.items[0].data.entries.Count -eq 1) 'Directory children were not collected'
$oldIdentity=[PCScopeMetadata]::FileIdentity($watched)
$renamed=Join-Path $fixture 'renamed.txt';Move-Item -LiteralPath $watched -Destination $renamed
$newIdentity=[PCScopeMetadata]::FileIdentity($renamed)
Assert (($oldIdentity -join '|') -eq ($newIdentity -join '|')) 'Rename changed filesystem identity'
$before=@(Scan-WatchedDirectory $fixture)
[IO.File]::WriteAllText((Join-Path $fixture 'added.txt'),'second')
$after=@(Scan-WatchedDirectory $fixture)
Assert ($after[0].data.entries.Count -eq $before[0].data.entries.Count+1) 'Directory changes were not observed'
New-Item -ItemType Directory -Path (Join-Path $fixture 'nested') | Out-Null
[IO.File]::WriteAllText((Join-Path $fixture 'nested/hidden.txt'),'not recursively read')
$nonRecursive=@(Scan-WatchedDirectory $fixture)
Assert (-not ($nonRecursive[0].data.entries|Where-Object name -eq 'hidden.txt')) 'Directory collection recursed'
$missing=@(Scan-WatchedDirectory (Join-Path $fixture 'missing'));Assert (-not $missing[0].data.exists) 'Missing directory was not explicit'
$rejected=$false;try{Scan-WatchedDirectory $renamed|Out-Null}catch{$rejected=$true};Assert $rejected 'Directory collector accepted a file'
# CNG enumeration does not open keys or expose real names in this report.
$providers=@([PCScopeMetadata]::Providers());$success=0;$gaps=0;$keys=0
foreach($provider in $providers){foreach($location in @($false,$true)){try{$keys+=@([PCScopeMetadata]::Keys($provider,$location)).Count;$success++}catch{$gaps++}}}
$result=@{directory_fixture='passed';file_identity_and_rename='passed';recursive_scan='excluded';missing_directory='explicit';cng_providers=$providers.Count;cng_scopes_read=$success;cng_scope_gaps=$gaps;cng_key_metadata_count=$keys;fixture=$fixture}
$lines=@();foreach($reading in $script:captured){if($reading.error){continue};$run=[Guid]::NewGuid().ToString('N');$lines+=ConvertTo-Json -InputObject @{protocol_version=1;request_id=$run;type='snapshot';source=$reading.source;scope="machine:fixture/user:fixture/context:$($reading.scope)/shard:0";run_id=$run;observed_at=[DateTime]::UtcNow.ToString('o');complete=$true;items=@($reading.items)} -Depth 18 -Compress}
[IO.File]::WriteAllLines((Join-Path $fixture 'sources.jsonl'),$lines,[Text.UTF8Encoding]::new($false))
$result|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $fixture 'result.json') -Encoding UTF8
$result|ConvertTo-Json
