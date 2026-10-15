param([Parameter(Mandatory=$true)][string]$OutputDirectory,[Parameter(Mandatory=$true)][string]$ExpectedSid,[Parameter(Mandatory=$true)][string]$ProjectRoot)
$ErrorActionPreference='Stop'
$taskIdentity=[Security.Principal.WindowsIdentity]::GetCurrent()
if($taskIdentity.User.Value -ne $ExpectedSid -or -not ([Security.Principal.WindowsPrincipal]::new($taskIdentity)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Reference requires authorized original-account elevation'}
$taskOutput=(Resolve-Path -LiteralPath $OutputDirectory).Path
for($taskDirectory=[IO.DirectoryInfo]::new($taskOutput);$taskDirectory;$taskDirectory=$taskDirectory.Parent){if($taskDirectory.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Reference output must not pass through reparse points'}}
$env:TEMP=$taskOutput;$env:TMP=$taskOutput
$taskResults=[ordered]@{}
function Item([string]$id,$data){if([string]::IsNullOrWhiteSpace($id)){throw 'Missing reference identity'};[pscustomobject]@{id=$id;data=$data}}
function Read-Reference([string]$source,[scriptblock]$read){try{$taskResults[$source]=@{available=$true;items=@(& $read)}}catch{$taskResults[$source]=@{available=$false;error=$_.Exception.Message}}}
Read-Reference 'pcscope.software.optional-features' {
    Get-WindowsOptionalFeature -Online -LogPath (Join-Path $taskOutput 'features.log') -ScratchDirectory $taskOutput | ForEach-Object {Item $_.FeatureName @{name=$_.FeatureName;state=[string]$_.State}}
}
Read-Reference 'pcscope.software.provisioned' {
    Get-AppxProvisionedPackage -Online -LogPath (Join-Path $taskOutput 'provisioned.log') -ScratchDirectory $taskOutput | ForEach-Object {Item $_.PackageName @{DisplayName=$_.DisplayName;Version=[string]$_.Version;Architecture=[int]$_.Architecture;PackageName=$_.PackageName;InstallLocation=$_.InstallLocation}}
}
Read-Reference 'pcscope.security.secureboot' {Item 'secureboot' @{enabled=(Confirm-SecureBootUEFI)}}
# This fixed reference reads metadata only; Get-BitLockerVolume would also read recovery secrets.
function Emit($source,[scriptblock]$read){if($source -ne 'security.encryption'){throw 'Unsupported encryption reference source'};Read-Reference ('pcscope.'+$source) $read}
$taskEncryptionReference=Join-Path $ProjectRoot 'scripts/test-native-encryption-reference.ps1'
& ([scriptblock]::Create([IO.File]::ReadAllText($taskEncryptionReference)))
Read-Reference 'pcscope.security.tpm' {
    # Bypass both constructors. Invoke only audited metadata getters on the original bridge;
    # never instantiate TpmObject or invoke Get-Tpm, OwnerAuth, SelfTest, Enable/Disable/Clear.
    $taskType=(Get-Command Get-Tpm).ImplementingType.Assembly.GetType('Microsoft.Tpm.Commands.Win32_Tpm')
    $taskObject=[Runtime.Serialization.FormatterServices]::GetUninitializedObject($taskType)
    $taskFlags=[Reflection.BindingFlags]'Public,NonPublic,Instance'
    $taskFields=@{TpmPresent=(@(Get-CimInstance -Namespace root/cimv2/Security/MicrosoftTpm -ClassName Win32_Tpm -OperationTimeoutSec 15).Count -eq 1)}
    foreach($taskPair in @(@('TpmReady','IsReady'),@('TpmEnabled','IsEnabled'),@('TpmActivated','IsActivated'),@('TpmOwned','IsOwned'),@('RestartPending','HasPendingPpiCode'),@('ManufacturerIdTxt','get_ManufacturerIdTxt'),@('ManufacturerVersion','get_ManufacturerVersion'))){$taskFields[$taskPair[0]]=$taskType.GetMethod($taskPair[1],$taskFlags).Invoke($taskObject,@())}
    # LockedOut is deliberately excluded here: the legacy constructor detects it via failing
    # authentication/self-test. Native unauthenticated capability framing has separate tests.
    Item 'tpm' $taskFields
}
try{
    $taskManager=New-Object 'Windows.Management.Deployment.PackageManager,Windows.Management.Deployment,ContentType=WindowsRuntime'
    $taskPackages=@($taskManager.FindProvisionedPackages())
    $taskResults['provisioned_winrt_probe']=@{available=$true;count=$taskPackages.Count;packages=@($taskPackages | ForEach-Object {@{PackageName=$_.Id.FullName;Name=$_.Id.Name;Version=[string]$_.Id.Version;Architecture=[int]$_.Id.Architecture;InstallLocation=if($_.InstalledLocation){$_.InstalledLocation.Path}else{$null}}})}
}catch{$taskResults['provisioned_winrt_probe']=@{available=$false;error=$_.Exception.Message}}
$taskReport=@{sources=$taskResults;original_account=$true;elevated=$true;system_configuration_changes=$false;owner_auth_read=$false;recovery_secrets_read=$false;self_test_run=$false;tpm_reference_excluded_fields=@('LockedOut')}
$taskJson=$taskReport | ConvertTo-Json -Depth 18
$taskFile=[IO.File]::Open((Join-Path $taskOutput 'reference.json'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
try{$taskBytes=[Text.UTF8Encoding]::new($false).GetBytes($taskJson);$taskFile.Write($taskBytes,0,$taskBytes.Length)}finally{$taskFile.Dispose()}
