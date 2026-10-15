Emit 'security.protection' {Get-MpComputerStatus | ForEach-Object {Item 'defender' (Fields $_ @('AMServiceEnabled','AntispywareEnabled','AntivirusEnabled','BehaviorMonitorEnabled','IoavProtectionEnabled','NISEnabled','OnAccessProtectionEnabled','RealTimeProtectionEnabled','IsTamperProtected','AMEngineVersion','AMProductVersion','AntivirusSignatureVersion','AntivirusSignatureLastUpdated','QuickScanEndTime','FullScanEndTime'))}}
Emit 'security.protection-policy' {Get-MpPreference | ForEach-Object {Item 'defender-policy' (Fields $_ @('DisableRealtimeMonitoring','DisableBehaviorMonitoring','DisableIOAVProtection','DisableScriptScanning','PUAProtection','ExclusionPath','ExclusionProcess','ExclusionExtension'))}}
Emit 'security.products' {Cim-Items AntiVirusProduct instanceGuid @('instanceGuid','displayName','productState','timestamp') 'root/SecurityCenter2'}
Emit 'security.detections' {Get-MpThreatDetection | ForEach-Object {Item ([string]$_.DetectionID) (Fields $_ @('DetectionID','ThreatID','InitialDetectionTime','LastThreatStatusChangeTime','ActionSuccess','CleaningActionID','ThreatStatusID','CurrentThreatExecutionStatusID'))}}
Emit 'security.deviceguard' {Cim-Items Win32_DeviceGuard InstanceIdentifier @('InstanceIdentifier','VirtualizationBasedSecurityStatus','SecurityServicesConfigured','SecurityServicesRunning','AvailableSecurityProperties','RequiredSecurityProperties','CodeIntegrityPolicyEnforcementStatus') 'root/Microsoft/Windows/DeviceGuard'}
Emit 'security.tpm' {Get-Tpm | ForEach-Object {Item 'tpm' (Fields $_ @('TpmPresent','TpmReady','TpmEnabled','TpmActivated','TpmOwned','RestartPending','ManufacturerIdTxt','ManufacturerVersion','LockedOut'))}}
Emit 'security.secureboot' {Item 'secureboot' @{enabled=(Confirm-SecureBootUEFI)}}
Emit 'security.key-metadata' {
    if(-not ('PCScopeMetadata' -as [type])){Add-Type -Path (Join-Path $PSScriptRoot 'MetadataNative.cs')}
    $script:keyProviders=@([PCScopeMetadata]::Providers())
    Item 'providers' @{providers=$script:keyProviders;basis='registered CNG key storage providers; no private key material'}
}
if($script:keyProviders){
    foreach($provider in $script:keyProviders){foreach($keyLocation in @('user','machine')){
        $scopeContext=Hash-Text "key-provider/$provider/$keyLocation"
        Emit 'security.key-metadata' {try{foreach($key in @([PCScopeMetadata]::Keys($provider,($keyLocation -eq 'machine')))){Item "$($key.Name)/$($key.LegacyKeySpec)" @{provider=$provider;location=$keyLocation;name=$key.Name;algorithm=$key.Algorithm;legacy_key_spec=$key.LegacyKeySpec;flags=$key.Flags;private_material_collected=$false;basis='NCryptEnumKeys with SILENT; key names and algorithm metadata only'}}}catch{throw "CNG provider '$provider' ($keyLocation): $($_.Exception.Message)"}}
    }}
    $scopeContext=''
}
foreach($location in @('LocalMachine','CurrentUser')){Emit "security.certificates.$($location.ToLowerInvariant())" {
    foreach($storeName in @('Root','CA','My','TrustedPublisher','Disallowed','TrustedPeople')){
        $store=[Security.Cryptography.X509Certificates.X509Store]::new($storeName,[Security.Cryptography.X509Certificates.StoreLocation]::$location)
        try{$store.Open([Security.Cryptography.X509Certificates.OpenFlags]::ReadOnly -bor [Security.Cryptography.X509Certificates.OpenFlags]::OpenExistingOnly);foreach($cert in $store.Certificates){Item "$storeName/$($cert.Thumbprint)" @{store=$storeName;location=$location;thumbprint=$cert.Thumbprint;subject=$cert.Subject;issuer=$cert.Issuer;not_before=(Iso $cert.NotBefore);not_after=(Iso $cert.NotAfter);has_private_key=$cert.HasPrivateKey;signature_algorithm=$cert.SignatureAlgorithm.Value;trust='not evaluated'}}}finally{$store.Close()}
    }
}}
Emit 'security.encryption' {Get-BitLockerVolume | ForEach-Object {Item $_.MountPoint @{mount_point=$_.MountPoint;volume_type=[string]$_.VolumeType;volume_status=[string]$_.VolumeStatus;protection_status=[string]$_.ProtectionStatus;encryption_method=[string]$_.EncryptionMethod;encryption_percentage=$_.EncryptionPercentage;lock_status=[string]$_.LockStatus;protector_types=@($_.KeyProtector|ForEach-Object {[string]$_.KeyProtectorType}|Sort-Object);recovery_material_collected=$false}}}
Emit 'policy.configuration' {Registry LocalMachine 'SOFTWARE\Microsoft\Windows\CurrentVersion\Policies' @('EnableLUA','ConsentPromptBehaviorAdmin','ConsentPromptBehaviorUser','PromptOnSecureDesktop','EnableInstallerDetection','FilterAdministratorToken') 3;Registry CurrentUser 'SOFTWARE\Microsoft\Windows\CurrentVersion\Policies' @() 3;Registry LocalMachine 'SOFTWARE\Policies\Microsoft\Windows\Safer' @() 3}
Emit 'security.ssh-public-keys' {
    $directory=Join-Path $env:USERPROFILE '.ssh';if(Test-Path -LiteralPath $directory){foreach($file in @(Get-ChildItem -LiteralPath $directory -Filter '*.pub' -File)){
        if($file.Length -gt 65536 -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Public key file outside safe read policy'}
        $line=[IO.File]::ReadAllText($file.FullName).Trim();$parts=@($line -split '\s+',3)
        if($parts.Count -lt 2 -or $parts[0] -notmatch '^(ssh-(rsa|ed25519)|ecdsa-sha2-nistp(256|384|521)|sk-ssh-ed25519@openssh.com|sk-ecdsa-sha2-nistp256@openssh.com)$'){throw 'Unsupported public-key encoding; file content not retained'}
        $sha=[Security.Cryptography.SHA256]::Create();try{$digest=$sha.ComputeHash([Convert]::FromBase64String($parts[1]));$fingerprint='SHA256:'+[Convert]::ToBase64String($digest).TrimEnd('=')}finally{$sha.Dispose()}
        Item $file.FullName @{path=$file.FullName;algorithm=$parts[0];fingerprint=$fingerprint;comment_collected=$false;key_material_collected=$false}
    }}
}
function Read-ChannelBoundary([string]$channel,[bool]$reverse){
    $query=[Diagnostics.Eventing.Reader.EventLogQuery]::new($channel,[Diagnostics.Eventing.Reader.PathType]::LogName)
    $query.ReverseDirection=$reverse
    $reader=[Diagnostics.Eventing.Reader.EventLogReader]::new($query)
    try{$event=$reader.ReadEvent();if($null -eq $event){return $null};try{@{record_id=$event.RecordId;time=(Iso $event.TimeCreated)}}finally{$event.Dispose()}}finally{$reader.Dispose()}
}
Emit 'collector.event-channel-coverage' {
    $channelPath=Join-Path $PSScriptRoot '../channels.json'
    if(-not (Test-Path -LiteralPath $channelPath)){$channelPath=Join-Path $PSScriptRoot '../../eventlog/channels.json'}
    foreach($channel in (Get-Content -Raw -LiteralPath $channelPath|ConvertFrom-Json)){
        try{
            $log=Get-WinEvent -ListLog $channel -ErrorAction Stop
            $oldest=Read-ChannelBoundary $channel $false;$newest=Read-ChannelBoundary $channel $true
            Item $channel @{channel=$channel;readable=$true;enabled=$log.IsEnabled;log_mode=[string]$log.LogMode;maximum_bytes=$log.MaximumSizeInBytes;record_count=$log.RecordCount;oldest_record=$oldest.record_id;oldest_time=$oldest.time;newest_record=$newest.record_id;newest_time=$newest.time;range_status='observed_retained_endpoints_only';basis='metadata plus actual read of retained endpoints; not proof of audit or historical completeness'}
        }catch{Item $channel @{channel=$channel;readable=$false;reason=$_.Exception.Message;basis='capability check; not an empty history'}}
    }
}
