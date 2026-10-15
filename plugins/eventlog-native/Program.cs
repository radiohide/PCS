using System;
using System.IO;
using System.Text;
using System.Linq;
using System.Xml;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics.Eventing.Reader;
using System.Security.Cryptography;
using System.Text.RegularExpressions;
using System.Web.Script.Serialization;

// Standalone Windows Event Log reader. No PowerShell engine or child process.
class Program {
 static JavaScriptSerializer J=new JavaScriptSerializer {MaxJsonLength=2097152,RecursionLimit=64};
 static Dictionary<string,object> D(params object[] a){var d=new Dictionary<string,object>();for(int i=0;i<a.Length;i+=2)d[(string)a[i]]=a[i+1];return d;}
 static object G(object d,string k){var m=d as Dictionary<string,object>;return m!=null&&m.ContainsKey(k)?m[k]:null;}
 static string S(object o){return o==null?"":Convert.ToString(o,System.Globalization.CultureInfo.InvariantCulture);}
 static long N(object o){return o==null?0:Convert.ToInt64(o);}
 static bool ItemEnabled(object request,EventRecord e){
  var rows=G(G(request,"selection"),"event_rules") as IEnumerable;if(rows==null)return true;
  foreach(var row in rows){var rule=G(row,"rule");if(G(rule,"status") is bool&&(bool)G(rule,"status"))continue;
   string provider=S(G(rule,"provider"));if(provider.Length>0&&provider!=e.ProviderName)continue;
   var ids=G(rule,"event_ids") as IEnumerable;if(ids!=null){bool match=false;foreach(var id in ids)if(N(id)==e.Id){match=true;break;}if(!match)continue;}
   return !(G(row,"enabled") is bool)|| (bool)G(row,"enabled");
  }return true;
 }
 static string Hash(string s){using(var h=SHA256.Create())return BitConverter.ToString(h.ComputeHash(Encoding.UTF8.GetBytes(s))).Replace("-","").ToLowerInvariant();}
 static object Protect(object v){var b=Encoding.UTF8.GetBytes(J.Serialize(v));try{return D("sensitive",true,"protection","dpapi-current-user-v1","ciphertext",BitConverter.ToString(ProtectedData.Protect(b,Encoding.UTF8.GetBytes("PCScope/evidence/v1"),DataProtectionScope.CurrentUser)).Replace("-","").ToLowerInvariant());}finally{Array.Clear(b,0,b.Length);}}
 static object Text(string s){return Regex.IsMatch(s,@"(?i)password|passwd|pwd\s*=|secret|token|api[_-]?key|authorization|private key|bearer\s|-(enc|encodedcommand)\s|https?://[^\s/]+@")?Protect(s):(object)s;}
 static object Value(string name,string s){return !Regex.IsMatch(name,@"(?i)(required|enabled|lastset|expires|age|length|history|policy|type|status)$")&&Regex.IsMatch(name,@"(?i)password|passwd|secret|credential|private.?key|recovery.?password|^(token|access.?token|refresh.?token|api.?key|authorization|ScriptBlockText|ScriptText|Script|Binary)$")?Protect(s):Text(s);}
 static string Kind(string channel,string provider,int id){
  if(channel=="Security"&&provider=="Microsoft-Windows-Security-Auditing"&&SecurityKinds.ContainsKey(id))return SecurityKinds[id];
  if(channel=="Security"&&id==1102&&provider=="Microsoft-Windows-Eventlog")return "coverage.security_log_cleared";
  switch(provider){case "MsiInstaller":return "software.installer_event";case "Service Control Manager":return "startup.service_event";case "Microsoft-Windows-Windows Defender":return "security.defender_event";case "Microsoft-Windows-PowerShell":return "execution.powershell_event";case "Microsoft-Windows-TaskScheduler":return "startup.task_event";case "Microsoft-Windows-WindowsUpdateClient":return "system.update_event";case "Microsoft-Windows-Sysmon":return "execution.sysmon_event";case "Microsoft-Windows-Kernel-Power":case "Microsoft-Windows-Power-Troubleshooter":case "Microsoft-Windows-Kernel-Boot":return "system.lifecycle_event";default:return "windows.event";}
 }
 static Dictionary<int,string> SecurityKinds=new Dictionary<int,string>{{4624,"identity.logon"},{4625,"identity.logon_failed"},{4634,"identity.logoff"},{4647,"identity.logoff_requested"},{4672,"identity.special_privileges"},{4688,"execution.process_started"},{4689,"execution.process_stopped"},{4720,"identity.account_created"},{4722,"identity.account_enabled"},{4725,"identity.account_disabled"},{4726,"identity.account_deleted"},{4738,"identity.account_changed"},{4740,"identity.account_locked"},{4767,"identity.account_unlocked"},{4781,"identity.account_renamed"},{4732,"identity.group_member_added"},{4733,"identity.group_member_removed"},{4800,"identity.session_locked"},{4801,"identity.session_unlocked"},{4778,"identity.session_reconnected"},{4779,"identity.session_disconnected"},{4657,"registry.value_operation"},{4663,"files.access_audit"},{4670,"files.permissions_audit"},{4697,"startup.service_installed"},{4698,"startup.task_created"},{4699,"startup.task_deleted"},{4702,"startup.task_updated"},{4719,"policy.audit_changed"},{5140,"network.share_access"},{5156,"network.connection_allowed"},{5157,"network.connection_blocked"}};
 static Dictionary<string,object> Project(EventRecord e,string xml,string channel,object watch){
  var doc=new XmlDocument {XmlResolver=null};using(var r=XmlReader.Create(new StringReader(xml),new XmlReaderSettings {DtdProcessing=DtdProcessing.Prohibit,XmlResolver=null}))doc.Load(r);
  var fields=D();var system=D();var issues=new List<object>();var suppressed=new List<string>();int index=0,used=0;
  foreach(XmlElement n in doc.SelectNodes("/*[local-name()='Event']/*[local-name()='EventData' or local-name()='UserData']//*[not(*)]")){
   index++;string name=n.GetAttribute("Name");if(name==""){name=n.LocalName;if(name=="Data")name="Data["+index+"]";else for(var p=n.ParentNode;p!=null&&p.LocalName!="UserData"&&p.LocalName!="EventData";p=p.ParentNode)name=p.LocalName+"/"+name;}
   string key=fields.ContainsKey(name)?name+"#"+index:name;object v=Value(n.HasAttribute("Name")?n.GetAttribute("Name"):n.LocalName,n.InnerText);
   if(v is Dictionary<string,object>){issues.Add(D("field",key,"reason","encrypted_sensitive_field"));if(n.InnerText!="")suppressed.Add(n.InnerText);}
   if(v is string){if(((string)v).Length>16384){issues.Add(D("field",key,"reason","preview_limited_full_value_retained","original_characters",((string)v).Length));fields[key]=D("preview",((string)v).Substring(0,16384),"full_value",Protect(v));continue;}used+=Encoding.UTF8.GetByteCount((string)v);}
   if(used>262144){issues.Add(D("field",key,"reason","payload_budget_exceeded","remaining_fields_not_read",true));break;}fields[key]=v;
  }
  foreach(XmlElement n in doc.SelectNodes("/*[local-name()='Event']/*[local-name()='System']/*")){var entry=D("value",n.InnerText);foreach(XmlAttribute a in n.Attributes)entry[a.Name]=a.Value;system[n.LocalName]=entry;}
  string kind=Kind(channel,e.ProviderName,e.Id);
  if(kind.StartsWith("files.")||(e.ProviderName=="Microsoft-Windows-Sysmon"&&new[]{2,11,15,23,26,27,28,29}.Contains(e.Id))){var p=G(fields,e.ProviderName=="Microsoft-Windows-Sysmon"?"TargetFilename":"ObjectName") as string;var selected=G(watch,"files") as IEnumerable;bool allowed=false;if(p!=null&&selected!=null)foreach(var f in selected)if(f is string&&String.Equals(p.Replace('/','\\'),((string)f).Replace('/','\\'),StringComparison.OrdinalIgnoreCase))allowed=true;if(!allowed)return null;}
  object message=null,error=null;string status="available";try{string m=e.FormatDescription();if(String.IsNullOrWhiteSpace(m))status="not_provided";else {message=suppressed.Any(s=>m.Contains(s))?Protect(m):Text(m);if(message is Dictionary<string,object>)status="protected";}}catch(Exception ex){status="unavailable";error=Text(ex.Message);}
  string time=e.TimeCreated.HasValue?e.TimeCreated.Value.ToUniversalTime().ToString("o"):DateTime.UtcNow.ToString("o");
  return D("id",S(e.RecordId),"occurred_at",time,"kind",kind,"object_id",channel+"/"+e.ProviderName+"/"+e.Id,"data",D("channel",channel,"record_id",e.RecordId,"provider",e.ProviderName,"event_id",e.Id,"level",e.Level,"user_sid",e.UserId==null?null:e.UserId.Value,"writer_process_id",e.ProcessId,"writer_thread_id",e.ThreadId,"xml_sha256",Hash(xml),"raw_xml",Protect(xml),"payload_collected",message!=null,"message",message,"message_status",status,"message_error",error,"fields",fields,"system",system,"collection",D("policy","evidence-v2","field_issues",issues,"raw_xml_stored",true,"raw_xml_reason","full_source_xml_encrypted_display_controlled_by_privacy_setting"),"field_schema","evidence-v2","basis","persisted Windows event evidence","source_time_available",e.TimeCreated.HasValue));
 }
 static string Creation(EventLogInformation i){return i.CreationTime.HasValue?i.CreationTime.Value.ToUniversalTime().ToString("o"):null;}
 static void Main(){Console.InputEncoding=Encoding.UTF8;Console.OutputEncoding=new UTF8Encoding(false);string source="pcscope.eventlog.unknown",observed=DateTime.UtcNow.ToString("o"),rid=Guid.NewGuid().ToString();try{
  var input=new StringBuilder();char[] buf=new char[4096];int read;while((read=Console.In.Read(buf,0,buf.Length))>0){input.Append(buf,0,read);if(input.Length>65536)throw new Exception("Request exceeds limit");}
  var req=J.DeserializeObject(input.ToString());string channel=S(G(req,"channel"));var channels=J.Deserialize<string[]>(File.ReadAllText(Path.Combine(AppDomain.CurrentDomain.BaseDirectory,"channels.json")));if(!channels.Contains(channel))throw new Exception("Unsupported channel");source="pcscope.eventlog."+channel.ToLowerInvariant();
  string path=channel;var type=PathType.LogName;FileInfo archive=null;long archiveLength=0,archiveTicks=0;if(G(req,"archive_path")!=null){archive=new FileInfo(S(G(req,"archive_path")));if(!archive.Exists||archive.Extension.ToLowerInvariant()!=".evtx"||archive.FullName.StartsWith(@"\\")||(archive.Attributes&FileAttributes.ReparsePoint)!=0)throw new Exception("Archive must be an ordinary local EVTX file");path=archive.FullName;type=PathType.FilePath;source="pcscope.eventlog.import."+Hash(path.ToLowerInvariant());archiveLength=archive.Length;archiveTicks=archive.LastWriteTimeUtc.Ticks;}
  var session=EventLogSession.GlobalSession;var info=session.GetLogInformation(path,type);string creation=Creation(info);var resume=G(req,"resume");var cursor=resume==null?null:J.DeserializeObject(S(G(resume,"checkpoint")));string epoch=resume==null?Guid.NewGuid().ToString():S(G(resume,"epoch"));long sequence=resume==null?1:N(G(resume,"batch_seq"))+1;
  string reason=cursor!=null&&S(G(cursor,"creation"))!=S(creation)?"Log creation identity changed":null;long last=cursor!=null&&reason==null?N(G(cursor,"record_id")):0;var events=new List<object>();string fingerprint=S(G(cursor,"fingerprint")),bookmark=S(G(cursor,"bookmark"));int count=0,bytes=0;bool more=false;
  EventLogReader reader=new EventLogReader(new EventLogQuery(path,type,last>0?"*[System[EventRecordID >= "+last+"]]":"*"));try{
   if(last>0)using(var anchor=reader.ReadEvent()){if(anchor==null||anchor.RecordId!=last||Hash(anchor.ToXml())!=fingerprint)reason="Checkpoint event is unavailable or changed; retained source history may have been overwritten or cleared";}
   if(reason!=null){reader.Dispose();reader=new EventLogReader(new EventLogQuery(path,type,"*"));epoch=Guid.NewGuid().ToString();sequence=1;last=0;fingerprint=null;bookmark=null;events.Add(D("id","gap-"+epoch,"occurred_at",observed,"kind","coverage.log_reset","object_id",channel,"data",D("reason",reason,"oldest_available_record",info.OldestRecordNumber,"basis","source checkpoint validation")));}
   while(count<128){using(var e=reader.ReadEvent()){if(e==null)break;string xml=e.ToXml();var item=ItemEnabled(req,e)?Project(e,xml,archive==null?channel:e.LogName,G(req,"watch")):null;if(item!=null){int size=Encoding.UTF8.GetByteCount(J.Serialize(item))+1;if(size>950000)throw new Exception("Single event exceeds protocol budget; checkpoint retained");if(bytes+size>950000){more=true;break;}events.Add(item);bytes+=size;}last=e.RecordId.Value;fingerprint=Hash(xml);bookmark=null;count++;}}
  }finally{reader.Dispose();}
  var after=session.GetLogInformation(path,type);if(Creation(after)!=creation)throw new Exception("Log changed identity during read; batch discarded");if(archive!=null){archive.Refresh();if(archive.Length!=archiveLength||archive.LastWriteTimeUtc.Ticks!=archiveTicks)throw new Exception("Archive changed during read; batch discarded");}
  string checkpoint=J.Serialize(D("creation",creation,"record_id",last,"fingerprint",fingerprint,"bookmark",bookmark,"oldest_available_record",after.OldestRecordNumber,"has_more",count==128||more));
  string output=J.Serialize(D("protocol_version",1,"request_id",rid,"type","event_batch","source",source,"epoch",epoch,"batch_seq",sequence,"checkpoint",checkpoint,"events",events));if(Encoding.UTF8.GetByteCount(output)+1>1048576)throw new Exception("Encoded event batch exceeds protocol limit; checkpoint retained");Console.WriteLine(output);
 }catch(Exception ex){Console.WriteLine(J.Serialize(D("protocol_version",1,"request_id",rid,"type","gap","source",source,"observed_at",observed,"reason","Event log read failed; checkpoint unchanged: "+ex.Message)));}}
}
