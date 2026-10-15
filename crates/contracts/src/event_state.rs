//! Explicit Windows events that can supply current fields.
//!
//! This module is pure: it neither looks up an object nor reads the system. The storage layer
//! must resolve the declared identity and arbitrate effective time, stream ordering and replay.
//! Import time is never an effective time. Unknown versions remain events without a state write.
use crate::adapter::{ActionCandidate, ActionTarget, AdapterContext, Derivation, EvidenceInput, EvidenceKind, FieldCandidate, FieldKind, IdentityCandidate, ValueStatus};
use serde_json::Value;

const SECURITY: &str = "Microsoft-Windows-Security-Auditing";
const TASKS: &str = "Microsoft-Windows-TaskScheduler";
const TASK_CHANNEL: &str = "Microsoft-Windows-TaskScheduler/Operational";
const LSM: &str = "Microsoft-Windows-TerminalServices-LocalSessionManager";
const LSM_CHANNEL: &str = "Microsoft-Windows-TerminalServices-LocalSessionManager/Operational";

/// Fields already declared by the type catalog. Their source_time equals effective_time.
#[derive(Debug, Clone, PartialEq)]
pub struct EventStateIntent {
    pub fields: Vec<FieldCandidate>,
    pub effective_time: String,
    pub rule_id: &'static str,
}

/// SCM 7036 names a display name, which is NOT the service's stable Name identity.
/// Storage may apply `intent` only after finding exactly one existing service with this display
/// name in the same machine/installation/business scope. No match or several matches is an
/// unassigned event, never permission to create a service keyed by its display name.
#[derive(Debug, Clone, PartialEq)]
pub struct ServiceStateMatch {
    pub display_name: String,
    pub state: String,
    pub intent: EventStateIntent,
}

#[derive(Debug, Clone)]
pub(crate) enum Rule {
    Account { sid: String, enabled: bool },
    TaskStarted { path: String },
    Terminal { session_id: u32, connected: bool },
    Service { display_name: String, state: String },
}
#[derive(Debug, Clone)]
pub(crate) struct Recognized {
    pub rule: Rule,
    pub effective_time: String,
    pub rule_id: &'static str,
}

fn unsigned(value: &Value) -> Option<u64> {
    value.as_u64().or_else(||value.as_str().filter(|s|!s.is_empty() && s.bytes().all(|b|b.is_ascii_digit())).and_then(|s|s.parse().ok()))
}
fn readable(value: &Value) -> Option<&str> {
    value.as_str().map(str::trim).filter(|s|!s.is_empty() && *s!="******" && *s!="-" && !s.contains('\0') && s.len()<=4096)
}
pub(crate) fn field<'a>(input: &'a EvidenceInput, names: &[&str]) -> Option<&'a str> {
    let fields=input.payload.get("fields")?.as_object()?;
    let mut found=None;
    for name in names {
        if input.decoding.unreadable_fields.iter().chain(&input.decoding.masked_fields).any(|f|f==name || f==&format!("fields.{name}")) {return None;}
        // Duplicate source fields are preserved by the native reader as Name#N; do not pick one.
        if fields.keys().any(|key|key.starts_with(&format!("{name}#"))) {return None;}
        if let Some(value)=fields.get(*name) {
            let text=readable(value)?;
            if found.is_some_and(|previous|previous!=text) {return None;}
            found=Some(text);
        }
    }
    found
}

/// Only Version 0 schemas below have been checked. EvidenceInput.structure_version is the
/// collector envelope schema, not Windows event Version, and must not substitute for it.
fn header(input: &EvidenceInput, channel: &str, provider: &str, ids: &[u64], allow_import: bool) -> Option<(u64,String)> {
    if input.evidence.kind!=EvidenceKind::Event || !input.decoding.available || !input.decoding.problems.is_empty() {return None;}
    let live_source=format!("pcscope.eventlog.{}",channel.to_ascii_lowercase());
    let imported=input.source.starts_with("pcscope.eventlog.import.");
    if input.source!=live_source && !(allow_import && imported) {return None;}
    let p=&input.payload;
    if p.get("channel")?.as_str()?!=channel || p.get("provider")?.as_str()?!=provider {return None;}
    let id=unsigned(p.get("event_id")?)?;
    if !ids.contains(&id) || p["source_time_available"]==false {return None;}
    let top=p.get("version");
    let system=p.get("system");
    let nested=system.and_then(|s|s.get("Version")).and_then(|v|v.get("value"));
    let version=unsigned(nested.or(top)?)?;
    if version!=0 || top.is_some_and(|value|unsigned(value)!=Some(version)) {return None;}
    if let Some(system)=system {
        for (key,attribute,expected) in [("Provider","Name",provider),("Channel","value",channel)] {
            if let Some(value)=system.get(key) {if value.get(attribute).and_then(Value::as_str)!=Some(expected) {return None;}}
        }
        if let Some(value)=system.get("EventID") {if value.get("value").and_then(unsigned)!=Some(id) {return None;}}
        // Explicitly malformed Version metadata cannot fall back to the convenience field.
        if system.get("Version").is_some() && nested.is_none() {return None;}
    }
    let effective=crate::semantics::canonical_time(input.source_time.as_deref()?).ok()?;
    Some((id,effective))
}

fn guid(value: &str) -> bool {
    let value=value.strip_prefix('{').and_then(|s|s.strip_suffix('}')).unwrap_or(value);
    value.len()==36 && value.bytes().enumerate().all(|(i,b)|if [8,13,18,23].contains(&i){b==b'-'}else{b.is_ascii_hexdigit()})
}

pub(crate) fn recognized(input: &EvidenceInput, allow_import: bool) -> Option<Recognized> {
    // Microsoft audit schemas: event-4722/event-4725, Version 0, TargetSid identifies the target.
    // https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-10/security/threat-protection/auditing/event-4722
    // https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-10/security/threat-protection/auditing/event-4725
    if let Some((id,effective_time))=header(input,"Security",SECURITY,&[4722,4725],allow_import) {
        let sid=field(input,&["TargetSid"])?;
        if !crate::adapters::event::is_account_sid(sid) {return None;}
        return Some(Recognized {rule:Rule::Account {sid:sid.into(),enabled:id==4722},effective_time,rule_id:if id==4722{"security.account-enabled.v0"}else{"security.account-disabled.v0"}});
    }
    // Local TaskScheduler provider metadata: 100 v0 is an instance start. A finish or action
    // return code must not overwrite the task's Enabled setting or invent whole-task success.
    if let Some((_,effective_time))=header(input,TASK_CHANNEL,TASKS,&[100],allow_import) {
        let path=field(input,&["TaskName"])?;
        let run=field(input,&["InstanceId"])?;
        if !path.starts_with('\\') || path.ends_with('\\') || path.split('\\').any(|p|p=="."||p=="..") || !guid(run) {return None;}
        return Some(Recognized {rule:Rule::TaskStarted {path:path.into()},effective_time,rule_id:"task-scheduler.started.v0"});
    }
    // Local provider manifest: LSM 24/25 v0 explicitly disconnect/reconnect an existing session.
    // The native collector flattens UserData/EventXML/SessionID as EventXML/SessionID.
    if let Some((id,effective_time))=header(input,LSM_CHANNEL,LSM,&[24,25],allow_import) {
        let raw=field(input,&["EventXML/SessionID","SessionID"])?;
        if !raw.bytes().all(|b|b.is_ascii_digit()) {return None;}
        let session_id=raw.parse::<u32>().ok()?;
        if session_id==u32::MAX {return None;}
        return Some(Recognized {rule:Rule::Terminal {session_id,connected:id==25},effective_time,rule_id:if id==25{"terminal.reconnected.v0"}else{"terminal.disconnected.v0"}});
    }
    if let Some((_,effective_time))=header(input,"System","Service Control Manager",&[7036],allow_import) {
        let display_name=field(input,&["param1"])?;
        // An allowlist of exact source tokens, not substring matching of a translated message.
        // English tokens and the retained Chinese SCM tokens are independent explicit matches;
        // unrecognised languages/states remain readable events without a field write.
        let raw=field(input,&["param2"])?;
        let state=match raw {"running"|"正在运行"=>"Running","stopped"|"停止"=>"Stopped",_=>return None};
        return Some(Recognized {rule:Rule::Service {display_name:display_name.into(),state:state.into()},effective_time,rule_id:"scm.service-state.v0"});
    }
    None
}

fn scope(context: &AdapterContext) -> String {
    crate::adapters::installation_scope(&crate::semantics::business_scope(&context.collected_scope))
}
fn identity_value<'a>(identity: &'a IdentityCandidate, key: &str) -> Option<&'a str> {
    let mut values=identity.fields.iter().filter(|f|f.key==key);
    let value=values.next()?.value.as_str();
    if values.next().is_some() {return None;}
    readable(&Value::String(value.to_owned())).map(|_|value)
}
fn same_identity(left: &str, right: &str) -> bool {left.eq_ignore_ascii_case(right)}

fn intent(type_id: &str, id: &str, value: Value, found: &Recognized) -> Option<EventStateIntent> {
    let (_,kind)=crate::model::field_definition(type_id,id)?;
    let field_kind=match kind {"configuration"=>FieldKind::Configuration,"state"=>FieldKind::State,_=>return None};
    let value_type=if value.is_boolean(){"boolean"}else if value.is_number(){"number"}else{"string"};
    Some(EventStateIntent {fields:vec![FieldCandidate {
        field_id:id.into(),field_kind,value:Some(value),value_type:value_type.into(),status:ValueStatus::Known,
        source_time:Some(found.effective_time.clone()),derivation:Derivation::Decoded,conflicts:Vec::new(),
    }],effective_time:found.effective_time.clone(),rule_id:found.rule_id})
}

/// Returns no current-field intent for imported material, unknown schemas, incomplete/ambiguous
/// targets or targets that disagree with the event. Storage remains responsible for time-order
/// arbitration and verifying that the event's machine/installation is the requested context.
pub fn derive_event_state(input: &EvidenceInput, context: &AdapterContext, action: &ActionCandidate) -> Option<EventStateIntent> {
    let found=recognized(input,false)?;
    let ActionTarget::Assigned {identity}= &action.target else {return None};
    if !identity.missing.is_empty() || identity.ambiguous.is_some() || context.machine_id.is_empty() || context.installation_id.is_empty()
        || !action.evidence.contains(&input.evidence) {return None;}
    let base=scope(context);
    if base.is_empty() {return None;}
    if identity_value(identity,"scope")!=Some(identity.scope.as_str()) {return None;}
    match &found.rule {
        Rule::Account {sid,enabled} if identity.type_id=="account.user" && identity.scope==base
            && action.action_id==if *enabled {"identity.account_enabled"}else{"identity.account_disabled"}
            && same_identity(identity_value(identity,"sid")?,sid) => intent(&identity.type_id,"enabled",Value::Bool(*enabled),&found),
        Rule::TaskStarted {path} if identity.type_id=="startup.scheduled_task" && identity.scope==base
            && action.action_id=="startup.task_started" && same_identity(identity_value(identity,"path")?,path) =>
                intent(&identity.type_id,"last_run",Value::String(found.effective_time.clone()),&found),
        Rule::Terminal {session_id,connected} => {
            // Context.boot_id must be the event's proven boot, never today's boot guessed for
            // old logs. Boot-specific scope prevents a reused WTS ID changing another session.
            let boot=context.boot_id.as_deref().filter(|b|!b.trim().is_empty() && !b.contains('/'))?;
            if identity.type_id!="runtime.terminal_session" || identity.scope!=format!("{base}/boot:{boot}")
                || identity_value(identity,"native_id")?.parse::<u32>().ok()!=Some(*session_id)
                || action.action_id!=if *connected {"identity.session_reconnected"}else{"identity.session_disconnected"} {return None;}
            // Existing snapshot state uses WTS_CONNECTSTATE_CLASS: Active=0, Disconnected=4.
            // https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/ne-wtsapi32-wts_connectstate_class
            intent(&identity.type_id,"state",Value::from(if *connected {0}else{4}),&found)
        }
        // A display-name service match is resolved by storage, through service_state_match.
        _=>None,
    }
}

pub fn service_state_match(input: &EvidenceInput, context: &AdapterContext) -> Option<ServiceStateMatch> {
    if context.machine_id.is_empty() || context.installation_id.is_empty() || scope(context).is_empty() {return None;}
    let found=recognized(input,false)?;
    let Rule::Service {display_name,state}=&found.rule else {return None};
    Some(ServiceStateMatch {display_name:display_name.clone(),state:state.clone(),intent:intent("startup.service","state",Value::String(state.clone()),&found)?})
}

/// Historical text stays readable without a uniquely matched current object.
pub fn service_display_name(input: &EvidenceInput) -> Option<String> {
    match recognized(input,true)?.rule {
        Rule::Service {display_name,..}=>Some(display_name),
        _=>None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{ActionOutcome,Decoding,EvidenceRef,TimePrecision};
    use serde_json::json;
    fn context()->AdapterContext {AdapterContext {catalog_version:crate::model::catalog_version().into(),rule_version:"test".into(),machine_id:"m".into(),installation_id:"i".into(),boot_id:Some("boot-1".into()),collected_scope:"machine:m/install:i".into(),complete:true}}
    fn input(channel:&str,provider:&str,id:u64,fields:Value)->EvidenceInput {EvidenceInput {
        source:format!("pcscope.eventlog.{}",channel.to_lowercase()),item_id:"windows.event".into(),structure_version:"evidence-v2".into(),source_scope:"machine:m/install:i".into(),
        evidence:EvidenceRef {kind:EvidenceKind::Event,id:format!("{channel}/1")},native_record_id:"1".into(),source_time:Some("2026-10-13T01:00:00Z".into()),observed_at:"2026-10-13T01:00:01Z".into(),ingested_at:"2026-10-13T01:00:02Z".into(),
        decoding:Decoding {available:true,..Default::default()},payload:json!({"channel":channel,"provider":provider,"event_id":id,"fields":fields,"system":{"Version":{"value":"0"}},"source_time_available":true}),
    }}
    fn action(input:&EvidenceInput,typ:&str,key:&str,value:&str,id:&str)->ActionCandidate {ActionCandidate {occurrence_key:None,source_record:None,target:ActionTarget::Assigned {identity:crate::adapters::identity(typ,"machine:m/install:i",&input.evidence.id,&[(key,Some(value.into()))])},action_id:id.into(),phase:None,result:ActionOutcome::Observed,precision:TimePrecision::Exact,occurred_at:input.source_time.clone(),interval_from:None,interval_until:None,observed_at:input.observed_at.clone(),field_id:None,before:None,after:None,evidence:vec![input.evidence.clone()],actor:None,program:None}}
    #[test] fn enabled_and_disabled_use_target_sid_and_declared_configuration_field(){
        for (id,value,kind) in [(4722,true,"identity.account_enabled"),(4725,false,"identity.account_disabled")] {
            let input=input("Security",SECURITY,id,json!({"TargetSid":"S-1-5-21-1","SubjectUserSid":"S-1-5-21-2"}));
            let a=action(&input,"account.user","sid","S-1-5-21-1",kind);
            let got=derive_event_state(&input,&context(),&a).unwrap();
            assert_eq!(got.fields[0].value,Some(json!(value)));assert_eq!(got.fields[0].field_id,"enabled");assert_eq!(got.fields[0].field_kind,FieldKind::Configuration);
            assert_eq!(got.effective_time,"2026-10-13T01:00:00.000Z");
            assert!(derive_event_state(&input,&context(),&action(&input,"account.user","sid","S-1-5-21-2",kind)).is_none());
        }
    }
    #[test] fn unknown_provider_id_version_import_and_unavailable_time_never_write(){
        let base=input("Security",SECURITY,4722,json!({"TargetSid":"S-1-5-21-1"}));let a=action(&base,"account.user","sid","S-1-5-21-1","identity.account_enabled");
        for case in 0..11 {let mut i=base.clone();match case {
            0=>i.payload["provider"]=json!("Another provider"),1=>i.payload["event_id"]=json!(4720),2=>i.payload["system"]["Version"]["value"]=json!("1"),
            3=>{i.payload["system"].as_object_mut().unwrap().remove("Version");},4=>i.payload["version"]=json!(1),5=>i.source="pcscope.eventlog.import.archive".into(),
            6=>i.source_time=None,7=>i.payload["source_time_available"]=json!(false),8=>i.payload["channel"]=json!("Application"),9=>i.decoding.available=false,
            _=>i.payload["system"]["Provider"]=json!({"Name":"Other"}),
        }assert!(derive_event_state(&i,&context(),&a).is_none(),"case {case}");}
    }
    #[test] fn incomplete_protected_or_wrong_scope_identity_is_not_a_current_update(){
        let base=input("Security",SECURITY,4722,json!({"TargetSid":"S-1-5-21-1"}));let a=action(&base,"account.user","sid","S-1-5-21-1","identity.account_enabled");
        for value in [json!("S-1-0-0"),json!("******"),json!({"sensitive":true}),json!(123)] {let mut i=base.clone();i.payload["fields"]["TargetSid"]=value;assert!(derive_event_state(&i,&context(),&a).is_none());}
        let mut i=base.clone();i.payload["fields"]["TargetSid#2"]=json!("S-1-5-21-1");assert!(derive_event_state(&i,&context(),&a).is_none());
        let mut a=a;let ActionTarget::Assigned{identity}= &mut a.target else{panic!()};identity.missing.push("sid".into());assert!(derive_event_state(&base,&context(),&a).is_none());
        let ActionTarget::Assigned{identity}= &mut a.target else{panic!()};identity.missing.clear();identity.scope="other".into();assert!(derive_event_state(&base,&context(),&a).is_none());
    }
    #[test] fn task_start_updates_last_run_only_and_end_does_not_invent_whole_task_result(){
        let base=input(TASK_CHANNEL,TASKS,100,json!({"TaskName":"\\Folder\\Task","InstanceId":"{12345678-1234-1234-1234-123456789abc}"}));
        let a=action(&base,"startup.scheduled_task","path","\\Folder\\Task","startup.task_started");let got=derive_event_state(&base,&context(),&a).unwrap();
        assert_eq!(got.fields.len(),1);assert_eq!(got.fields[0].field_id,"last_run");
        for id in [102,201] {let mut i=base.clone();i.payload["event_id"]=json!(id);i.payload["fields"]["ResultCode"]=json!(0);assert!(derive_event_state(&i,&context(),&a).is_none());}
        let mut i=base;i.payload["fields"]["InstanceId"]=json!("unknown");assert!(derive_event_state(&i,&context(),&a).is_none());
    }
    #[test] fn terminal_connection_state_requires_event_boot_and_never_translates_lock_as_connection(){
        for (id,connected,kind) in [(24,false,"identity.session_disconnected"),(25,true,"identity.session_reconnected")] {
            let i=input(LSM_CHANNEL,LSM,id,json!({"EventXML/SessionID":"7"}));let mut a=action(&i,"runtime.terminal_session","native_id","7",kind);
            let ActionTarget::Assigned {identity}=&mut a.target else{panic!()};identity.scope.push_str("/boot:boot-1");identity.fields.iter_mut().find(|f|f.key=="scope").unwrap().value=identity.scope.clone();
            assert_eq!(derive_event_state(&i,&context(),&a).unwrap().fields[0].value,Some(json!(if connected {0}else{4})));
            let mut c=context();c.boot_id=None;assert!(derive_event_state(&i,&c,&a).is_none());c.boot_id=Some("boot-2".into());assert!(derive_event_state(&i,&c,&a).is_none());
        }
        for id in [4778,4779,4800,4801] {let i=input("Security",SECURITY,id,json!({"SessionId":"7","SessionName":"RDP-Tcp#7"}));assert!(recognized(&i,false).is_none());}
    }
    #[test] fn scm_display_name_is_a_match_request_not_a_service_identity(){
        for (raw,expected) in [("running","Running"),("stopped","Stopped"),("正在运行","Running"),("停止","Stopped")] {
            let i=input("System","Service Control Manager",7036,json!({"param1":"Example display name","param2":raw}));let matched=service_state_match(&i,&context()).unwrap();
            assert_eq!(matched.display_name,"Example display name");assert_eq!(matched.state,expected);assert_eq!(matched.intent.fields[0].value,Some(json!(expected)));
            assert!(derive_event_state(&i,&context(),&action(&i,"startup.service","name","Example display name","observed.state_changed")).is_none(),"only storage may uniquely resolve a display name");
        }
        for raw in ["not running","running soon","已停止","4","en cours d’exécution"] {let i=input("System","Service Control Manager",7036,json!({"param1":"Example","param2":raw}));assert!(service_state_match(&i,&context()).is_none());}
    }
}
