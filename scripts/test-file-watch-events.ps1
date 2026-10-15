$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot '../plugins/eventlog/event-schema.ps1')
$watch=@{files=@('C:\watched\file.txt')}
$projection=@{kind='files.access_audit';fields=@{ObjectName='C:\watched\file.txt'}}
if(Allow-FileWatchEvent $projection 'Microsoft-Windows-Security-Auditing' 4663 @{files=@()}){throw 'Empty list leaked file audit'}
if(-not (Allow-FileWatchEvent $projection 'Microsoft-Windows-Security-Auditing' 4663 $watch)){throw 'Selected path was omitted'}
$projection.fields.ObjectName='C:\other\file.txt'
if(Allow-FileWatchEvent $projection 'Microsoft-Windows-Security-Auditing' 4663 $watch){throw 'Same filename outside list leaked'}
$projection=@{kind='execution.sysmon_event';fields=@{TargetFilename='C:\other\file.txt'}}
if(Allow-FileWatchEvent $projection 'Microsoft-Windows-Sysmon' 11 $watch){throw 'Sysmon file event leaked'}
if(-not (Allow-FileWatchEvent $projection 'Microsoft-Windows-Sysmon' 1 @{files=@()})){throw 'Non-file process event incorrectly suppressed'}
'PASS: empty/selected/other directory and Sysmon file events; non-file event preserved'
