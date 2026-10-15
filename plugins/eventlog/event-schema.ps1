$policy=Join-Path $PSScriptRoot 'evidence.ps1'
if(-not (Test-Path -LiteralPath $policy)){$policy=Join-Path $PSScriptRoot '../powershell-adapter/evidence.ps1'}
. $policy
function Allow-FileWatchEvent($projection,[string]$provider,[int]$eventId,$watch){
    $fileEvent=$projection.kind.StartsWith('files.') -or ($provider -eq 'Microsoft-Windows-Sysmon' -and $eventId -in @(2,11,15,23,26,27,28,29))
    if(-not $fileEvent){return $true}
    $path=$projection.fields.ObjectName
    if($provider -eq 'Microsoft-Windows-Sysmon'){$path=$projection.fields.TargetFilename}
    if($path -isnot [string]){return $false}
    foreach($selected in @($watch.files)){if($selected -is [string] -and [string]::Equals($path.Replace('/','\'),$selected.Replace('/','\'),[StringComparison]::OrdinalIgnoreCase)){return $true}}
    return $false
}
function Event-Projection([string]$channel,[string]$provider,[int]$eventId,[string]$xmlText){
    $kind='windows.event';$facts=[ordered]@{}
    # Provider-qualified classification; all payload leaves are preserved unless explicitly protected.
    $allowed=@('SubjectUserSid','SubjectUserName','SubjectDomainName','SubjectLogonId','TargetUserSid','TargetUserName','TargetDomainName','TargetLogonId','LogonType','LogonId','SessionId','SessionID','ProcessId','ProcessID','NewProcessId','ParentProcessName','NewProcessName','ProcessName','TokenElevationType','MandatoryLabel','Status','SubStatus','FailureReason','IpAddress','IpPort','WorkstationName','AuthenticationPackageName','ElevatedToken','MemberSid','MemberName','GroupName','ServiceName','ServiceType','StartType','AccountName','TaskName','ResultCode','ErrorCode','DeviceInstanceId','DriverName','Image','ImageLoaded','ProcessGuid','ParentProcessGuid','ParentProcessId','SourceIp','DestinationIp','SourcePort','DestinationPort','Protocol','RuleName','TargetFilename','EventType','ProductName','ProductVersion','UpdateTitle')
    if($channel -eq 'Security' -and $provider -eq 'Microsoft-Windows-Security-Auditing'){
        $map=@{4624='identity.logon';4625='identity.logon_failed';4634='identity.logoff';4647='identity.logoff_requested';4672='identity.special_privileges';4688='execution.process_started';4689='execution.process_stopped';4720='identity.account_created';4722='identity.account_enabled';4725='identity.account_disabled';4726='identity.account_deleted';4738='identity.account_changed';4740='identity.account_locked';4767='identity.account_unlocked';4781='identity.account_renamed';4732='identity.group_member_added';4733='identity.group_member_removed';4800='identity.session_locked';4801='identity.session_unlocked';4778='identity.session_reconnected';4779='identity.session_disconnected';4657='registry.value_operation';4663='files.access_audit';4670='files.permissions_audit';4697='startup.service_installed';4698='startup.task_created';4699='startup.task_deleted';4702='startup.task_updated';4719='policy.audit_changed';5140='network.share_access';5156='network.connection_allowed';5157='network.connection_blocked'}
        if($map.ContainsKey($eventId)){$kind=$map[$eventId]}
    }
    if($channel -eq 'Security' -and $eventId -eq 1102 -and $provider -eq 'Microsoft-Windows-Eventlog'){$kind='coverage.security_log_cleared'}
    if($provider -eq 'MsiInstaller'){$kind='software.installer_event'}
    if($provider -eq 'Service Control Manager'){$kind='startup.service_event'}
    if($provider -eq 'Microsoft-Windows-Windows Defender'){$kind='security.defender_event'}
    if($provider -eq 'Microsoft-Windows-PowerShell'){$kind='execution.powershell_event'}
    if($provider -eq 'Microsoft-Windows-TaskScheduler'){$kind='startup.task_event'}
    if($provider -eq 'Microsoft-Windows-WindowsUpdateClient'){$kind='system.update_event'}
    if($provider -eq 'Microsoft-Windows-Sysmon'){$kind='execution.sysmon_event'}
    if($provider -in @('Microsoft-Windows-Kernel-Power','Microsoft-Windows-Power-Troubleshooter','Microsoft-Windows-Kernel-Boot')){$kind='system.lifecycle_event'}
    $settings=[Xml.XmlReaderSettings]::new();$settings.DtdProcessing=[Xml.DtdProcessing]::Prohibit;$settings.XmlResolver=$null
    $reader=[Xml.XmlReader]::Create([IO.StringReader]::new($xmlText),$settings)
    $issues=[Collections.Generic.List[object]]::new();$suppressed=[Collections.Generic.List[string]]::new();$used=0;$index=0
    try{
        $doc=[Xml.XmlDocument]::new();$doc.XmlResolver=$null;$doc.Load($reader)
        foreach($node in $doc.SelectNodes("/*[local-name()='Event']/*[local-name()='EventData' or local-name()='UserData']//*[not(*)]")){
            $index++;$name=$node.GetAttribute('Name');if(-not $name){$name=$node.LocalName;if($name -eq 'Data'){$name="Data[$index]"}else{$parent=$node.ParentNode;while($parent -and $parent.LocalName -notin @('UserData','EventData')){$name=$parent.LocalName+'/'+$name;$parent=$parent.ParentNode}}}
            $key=$name;if($facts.Contains($key)){$key="$name#$index"}
            $value=Protect-Value $node.LocalName $node.InnerText
            if($node.GetAttribute('Name')){$value=Protect-Value $node.GetAttribute('Name') $node.InnerText}
            if($value -is [System.Collections.IDictionary] -and $value.sensitive){$issues.Add(@{field=$key;reason='encrypted_sensitive_field'});if($node.InnerText){$suppressed.Add($node.InnerText)}}
            if($value -is [string]){
                if($value -ne $node.InnerText){$issues.Add(@{field=$key;reason='secret_redacted'})}
                if($value.Length -gt 16384){$issues.Add(@{field=$key;reason='preview_limited_full_value_retained';original_characters=$value.Length});$facts[$key]=@{preview=$value.Substring(0,16384);full_value=(Protect-Sensitive $value)};continue}
                $used+=[Text.Encoding]::UTF8.GetByteCount($value)
            }
            if($used -gt 262144){$issues.Add(@{field=$key;reason='payload_budget_exceeded';remaining_fields_not_read=$true});break}
            $facts[$key]=$value
        }
        $system=[ordered]@{}
        foreach($node in $doc.SelectNodes("/*[local-name()='Event']/*[local-name()='System']/*")){
            $entry=[ordered]@{value=$node.InnerText};foreach($attr in $node.Attributes){$entry[$attr.Name]=$attr.Value};$system[$node.LocalName]=$entry
        }
    }finally{$reader.Dispose()}
    return @{kind=$kind;fields=$facts;system=$system;issues=$issues.ToArray();suppressed=$suppressed.ToArray();schema='evidence-v2'}
}
