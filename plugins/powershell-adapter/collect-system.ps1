param([string]$SelectionJson='{}')
$selection=ConvertFrom-Json $SelectionJson
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false)
$machine=Get-ItemPropertyValue -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Cryptography' -Name MachineGuid
$sid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$run=[Guid]::NewGuid().ToString()
. (Join-Path $PSScriptRoot 'evidence.ps1')
function Item([string]$id,$data){if([string]::IsNullOrWhiteSpace($id)){throw 'Missing stable identity'};[pscustomobject]@{id=$id;data=$data}}
function Iso($date){if($null -eq $date){return $null};([datetime]$date).ToUniversalTime().ToString('o')}
function Emit-Collection([string]$source,[scriptblock]$query,[string]$scopeSuffix=''){
    if($source -in $selection.disabled_sources){return}
    $observed=[DateTime]::UtcNow.ToString('o')
    try {
        $items=@(& $query)
        $message=[ordered]@{protocol_version=1;request_id="$run/$source/$scopeSuffix";type='snapshot';source=$source;scope="machine:$machine/user:$sid/schema:1/$scopeSuffix";run_id=$run;observed_at=$observed;complete=$true;items=$items}
        $line=ConvertTo-Json -InputObject $message -Depth 14 -Compress
        if([Text.Encoding]::UTF8.GetByteCount($line)+1 -gt 1048576){throw 'Atomic snapshot exceeded 1 MiB; source requires streaming'}
        [Console]::WriteLine($line)
    }catch{
        $message=[ordered]@{protocol_version=1;request_id="$run/$source/$scopeSuffix/error";type='gap';source=$source;observed_at=$observed;reason="Collection failed; scope unchanged [$scopeSuffix]: $($_.Exception.GetType().Name): $($_.Exception.Message)"}
        [Console]::WriteLine((ConvertTo-Json -InputObject $message -Depth 5 -Compress))
    }
}
Emit-Collection 'pcscope.system.version' {
    $os=Get-CimInstance Win32_OperatingSystem
    Item 'operating-system' ([ordered]@{caption=$os.Caption;version=$os.Version;build=$os.BuildNumber;architecture=$os.OSArchitecture;install_time=(Iso $os.InstallDate);install_time_basis='Win32_OperatingSystem.InstallDate';computer_name=$os.CSName})
}
Emit-Collection 'pcscope.system.boot-state' {
    $os=Get-CimInstance Win32_OperatingSystem
    Item 'boot-state' ([ordered]@{last_boot_time=(Iso $os.LastBootUpTime);basis='snapshot';timezone=(Get-TimeZone).Id})
}
Emit-Collection 'pcscope.identity.accounts' {
    Get-LocalUser | ForEach-Object {Item $_.SID.Value ([ordered]@{name=$_.Name;enabled=$_.Enabled;description=$_.Description;last_logon=(Iso $_.LastLogon);password_last_set=(Iso $_.PasswordLastSet);account_expires=(Iso $_.AccountExpires);password_required=$_.PasswordRequired;user_may_change_password=$_.UserMayChangePassword;principal_source=[string]$_.PrincipalSource;created_at=$null;created_at_basis='not available from this source'})}
}
Emit-Collection 'pcscope.identity.groups' {
    Get-LocalGroup | ForEach-Object {$group=$_;$members=@(Get-LocalGroupMember -SID $group.SID | ForEach-Object {$_.SID.Value} | Sort-Object);Item $group.SID.Value ([ordered]@{name=$group.Name;description=$group.Description;members=$members})}
}
Emit-Collection 'pcscope.startup.services' {
    Get-CimInstance Win32_Service | ForEach-Object {Item $_.Name.ToLowerInvariant() ([ordered]@{name=$_.Name;display_name=$_.DisplayName;state=$_.State;start_mode=$_.StartMode;start_account=$_.StartName;service_type=$_.ServiceType;description=$_.Description;command=(Protect-Text $_.PathName);pid=$_.ProcessId;exit_code=$_.ExitCode;service_specific_exit_code=$_.ServiceSpecificExitCode})}
}
Emit-Collection 'pcscope.startup.tasks' {
    Get-ScheduledTask | ForEach-Object {$task=$_;$actions=@($task.Actions | ForEach-Object {Public-CimFields $_});Item ($task.TaskPath+$task.TaskName).ToLowerInvariant() ([ordered]@{name=$task.TaskName;path=$task.TaskPath;state=[string]$task.State;enabled=$task.Settings.Enabled;user_id=$task.Principal.UserId;run_level=[string]$task.Principal.RunLevel;actions=$actions})}
}
Emit-Collection 'pcscope.storage.volumes' {
    Get-CimInstance Win32_Volume | ForEach-Object {Item $_.DeviceID ([ordered]@{device_id=$_.DeviceID;drive_letter=$_.DriveLetter;label=$_.Label;file_system=$_.FileSystem;capacity=$_.Capacity;drive_type=$_.DriveType})}
}
Emit-Collection 'pcscope.network.adapters' {
    Get-NetAdapter -IncludeHidden | ForEach-Object {Item ([string]$_.InterfaceGuid) ([ordered]@{name=$_.Name;description=$_.InterfaceDescription;status=[string]$_.Status;mac_address=$_.MacAddress;link_speed=[string]$_.LinkSpeed;driver_version=$_.DriverVersion})}
}
Emit-Collection 'pcscope.network.addresses' {
    Get-NetIPAddress | ForEach-Object {Item "$($_.InterfaceIndex)/$($_.AddressFamily)/$($_.IPAddress)" ([ordered]@{interface_index=$_.InterfaceIndex;interface_alias=$_.InterfaceAlias;address=$_.IPAddress;prefix_length=$_.PrefixLength;address_family=[string]$_.AddressFamily;prefix_origin=[string]$_.PrefixOrigin;suffix_origin=[string]$_.SuffixOrigin;address_state=[string]$_.AddressState})}
}
Emit-Collection 'pcscope.virtualization.platform' {
    $computer=Get-CimInstance Win32_ComputerSystem
    Item 'hypervisor' ([ordered]@{hypervisor_present=$computer.HypervisorPresent;manufacturer=$computer.Manufacturer;model=$computer.Model})
    Get-CimInstance Win32_Processor | ForEach-Object {Item $_.DeviceID ([ordered]@{name=$_.Name;firmware_virtualization=$_.VirtualizationFirmwareEnabled;vm_monitor_extensions=$_.VMMonitorModeExtensions;slat=$_.SecondLevelAddressTranslationExtensions})}
}
Emit-Collection 'pcscope.hardware.devices' {
    Get-CimInstance Win32_PnPEntity | ForEach-Object {Item $_.DeviceID ([ordered]@{name=$_.Name;class=$_.PNPClass;class_guid=$_.ClassGuid;manufacturer=$_.Manufacturer;service=$_.Service;status=$_.Status;error_code=$_.ConfigManagerErrorCode})}
}
Emit-Collection 'pcscope.system.hotfixes' {
    Get-CimInstance Win32_QuickFixEngineering | ForEach-Object {Item "$($_.HotFixID)/$($_.ServicePackInEffect)" ([ordered]@{hotfix_id=$_.HotFixID;description=$_.Description;installed_on=[string]$_.InstalledOn;coverage='QFE only; not the complete Windows Update inventory'})}
}

function Hash-Text([string]$text){$sha=[Security.Cryptography.SHA256]::Create();try{([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($text)))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}}
function Hash-File([string]$path){$sha=[Security.Cryptography.SHA256]::Create();$stream=$null;try{$stream=[IO.File]::OpenRead($path);([BitConverter]::ToString($sha.ComputeHash($stream))).Replace('-','').ToLowerInvariant()}finally{if($null -ne $stream){$stream.Dispose()};$sha.Dispose()}}
$script:ShellFileCache=@{}
function Shell-FileMetadata([string]$raw){
    $path=[Environment]::ExpandEnvironmentVariables($raw.Trim().Trim('"'))
    if(-not [IO.Path]::IsPathRooted($path) -or $path.StartsWith('\\')){return [ordered]@{status='not_read';reason='relative or network path';raw_path=$raw}}
    if($script:ShellFileCache.ContainsKey($path)){return $script:ShellFileCache[$path]}
    $data=[ordered]@{path=$path;status='unknown';signature_status='not_verified'}
    try{
        $file=[IO.FileInfo]::new($path)
        if(-not $file.Exists){$data.status='not_found'}else{
            $data.status='observed';$data.length=$file.Length;$data.last_write_utc=$file.LastWriteTimeUtc.ToString('o')
            $version=[Diagnostics.FileVersionInfo]::GetVersionInfo($path);$data.file_version=$version.FileVersion;$data.company=$version.CompanyName
            if($file.Length -le 268435456){$data.sha256=Hash-File $path}else{$data.hash_status='skipped_size_limit'}
            try{$certificate=[Security.Cryptography.X509Certificates.X509Certificate]::CreateFromSignedFile($path);$data.embedded_signer=$certificate.Subject;$certificate.Dispose()}catch{$data.embedded_signer=$null}
            $file.Refresh();if($data.length -ne $file.Length -or $data.last_write_utc -ne $file.LastWriteTimeUtc.ToString('o')){$data.status='changed_during_read';$data.Remove('sha256')}
        }
    }catch{$data.status='unavailable';$data.error_type=$_.Exception.GetType().Name}
    $script:ShellFileCache[$path]=$data
    return $data
}
function Read-RegistryTree($root,[string]$path,[int]$depth){
    if($path -match '(?i)\\Classes\\CLSID\\' -and 'pcscope.shell.registrations.com' -in $selection.disabled_items){return}
    if('pcscope.shell.registrations.com' -in $selection.disabled_items){
        $checks=@{'preview'='\\PreviewHandlers';'overlay'='\\ShellIconOverlayIdentifiers';'approval'='\\Shell Extensions\\';'namespace'='\\(Desktop|MyComputer)\\NameSpace';'file-association'='\\Classes\\'}
        foreach($suffix in $checks.Keys){if("pcscope.shell.registrations.$suffix" -in $selection.disabled_items -and $path -match $checks[$suffix]){return}}
    }
    $key=$root.OpenSubKey($path,$false)
    if($null -eq $key){return}
    try{
        $values=[ordered]@{}
        foreach($name in @($key.GetValueNames() | Sort-Object)){
            if($name -match '^\{[0-9a-fA-F-]{36}\}$'){$script:ShellClsids[$name.ToLowerInvariant()]=$true}
            $value=$key.GetValue($name,$null,[Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            $kind=[string]$key.GetValueKind($name)
            $value=Protect-Value $name $value
            $values[$name]=[ordered]@{kind=$kind;value=$value}
            if($value -is [string] -and $value -match '^\{[0-9a-fA-F-]{36}\}$'){$script:ShellClsids[$value.ToLowerInvariant()]=$true}
        }
            $data=[ordered]@{registry_path=$path;values=$values;basis='registration only; not proof of loading or invocation';collection=@{policy='evidence-v2';remaining_depth=$depth;unvisited_subkeys=if($depth -eq 0){$key.SubKeyCount}else{0}}}
        if($path -match '(?i)\\InprocServer32$'){
            $serverPath=[string]$key.GetValue('',$null,[Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            if($serverPath){$data.implementation_file=Shell-FileMetadata $serverPath}
        }
        Item $path.ToLowerInvariant() $data
        if($depth -gt 0){foreach($child in @($key.GetSubKeyNames() | Sort-Object)){Read-RegistryTree $root ($path+'\'+$child) ($depth-1)}}
    }finally{$key.Dispose()}
}

foreach($hive in @('LocalMachine','CurrentUser')){
    if('pcscope.shell.registrations' -in $selection.disabled_sources){continue}
    foreach($view in @('Registry64','Registry32')){
        $root=$null
        try{
            $root=[Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::$hive,[Microsoft.Win32.RegistryView]::$view)
            $script:ShellClsids=@{}
            $shellItems=@(
                foreach($path in @(
                    'Software\Microsoft\Windows\CurrentVersion\PreviewHandlers',
                    'Software\Microsoft\Windows\CurrentVersion\Explorer\ShellIconOverlayIdentifiers',
                    'Software\Microsoft\Windows\CurrentVersion\Shell Extensions\Approved',
                    'Software\Microsoft\Windows\CurrentVersion\Shell Extensions\Blocked',
                    'Software\Microsoft\Windows\CurrentVersion\Explorer\Desktop\NameSpace',
                    'Software\Microsoft\Windows\CurrentVersion\Explorer\MyComputer\NameSpace'
                )){Read-RegistryTree $root $path 3}
                $classes=$root.OpenSubKey('Software\Classes',$false)
                if($null -ne $classes){
                    try{$types=@($classes.GetSubKeyNames() | Where-Object {$_ -notin @('CLSID','Interface','TypeLib','AppID')} | Sort-Object)}finally{$classes.Dispose()}
                    foreach($type in $types){
                        foreach($handler in @('shell','shellex','DefaultIcon')){Read-RegistryTree $root "Software\Classes\$type\$handler" 5}
                        if($type.StartsWith('.')){Read-RegistryTree $root "Software\Classes\$type" 0}
                    }
                    Read-RegistryTree $root 'Software\Classes\Directory\Background\shell' 5
                    Read-RegistryTree $root 'Software\Classes\Directory\Background\shellex' 5
                }
                foreach($clsid in @($script:ShellClsids.Keys | Sort-Object)){
                    Read-RegistryTree $root "Software\Classes\CLSID\$clsid" 0
                    Read-RegistryTree $root "Software\Classes\CLSID\$clsid\InprocServer32" 0
                    Read-RegistryTree $root "Software\Classes\CLSID\$clsid\LocalServer32" 0
                }
            )
            $unique=@{};foreach($entry in $shellItems){$unique[$entry.id]=$entry}
            $shards=@{};foreach($bucket in 0..15){$shards[$bucket]=[Collections.Generic.List[object]]::new()}
            foreach($entry in $unique.Values){$bucket=[Convert]::ToInt32((Hash-Text $entry.id).Substring(0,1),16);$shards[$bucket].Add($entry)}
            foreach($bucket in 0..15){Emit-Collection 'pcscope.shell.registrations' { $shards[$bucket] | Sort-Object id } "$hive/$view/shard:$bucket"}
        }catch{
            $failure=$_.Exception.Message
            Emit-Collection 'pcscope.shell.registrations' {throw $failure} "$hive/$view"
        }finally{if($null -ne $root){$root.Dispose()}}
    }
}

Emit-Collection 'pcscope.startup.logon' {
    foreach($hive in @('LocalMachine','CurrentUser')){foreach($view in @('Registry64','Registry32')){
        if('pcscope.startup.logon.registry' -in $selection.disabled_items){continue}
        $root=[Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::$hive,[Microsoft.Win32.RegistryView]::$view)
        try{
            foreach($suffix in @('Run','RunOnce')){
                $path="Software\Microsoft\Windows\CurrentVersion\$suffix";$key=$root.OpenSubKey($path,$false)
                if($null -ne $key){try{foreach($name in $key.GetValueNames()){Item "$hive/$view/$suffix/$name" ([ordered]@{hive=$hive;view=$view;registry_path=$path;name=$name;command=(Protect-Text ([string]$key.GetValue($name,$null,[Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)));command_collected=$true})}}finally{$key.Dispose()}}
            }
            foreach($item in @(Read-RegistryTree $root 'Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved' 1)){Item "$hive/$view/$($item.id)" $item.data}
        }finally{$root.Dispose()}
    }}
    foreach($folder in @([Environment]::GetFolderPath('Startup'),[Environment]::GetFolderPath('CommonStartup'))){
        if('pcscope.startup.logon.folders' -in $selection.disabled_items){continue}
        if($folder -and (Test-Path -LiteralPath $folder)){
            Get-ChildItem -LiteralPath $folder -File -Force | ForEach-Object {Item $_.FullName.ToLowerInvariant() ([ordered]@{path=$_.FullName;length=$_.Length;last_write_utc=$_.LastWriteTimeUtc.ToString('o');sha256=(Hash-File $_.FullName);shortcut_target_collected=$false})}
        }
    }
}
