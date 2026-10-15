$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot '../plugins/eventlog/event-schema.ps1')
function Reveal-Fixture($protected){$hex=$protected.ciphertext;$bytes=New-Object byte[] ($hex.Length/2);for($i=0;$i -lt $bytes.Length;$i++){$bytes[$i]=[Convert]::ToByte($hex.Substring($i*2,2),16)};$plain=[Security.Cryptography.ProtectedData]::Unprotect($bytes,[Text.Encoding]::UTF8.GetBytes('PCScope/evidence/v1'),[Security.Cryptography.DataProtectionScope]::CurrentUser);try{ConvertFrom-Json ([Text.Encoding]::UTF8.GetString($plain))}finally{[Array]::Clear($plain,0,$plain.Length)}}
$xml='<Event xmlns="http://schemas.microsoft.com/win/2004/08/events/event"><EventData><Data Name="TargetUserSid">S-1-5-21-test</Data><Data Name="TargetLogonId">0x123</Data><Data Name="CommandLine">SECRET-COMMAND</Data><Data Name="ScriptBlockText">SECRET-SCRIPT</Data><Data Name="Password">SECRET-PASSWORD</Data></EventData></Event>'
$projection=Event-Projection Security Microsoft-Windows-Security-Auditing 4720 $xml
if($projection.kind -ne 'identity.account_created' -or $projection.fields.TargetUserSid -ne 'S-1-5-21-test'){throw 'Account projection failed'}
if((ConvertTo-Json $projection.fields -Depth 5) -match 'SECRET-(SCRIPT|PASSWORD)'){throw 'Sensitive field leaked'}
if($projection.fields.CommandLine -ne 'SECRET-COMMAND'){throw 'Command evidence discarded'}
foreach($entry in @{4624='identity.logon';4634='identity.logoff';4647='identity.logoff_requested';4688='execution.process_started';4800='identity.session_locked'}.GetEnumerator()){
    if((Event-Projection Security Microsoft-Windows-Security-Auditing $entry.Key $xml).kind -ne $entry.Value){throw 'Event classification failed'}
}
if((Event-Projection Application UnknownProvider 4720 $xml).kind -ne 'windows.event'){throw 'Provider collision misclassified'}
$rejected=$false;try{Event-Projection System Unknown 1 '<!DOCTYPE foo [<!ENTITY xxe SYSTEM "file:///C:/Windows/win.ini">]><Event>&xxe;</Event>'|Out-Null}catch{$rejected=$true}
if(-not $rejected){throw 'External entity was not rejected'}
if((Reveal-Fixture $projection.fields.Password) -ne 'SECRET-PASSWORD' -or (Reveal-Fixture $projection.fields.ScriptBlockText) -ne 'SECRET-SCRIPT'){throw 'Sensitive originals were not retained'}
'PASS: account/session/process mappings, provider isolation, encrypted originals, XML DTD rejection'
$extra=Event-Projection Application incredibuild 88 '<Event><System><Version>2</Version><Correlation ActivityID="abc"/></System><EventData><Data>unnamed</Data><Data Name="Custom">a</Data><Data Name="Custom">b</Data><Data Name="CommandLine">app --password="hidden value" --safe=yes</Data></EventData><UserData><Custom><Detail>readable</Detail></Custom></UserData></Event>'
if($extra.fields.'Data[1]' -ne 'unnamed' -or $extra.fields.'Custom#3' -ne 'b' -or $extra.fields.'Custom/Detail' -ne 'readable'){throw 'Unknown, duplicate, unnamed or UserData field lost'}
if($extra.system.Version.value -ne '2' -or $extra.system.Correlation.ActivityID -ne 'abc'){throw 'System attributes lost'}
if((Reveal-Fixture $extra.fields.CommandLine) -ne 'app --password="hidden value" --safe=yes'){throw 'Original command was not retained'}
$large=Event-Projection Application Unknown 1 ('<Event><EventData><Data Name="Large">'+('x'*20000)+'</Data></EventData></Event>')
if($large.fields.Large.preview.Length -ne 16384 -or (Reveal-Fixture $large.fields.Large.full_value).Length -ne 20000){throw 'Full oversized value not retained'}
'PASS: arbitrary/unnamed/duplicate/UserData fields, System attributes, encrypted command and complete oversized value'
foreach($text in @('Authorization: Bearer SECRET-HEADER','Authorization: Basic SECRET-HEADER','powershell -EncodedCommand SECRET-HEADER','https://user:SECRET-HEADER@example.com')){$protected=Protect-Text $text;if((ConvertTo-Json $protected) -match 'SECRET-HEADER'){throw 'Header/encoded/URL credential leak'};if((Reveal-Fixture $protected) -cne $text){throw 'Original text lost'}}
if((Protect-Value PasswordRequired $true) -ne $true){throw 'Boolean password policy was redacted'}
'PASS: authorization schemes, encoded command, credential URLs, nonsecret password policy'
