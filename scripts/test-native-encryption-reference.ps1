# Test reference only: never call Get-BitLockerVolume, which internally reads recovery passwords.
$taskStructures=Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/Modules/BitLocker/Microsoft.BitLocker.Structures.dll'
[void][Reflection.Assembly]::LoadFrom($taskStructures)
function Read-EncryptionMetadata($volume,$method,$arguments=@{}) {
    if($method -notin @('GetLockStatus','GetEncryptionMethod','GetConversionStatus','GetProtectionStatus','GetKeyProtectors','GetKeyProtectorType')){throw 'Unsupported metadata operation'}
    $result=Invoke-CimMethod -InputObject $volume -MethodName $method -Arguments $arguments -ErrorAction Stop
    if($null -eq $result.ReturnValue -or $result.ReturnValue -ne 0){throw 'Encryption metadata query failed'}
    return $result
}
Emit 'security.encryption' {
    $volumes=@(Get-CimInstance -Namespace root/cimv2/Security/MicrosoftVolumeEncryption -ClassName Win32_EncryptableVolume -ErrorAction Stop)
    if($volumes.Count){$storage=@(Get-CimInstance -Namespace root/Microsoft/Windows/Storage -ClassName MSFT_Volume -ErrorAction Stop)}
    foreach($volume in $volumes) {
        $match=@($storage|Where-Object UniqueId -eq $volume.DeviceID);if($match.Count -ne 1){throw 'Missing matching volume metadata'}
        $mount=if($volume.DriveLetter){$volume.DriveLetter}else{$volume.DeviceID}
        $locked=(Read-EncryptionMetadata $volume GetLockStatus).LockStatus
        $method=(Read-EncryptionMetadata $volume GetEncryptionMethod).EncryptionMethod
        $status='';$percentage=$null
        if($locked -eq 0){
            $conversion=Read-EncryptionMetadata $volume GetConversionStatus
            $state=$conversion.ConversionStatus
            if($state -eq 1 -and $conversion.WipingStatus -eq 2){$state=6}
            elseif($state -eq 1 -and $conversion.WipingStatus -eq 3){$state=7}
            $status=[string][Microsoft.BitLocker.Structures.BitLockerVolumeStatus]$state
            $percentage=$conversion.EncryptionPercentage
        }
        $protection=(Read-EncryptionMetadata $volume GetProtectionStatus).ProtectionStatus
        $ids=(Read-EncryptionMetadata $volume GetKeyProtectors).VolumeKeyProtectorID
        $types=@(foreach($id in $ids){$type=(Read-EncryptionMetadata $volume GetKeyProtectorType @{VolumeKeyProtectorID=$id}).KeyProtectorType;[string][Microsoft.BitLocker.Structures.BitLockerVolumeKeyProtectorType]$type})
        Item $mount @{mount_point=$mount;volume_type=if([char]$match[0].DriveLetter -eq $env:SystemDrive[0]){'OperatingSystem'}else{'Data'};volume_status=$status;protection_status=[string][Microsoft.BitLocker.Structures.BitLockerVolumeProtectionStatus]$protection;encryption_method=[string][Microsoft.BitLocker.Structures.BitLockerVolumeEncryptionMethodOnGet]([int]$method);encryption_percentage=$percentage;lock_status=[string][Microsoft.BitLocker.Structures.BitLockerVolumeLockStatus]$locked;protector_types=@($types|Sort-Object);recovery_material_collected=$false}
    }
}
