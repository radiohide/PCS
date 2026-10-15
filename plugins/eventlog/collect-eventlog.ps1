$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$request=ConvertFrom-Json ([Console]::In.ReadToEnd())
$channels=Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot 'channels.json')|ConvertFrom-Json
if($request.channel -cnotin $channels){throw 'Unsupported channel'}
. (Join-Path $PSScriptRoot 'event-schema.ps1')
$channel=[string]$request.channel
$source='pcscope.eventlog.'+$channel.ToLowerInvariant()
$observed=[DateTime]::UtcNow.ToString('o')
$requestId=[Guid]::NewGuid().ToString()
$reader=$null
function Fingerprint([string]$text){$sha=[Security.Cryptography.SHA256]::Create();try{([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($text)))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}}
$logPath=$channel;$pathType=[Diagnostics.Eventing.Reader.PathType]::LogName;$archiveStamp=$null
if($request.archive_path){
    $archive=Get-Item -LiteralPath ([string]$request.archive_path)
    if($archive.PSIsContainer -or $archive.Extension -ine '.evtx' -or $archive.FullName.StartsWith('\\') -or ($archive.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Archive must be an ordinary local EVTX file'}
    $logPath=$archive.FullName;$pathType=[Diagnostics.Eventing.Reader.PathType]::FilePath
    $archiveStamp="$($archive.Length)/$($archive.LastWriteTimeUtc.Ticks)"
    $source='pcscope.eventlog.import.'+(Fingerprint $archive.FullName.ToLowerInvariant())
}
try{
    $session=[Diagnostics.Eventing.Reader.EventLogSession]::GlobalSession
    # Disabled channels may still contain useful history. Reading does not enable logging.
    $info=$session.GetLogInformation($logPath,$pathType)
    $creation=if($null -ne $info.CreationTime){$info.CreationTime.ToUniversalTime().ToString('o')}else{$null}
    $resume=$request.resume
    $cursor=if($null -ne $resume){ConvertFrom-Json $resume.checkpoint}else{$null}
    $epoch=if($null -ne $resume){[string]$resume.epoch}else{[Guid]::NewGuid().ToString()}
    $sequence=if($null -ne $resume){[long]$resume.batch_seq+1}else{1}
    $events=[Collections.Generic.List[object]]::new()
    $resetReason=$null
    if($null -ne $cursor -and $cursor.creation -ne $creation){$resetReason='Log creation identity changed'}
    $lastId=if($null -ne $cursor -and -not $resetReason){[long]$cursor.record_id}else{0}
    $queryText=if($lastId -gt 0){"*[System[EventRecordID >= $lastId]]"}else{'*'}
    $query=[Diagnostics.Eventing.Reader.EventLogQuery]::new($logPath,$pathType,$queryText)
    $reader=[Diagnostics.Eventing.Reader.EventLogReader]::new($query)
    if($lastId -gt 0){
        $anchor=$reader.ReadEvent()
        try{if($null -eq $anchor -or $anchor.RecordId -ne $lastId -or (Fingerprint $anchor.ToXml()) -ne $cursor.fingerprint){$resetReason='Checkpoint event is unavailable or changed; retained source history may have been overwritten or cleared'}}finally{if($null -ne $anchor){$anchor.Dispose()}}
    }
    if($resetReason){
        $reader.Dispose();$reader=[Diagnostics.Eventing.Reader.EventLogReader]::new([Diagnostics.Eventing.Reader.EventLogQuery]::new($logPath,$pathType,'*'))
        $epoch=[Guid]::NewGuid().ToString();$sequence=1;$lastId=0;$cursor=$null
        $events.Add([ordered]@{id='gap-'+$epoch;occurred_at=$observed;kind='coverage.log_reset';object_id=$channel;data=[ordered]@{reason=$resetReason;oldest_available_record=$info.OldestRecordNumber;basis='source checkpoint validation'}})
    }
    $fingerprint=if($null -ne $cursor){$cursor.fingerprint}else{$null}
    $bookmark=if($null -ne $cursor){$cursor.bookmark}else{$null}
    $count=0;$batchBytes=0;$hasMore=$false
    while($count -lt 128){
        $event=$reader.ReadEvent()
        if($null -eq $event){break}
        try{
            if($request.archive_path){$channel=[string]$event.LogName}
            $xml=$event.ToXml();$nextFingerprint=Fingerprint $xml
            $enabled=$true
            foreach($entry in @($request.selection.event_rules)){
                if($entry.rule.status){continue}
                if($entry.rule.provider -and $entry.rule.provider -cne $event.ProviderName){continue}
                if($entry.rule.event_ids -and $event.Id -notin $entry.rule.event_ids){continue}
                $enabled=$entry.enabled;break
            }
            if($enabled -eq $false){$lastId=[long]$event.RecordId;$bookmark=$event.Bookmark.BookmarkXml;$fingerprint=$nextFingerprint;$count++;continue}
            $time=if($null -ne $event.TimeCreated){$event.TimeCreated.ToUniversalTime().ToString('o')}else{$observed}
            $projection=Event-Projection $channel $event.ProviderName $event.Id $xml
            if(-not (Allow-FileWatchEvent $projection $event.ProviderName $event.Id $request.watch)){
                # Scope filtering still consumes source records; otherwise a filtered event would stall the cursor.
                $lastId=[long]$event.RecordId;$bookmark=$event.Bookmark.BookmarkXml;$fingerprint=$nextFingerprint;$count++;continue
            }
            $message=$null;$messageStatus='available';$messageError=$null
            try{
                $message=$event.FormatDescription()
                if([string]::IsNullOrWhiteSpace($message)){$messageStatus='not_provided'}else{
                    $containsSensitive=$false;foreach($secret in $projection.suppressed){if($message.Contains($secret)){$containsSensitive=$true;break}}
                    $message=if($containsSensitive){Protect-Sensitive $message}else{Protect-Text $message}
                    if($message -is [System.Collections.IDictionary]){$messageStatus='protected'}
                }
            }catch{if($message){throw};$messageStatus='unavailable';$messageError=Protect-Text $_.Exception.Message}
            $candidate=[ordered]@{id=[string]$event.RecordId;occurred_at=$time;kind=$projection.kind;object_id="$channel/$($event.ProviderName)/$($event.Id)";data=[ordered]@{channel=$channel;record_id=[long]$event.RecordId;provider=$event.ProviderName;event_id=$event.Id;level=$event.Level;user_sid=if($event.UserId){$event.UserId.Value}else{$null};writer_process_id=$event.ProcessId;writer_thread_id=$event.ThreadId;xml_sha256=$nextFingerprint;raw_xml=(Protect-Sensitive $xml);payload_collected=(-not [string]::IsNullOrWhiteSpace($message));message=$message;message_status=$messageStatus;message_error=$messageError;fields=$projection.fields;system=$projection.system;collection=@{policy='evidence-v2';field_issues=@($projection.issues);raw_xml_stored=$true;raw_xml_reason='full_source_xml_encrypted_display_controlled_by_privacy_setting'};field_schema=$projection.schema;basis='persisted Windows event evidence';source_time_available=($null -ne $event.TimeCreated)}}
            $candidateBytes=[Text.Encoding]::UTF8.GetByteCount((ConvertTo-Json -InputObject $candidate -Depth 18 -Compress))+1
            if($candidateBytes -gt 950000){throw 'Single event exceeds protocol budget; checkpoint retained'}
            if($batchBytes+$candidateBytes -gt 950000){$hasMore=$true;break}
            $events.Add($candidate);$batchBytes+=$candidateBytes
            # Advance only after the event fits the output; the next batch re-reads a deferred event.
            $lastId=[long]$event.RecordId;$bookmark=$event.Bookmark.BookmarkXml;$fingerprint=$nextFingerprint
            $count++
        }finally{$event.Dispose()}
    }
    $after=$session.GetLogInformation($logPath,$pathType)
    if($request.archive_path){$archive.Refresh();if($archiveStamp -ne "$($archive.Length)/$($archive.LastWriteTimeUtc.Ticks)"){throw 'Archive changed during read; batch discarded'}}
    $afterCreation=if($null -ne $after.CreationTime){$after.CreationTime.ToUniversalTime().ToString('o')}else{$null}
    if($afterCreation -ne $creation){throw 'Log changed identity during read; batch discarded'}
    $checkpoint=ConvertTo-Json -InputObject ([ordered]@{creation=$creation;record_id=$lastId;fingerprint=$fingerprint;bookmark=$bookmark;oldest_available_record=$after.OldestRecordNumber;has_more=($count -eq 128 -or $hasMore)}) -Compress
    $line=ConvertTo-Json -InputObject ([ordered]@{protocol_version=1;request_id=$requestId;type='event_batch';source=$source;epoch=$epoch;batch_seq=$sequence;checkpoint=$checkpoint;events=$events.ToArray()}) -Depth 18 -Compress
    if([Text.Encoding]::UTF8.GetByteCount($line)+1 -gt 1048576){throw 'Encoded event batch exceeds protocol limit; checkpoint retained'}
    [Console]::WriteLine($line)
}catch{
    [Console]::WriteLine((ConvertTo-Json -InputObject ([ordered]@{protocol_version=1;request_id=$requestId;type='gap';source=$source;observed_at=$observed;reason="Event log read failed; checkpoint unchanged: $($_.Exception.Message)"}) -Depth 5 -Compress))
}finally{if($null -ne $reader){$reader.Dispose()}}
