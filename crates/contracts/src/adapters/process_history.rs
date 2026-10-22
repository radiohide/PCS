//! Historical process identity is distinct from a native PID + creation-time identity.
//! Security 4688 identifies one start occurrence; Sysmon supplies its own process GUID.
use super::*;
use crate::adapter::{ActionCandidate,ActionTarget};
use crate::event_state::field;

pub fn pid(value:&str)->Option<String> {
    let number=value.strip_prefix("0x").or_else(||value.strip_prefix("0X"))
        .map(|hex|u32::from_str_radix(hex,16).ok()).unwrap_or_else(||value.parse::<u32>().ok())?;
    (number>0).then(||number.to_string())
}
pub fn security_start(input:&EvidenceInput,context:&AdapterContext)->Option<IdentityCandidate> {
    if input.evidence.kind!=EvidenceKind::Event||input.source!="pcscope.eventlog.security"||!input.decoding.available||!input.decoding.problems.is_empty(){return None}
    let p=&input.payload;
    if p["channel"]!="Security"||p["provider"]!="Microsoft-Windows-Security-Auditing"||p["event_id"]!=4688||p["source_time_available"]==false{return None}
    let number=|v:&Value|v.as_u64().or_else(||v.as_str()?.parse().ok());
    let nested=p.get("system").and_then(|s|s.get("Version")).and_then(|v|v.get("value"));
    let version=number(nested.or_else(||p.get("version"))?)?;
    if version>2||p.get("version").is_some_and(|v|number(v)!=Some(version)){return None}
    if let Some(system)=p.get("system") {
        if system.get("Version").is_some()&&nested.is_none(){return None}
        for (key,attr,value) in [("Provider","Name","Microsoft-Windows-Security-Auditing"),("Channel","value","Security")] {
            if system.get(key).is_some_and(|v|v.get(attr).and_then(Value::as_str)!=Some(value)){return None}
        }
        if system.get("EventID").is_some_and(|v|v.get("value").and_then(number)!=Some(4688)){return None}
    }
    crate::semantics::canonical_time(input.source_time.as_deref()?).ok()?;
    let pid=pid(field(input,&["NewProcessId"])?)?;
    Some(identity("runtime.process",&installation_scope(&context.collected_scope),&input.evidence.id,
        &[("pid",Some(pid)),("created_at",None),("start_event",Some(input.evidence.id.clone()))]))
}
pub fn is_history(candidate:&IdentityCandidate)->bool {
    crate::identity::historical_process_key(candidate).is_some()
}
pub fn fields(input:&EvidenceInput,action:&ActionCandidate)->Vec<FieldCandidate> {
    let ActionTarget::Assigned{identity}=&action.target else{return vec![]};
    if !is_history(identity){return vec![]}
    let image=field(input,&["NewProcessName","Image"]).map(str::to_owned);
    let pid=identity.fields.iter().find(|f|f.key=="pid").map(|f|f.value.clone());
    let mut values=vec![("identity_basis",Some("event_history".into())),("pid",pid),
        ("name",image.as_deref().and_then(|s|s.rsplit(['\\','/']).next()).map(str::to_owned)),("image",image)];
    values.push((if action.action_id=="execution.process_stopped"{"ended_at"}else{"started_at"},action.occurred_at.clone()));
    values.into_iter().filter_map(|(key,value)|value.map(|value|FieldCandidate{
        field_id:key.into(),field_kind:if key=="pid"{FieldKind::Identity}else{FieldKind::Configuration},value:Some(Value::String(value)),value_type:"string".into(),status:ValueStatus::Known,
        source_time:action.occurred_at.clone(),derivation:Derivation::SourceProvided,conflicts:vec![],
    })).collect()
}
