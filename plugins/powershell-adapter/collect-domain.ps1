param([ValidateSet('system','security','network','storage','startup','desktop','virtualization','applications','scoped')][string]$Domain,[string]$WatchListJson='[]',[string]$DirectoryListJson='[]',[string]$SelectionJson='{}')
$selection=ConvertFrom-Json $SelectionJson
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$machine=Get-ItemPropertyValue -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Cryptography' -Name MachineGuid
$sid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$run=[Guid]::NewGuid().ToString()
$scopeContext=''
. (Join-Path $PSScriptRoot 'evidence.ps1')
function Item([string]$id,$data){if([string]::IsNullOrWhiteSpace($id)){throw 'Missing identity'};[pscustomobject]@{id=$id;data=$data}}
function Hash-Text([string]$text){$sha=[Security.Cryptography.SHA256]::Create();try{([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($text)))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}}
function Iso($value){if($null -ne $value){([datetime]$value).ToUniversalTime().ToString('o')}}
function Fields($object,[string[]]$names){$result=[ordered]@{};foreach($name in $names){$result[$name]=Protect-Value $name $object.$name};if($object -is [Microsoft.Management.Infrastructure.CimInstance]){$result['source_properties']=Public-CimFields $object};return $result}
function Emit([string]$source,[scriptblock]$query){
    $source='pcscope.'+$source;if($source -in $selection.disabled_sources){return};$observed=[DateTime]::UtcNow.ToString('o')
    try{
        $items=@(& $query);$ids=@{};$buckets=@{}
        foreach($item in $items){if($ids.ContainsKey($item.id)){throw "Duplicate identity: $($item.id)"};$ids[$item.id]=$true;$bucket=(Hash-Text $item.id).Substring(0,1);if(-not $buckets.ContainsKey($bucket)){$buckets[$bucket]=[Collections.Generic.List[object]]::new()};$buckets[$bucket].Add($item)}
        # Encode every shard first: failures never emit a partially completed source run.
        $lines=[Collections.Generic.List[string]]::new()
        foreach($bucket in '0123456789abcdef'.ToCharArray()){
            $key=[string]$bucket;$subset=@();if($buckets.ContainsKey($key)){$subset=@($buckets[$key].ToArray()|Sort-Object id)}
            $line=ConvertTo-Json -InputObject ([ordered]@{protocol_version=1;request_id="$run/$source/$scopeContext/$key";type='snapshot';source=$source;scope="machine:$machine/user:$sid/schema:1/context:$scopeContext/shard:$key";run_id=$run;observed_at=$observed;complete=$true;items=$subset}) -Depth 18 -Compress
            if([Text.Encoding]::UTF8.GetByteCount($line)+1 -gt 1048576){throw 'Source shard exceeds protocol limit'};$lines.Add($line)
        }
        foreach($line in $lines){[Console]::WriteLine($line)}
    }catch{[Console]::WriteLine((ConvertTo-Json -InputObject ([ordered]@{protocol_version=1;request_id="$run/$source/$scopeContext/error";type='gap';source=$source;observed_at=$observed;reason="Source unavailable; previous state retained: $($_.Exception.Message)"}) -Compress))}
}
function Registry([string]$hive,[string]$path,[string[]]$allowed,[int]$depth=0){
    foreach($view in @('Registry64','Registry32')){
        $root=[Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::$hive,[Microsoft.Win32.RegistryView]::$view)
        try{Registry-Key $root $hive $view $path $allowed $depth}finally{$root.Dispose()}
    }
}
function Registry-Key($root,$hive,$view,$path,$allowed,$depth){
    $key=$root.OpenSubKey($path,$false);if($null -eq $key){return}
    try{
        $values=[ordered]@{}
        foreach($name in @($key.GetValueNames()|Sort-Object)){
            $value=$key.GetValue($name,$null,[Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            # Preserve configuration values, not only the historical display-field allowlist.
            $value=Protect-Value $name $value
            $values[$name]=@{kind=[string]$key.GetValueKind($name);value=$value}
        }
        Item "$hive/$view/$path" @{path=$path;hive=$hive;view=$view;values=$values;basis='configuration snapshot';collection=@{policy='evidence-v2';scope='configured_registry_paths_and_depth_only';remaining_depth=$depth;unvisited_subkeys=if($depth -eq 0){$key.SubKeyCount}else{0}}}
        if($depth -gt 0){foreach($child in @($key.GetSubKeyNames()|Sort-Object)){Registry-Key $root $hive $view "$path\$child" $allowed ($depth-1)}}
    }finally{$key.Dispose()}
}
function Cim-Items([string]$class,[string]$identity,[string[]]$properties,[string]$namespace='root/cimv2'){
    Get-CimInstance -Namespace $namespace -ClassName $class | ForEach-Object {Item ([string]$_.$identity) (Fields $_ $properties)}
}
. (Join-Path $PSScriptRoot "domains/$Domain.ps1")
