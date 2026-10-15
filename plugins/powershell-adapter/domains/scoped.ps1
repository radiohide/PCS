$options=Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot '../collector-options.json')|ConvertFrom-Json
$watchFiles=ConvertFrom-Json $WatchListJson
if($null -eq $watchFiles){$watchFiles=@()}
$watchDirectories=@(ConvertFrom-Json $DirectoryListJson)
function Add-FileIdentity($data,[string]$path){
    if(-not ('PCScopeMetadata' -as [type])){Add-Type -Path (Join-Path $PSScriptRoot 'MetadataNative.cs')}
    $identity=[PCScopeMetadata]::FileIdentity($path)
    $data.volume_identity=$identity[0];$data.file_id=$identity[1]
}
function Scan-WatchedDirectory([string]$path){
    if($path -notmatch '^[A-Za-z]:\\' -or $path.Substring(3) -match '[:*?<>|\"]'){throw 'Only explicit local directory paths are supported'}
    try{$directory=Get-Item -LiteralPath $path -Force -ErrorAction Stop}catch{
        if($_.CategoryInfo.Category -eq 'ObjectNotFound'){Item $path.ToLowerInvariant() @{path=$path;exists=$false;basis='configured directory absent; not proof of deletion'};return};throw
    }
    if(-not $directory.PSIsContainer -or ($directory.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Directory must be ordinary; reparse targets are not followed'}
    $entries=@(Get-ChildItem -LiteralPath $directory.FullName -Force | Sort-Object Name | ForEach-Object {@{name=$_.Name;directory=$_.PSIsContainer;size=if($_.PSIsContainer){$null}else{$_.Length};last_write_utc=(Iso $_.LastWriteTimeUtc);reparse_point=(($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)}})
    if($entries.Count -gt $options.max_files_per_root){throw 'Directory entry limit exceeded; snapshot discarded without replacing previous state'}
    $acl=Get-Acl -LiteralPath $directory.FullName
    $data=@{path=$directory.FullName;exists=$true;attributes=[string]$directory.Attributes;owner=$acl.Owner;dacl_sddl=$acl.Sddl;entries=$entries;last_write_utc=(Iso $directory.LastWriteTimeUtc);basis='explicit directory and immediate child metadata; no recursion or file content'}
    Add-FileIdentity $data $directory.FullName
    Item $directory.FullName.ToLowerInvariant() $data
}
if($options.max_files_per_root -lt 1 -or $options.max_files_per_root -gt 100000 -or $options.max_hash_bytes -lt 0 -or $options.max_hash_bytes -gt 268435456){throw 'Invalid bounded scan configuration'}
function Scan-Root([string]$path,[bool]$portable){
    if(-not [IO.Path]::IsPathRooted($path) -or $path.StartsWith('\\')){throw 'Only explicit local absolute roots are supported'}
    $base=Get-Item -LiteralPath $path
    if(-not $base.PSIsContainer -or ($base.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Root must be an ordinary directory'}
    $queue=[Collections.Generic.Stack[string]]::new();$queue.Push($base.FullName);$count=0
    while($queue.Count){
        $directory=$queue.Pop()
        foreach($file in @(Get-ChildItem -LiteralPath $directory -Force)){
            $count++;if($count -gt $options.max_files_per_root){throw 'File scan limit reached; source snapshot discarded'}
            $reparse=($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0
            if($file.PSIsContainer -and -not $reparse){$queue.Push($file.FullName)}
            if($portable -and ($file.PSIsContainer -or $file.Extension -notin @('.exe','.dll','.msi'))){continue}
            $data=@{path=$file.FullName;directory=$file.PSIsContainer;size=if($file.PSIsContainer){$null}else{$file.Length};creation_utc=(Iso $file.CreationTimeUtc);last_write_utc=(Iso $file.LastWriteTimeUtc);attributes=[string]$file.Attributes;reparse_point=$reparse;basis='bounded snapshot; creation time is filesystem metadata, not installation time';content_stored=$false;hash_status=if(-not $options.hash_files){'disabled_by_configuration'}elseif($file.PSIsContainer){'not_a_file'}elseif($reparse){'reparse_target_not_read'}elseif($file.Length -gt $options.max_hash_bytes){'skipped_size_limit'}else{'pending'};security_status=if($reparse){'reparse_target_not_read'}else{'owner_and_dacl_only_no_sacl'}}
            if(-not $reparse){$acl=$file.GetAccessControl([Security.AccessControl.AccessControlSections]::Access -bor [Security.AccessControl.AccessControlSections]::Owner);$data.owner=$acl.Owner;$data.dacl_sddl=$acl.GetSecurityDescriptorSddlForm([Security.AccessControl.AccessControlSections]::Access -bor [Security.AccessControl.AccessControlSections]::Owner)
                if(-not $file.PSIsContainer){
                    $data.streams=@(Get-Item -LiteralPath $file.FullName -Stream * | ForEach-Object {@{name=$_.Stream;size=$_.Length}})
                    if($portable){$v=[Diagnostics.FileVersionInfo]::GetVersionInfo($file.FullName);$data.version=$v.FileVersion;$data.company=$v.CompanyName;$data.signature_status='not_verified'}
                    if($options.hash_files -and $file.Length -le $options.max_hash_bytes){$sha=[Security.Cryptography.SHA256]::Create();$stream=$null;try{$stream=[IO.File]::OpenRead($file.FullName);$data.sha256=([BitConverter]::ToString($sha.ComputeHash($stream))).Replace('-','').ToLowerInvariant()}finally{if($null -ne $stream){$stream.Dispose()};$sha.Dispose()};$file.Refresh();if($data.size -ne $file.Length -or $data.last_write_utc -ne (Iso $file.LastWriteTimeUtc)){throw 'File changed during hashing; snapshot discarded'}}
                }
            }
            if($data.ContainsKey('sha256')){$data.hash_status='collected'}
            Add-FileIdentity $data $file.FullName
            Item $file.FullName.ToLowerInvariant() $data
        }
    }
}
function Scan-WatchedFile([string]$path){
    if($path -notmatch '^[A-Za-z]:\\' -or $path.Substring(3) -match '[:*?<>|\"]'){throw 'Only explicit local file paths are supported'}
    try{$file=Get-Item -LiteralPath $path -Force -ErrorAction Stop}catch{
        if($_.CategoryInfo.Category -eq 'ObjectNotFound'){Item $path.ToLowerInvariant() @{path=$path;exists=$false;basis='explicit file path absent at observation; not proof of deletion'};return};throw
    }
    if($file.PSIsContainer){throw 'The watch list accepts files only, not directories'}
    $reparse=($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0
    $data=@{path=$file.FullName;exists=$true;directory=$false;size=$file.Length;creation_utc=(Iso $file.CreationTimeUtc);last_write_utc=(Iso $file.LastWriteTimeUtc);attributes=[string]$file.Attributes;content_stored=$false;reparse_point=$reparse;basis='explicit file allowlist snapshot; no recursive directory scan';hash_status='disabled_by_configuration'}
    if(-not $reparse){
        $acl=$file.GetAccessControl([Security.AccessControl.AccessControlSections]::Access -bor [Security.AccessControl.AccessControlSections]::Owner)
        $data.owner=$acl.Owner;$data.dacl_sddl=$acl.GetSecurityDescriptorSddlForm([Security.AccessControl.AccessControlSections]::Access -bor [Security.AccessControl.AccessControlSections]::Owner)
        $data.streams=@(Get-Item -LiteralPath $file.FullName -Stream * | ForEach-Object {@{name=$_.Stream;size=$_.Length}})
        if($options.hash_files){
            $data.hash_status='skipped_size_limit'
            if($file.Length -le $options.max_hash_bytes){$sha=[Security.Cryptography.SHA256]::Create();$stream=$null;try{$stream=[IO.File]::OpenRead($file.FullName);$data.sha256=([BitConverter]::ToString($sha.ComputeHash($stream))).Replace('-','').ToLowerInvariant()}finally{if($stream){$stream.Dispose()};$sha.Dispose()};$data.hash_status='collected';$file.Refresh();if($data.size -ne $file.Length -or $data.last_write_utc -ne (Iso $file.LastWriteTimeUtc)){throw 'File changed during hashing; snapshot discarded'}}
        }
    }else{$data.hash_status='reparse_target_not_read';$data.security_status='reparse_target_not_read'}
    Add-FileIdentity $data $path
    Item $path.ToLowerInvariant() $data
}
Emit 'collector.scoped-configuration' {Item 'options' @{watch_files=$watchFiles;watch_directories=$watchDirectories;file_roots=@();portable_roots=@($options.portable_roots);module_process_names=@($options.module_process_names);hash_files=$options.hash_files;max_files_per_root=$options.max_files_per_root;max_hash_bytes=$options.max_hash_bytes;docker_metadata=[bool]$options.docker_metadata;mihomo_metadata=[bool]$options.mihomo_metadata;content_versions='not implemented';basis='explicit file allowlist from plugin configuration; empty list means no file monitoring; legacy file_roots ignored'}}
if($options.mihomo_metadata){Emit 'apps.mihomo' {
    if($options.mihomo_port -lt 1 -or $options.mihomo_port -gt 65535){throw 'Invalid Mihomo loopback port'}
    $headers=@{};if($env:PCSCOPE_MIHOMO_TOKEN){$headers.Authorization='Bearer '+$env:PCSCOPE_MIHOMO_TOKEN}
    $base='http://127.0.0.1:'+[string]$options.mihomo_port
    $version=Invoke-RestMethod -Uri ($base+'/version') -Headers $headers -Method Get -TimeoutSec 5 -MaximumRedirection 0
    $config=Invoke-RestMethod -Uri ($base+'/configs') -Headers $headers -Method Get -TimeoutSec 5 -MaximumRedirection 0
    Item 'runtime' @{version=$version.version;meta=$version.meta;mode=$config.mode;log_level=$config.'log-level';ipv6=$config.ipv6;allow_lan=$config.'allow-lan';port=$config.port;socks_port=$config.'socks-port';mixed_port=$config.'mixed-port';tun_enabled=$config.tun.enable;basis='explicit local Mihomo API; no subscription, node credentials or traffic content'}
}}
if($options.docker_metadata){Emit 'virtualization.docker' {
    $docker=Join-Path $env:ProgramFiles 'Docker/Docker/resources/bin/docker.exe'
    if(-not (Test-Path -LiteralPath $docker)){throw 'Supported Docker Desktop CLI is not installed'}
    $endpoint='npipe:////./pipe/docker_engine'
    foreach($spec in @(
        @{type='container';args=@('container','ls','--all','--no-trunc');template='{"id":{{json .ID}},"names":{{json .Names}},"image":{{json .Image}},"state":{{json .State}},"status":{{json .Status}}}'},
        @{type='image';args=@('image','ls','--no-trunc');template='{"id":{{json .ID}},"repository":{{json .Repository}},"tag":{{json .Tag}}}'},
        @{type='volume';args=@('volume','ls');template='{"id":{{json .Name}},"driver":{{json .Driver}}}'},
        @{type='network';args=@('network','ls','--no-trunc');template='{"id":{{json .ID}},"name":{{json .Name}},"driver":{{json .Driver}},"scope":{{json .Scope}}}'})
    ){
        $arguments=@('--host',$endpoint)+$spec.args+@('--format',$spec.template)
        $lines=@(& $docker @arguments 2>$null);if($LASTEXITCODE -ne 0){throw "Docker $($spec.type) query failed"}
        foreach($line in $lines){$data=ConvertFrom-Json $line;$identity=if($spec.type -eq 'image'){"$($data.id)/$($data.repository)/$($data.tag)"}else{$data.id};Item "$($spec.type)/$identity" @{kind=$spec.type;data=$data;basis='local Docker API metadata; no logs, arguments, environment or container contents'}}
    }
}}
foreach($path in $watchFiles){$scopeContext=Hash-Text "watch-file/$path";Emit 'files.metadata-security' {Scan-WatchedFile $path}}
foreach($path in $watchDirectories){$scopeContext=Hash-Text "watch-directory/$path";Emit 'files.directory-security' {Scan-WatchedDirectory $path}}
foreach($path in @($options.portable_roots)){$scopeContext=Hash-Text "$path/$($options.hash_files)/$($options.max_files_per_root)/$($options.max_hash_bytes)";Emit 'software.portable' {Scan-Root $path $true}}
$scopeContext=Hash-Text (ConvertTo-Json -InputObject @($options.module_process_names) -Compress)
if(@($options.module_process_names).Count){Emit 'execution.modules' {foreach($name in @($options.module_process_names)){if($name -match '[*?\[\]]'){throw 'Wildcard process selections are not supported'};foreach($process in @(Get-Process|Where-Object ProcessName -eq $name)){try{$started=Iso $process.StartTime;foreach($module in $process.Modules){Item "$($process.Id)/$started/$($module.FileName)" @{pid=$process.Id;process_start=$started;process_name=$name;module_path=$module.FileName;size=$module.ModuleMemorySize;version=$module.FileVersionInfo.FileVersion;company=$module.FileVersionInfo.CompanyName;basis='selected process module snapshot'}}}finally{$process.Dispose()}}}}}

