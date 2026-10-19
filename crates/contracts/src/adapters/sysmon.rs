//! Decode the native reader's broad execution.sysmon_event family.
//! Checked Windows schemas: microsoft/SysmonCommon manifest.xml (schema 4.90).
//! https://github.com/microsoft/SysmonCommon/blob/main/manifest.xml
//! A Sysmon ProcessGuid is reliable inside this stream; it is not a Windows creation timestamp.
use super::*;
use crate::adapter::{ActionCandidate,ActionOutcome,ActionTarget,TimePrecision};
use crate::event_state::field;

const CHANNEL:&str="Microsoft-Windows-Sysmon/Operational";
const SOURCE:&str="pcscope.eventlog.microsoft-windows-sysmon/operational";
fn integer(v:&Value)->Option<u64>{v.as_u64().or_else(||v.as_str()?.parse().ok())}
fn process_guid(input:&EvidenceInput)->Option<String>{
    let value=field(input,&["ProcessGuid"])?;
    let value=value.strip_prefix('{').and_then(|s|s.strip_suffix('}')).unwrap_or(value);
    if value.len()!=36||!value.bytes().enumerate().all(|(i,b)|if [8,13,18,23].contains(&i){b==b'-'}else{b.is_ascii_hexdigit()})||value.bytes().all(|b|b==b'0'||b==b'-'){return None}
    Some(value.to_ascii_lowercase())
}
pub fn action(input:&EvidenceInput,context:&AdapterContext)->Option<ActionCandidate>{
    if input.evidence.kind!=EvidenceKind::Event||!input.decoding.available||!input.decoding.problems.is_empty(){return None}
    if input.source!=SOURCE&&!input.source.starts_with("pcscope.eventlog.import."){return None}
    let p=&input.payload;
    if p["channel"]!=CHANNEL||p["provider"]!="Microsoft-Windows-Sysmon"{return None}
    let id=integer(p.get("event_id")?)?;
    let (version,action_id,typ)=match id {
        1=>(5,"execution.process_started","runtime.process"),
        5=>(3,"execution.process_stopped","runtime.process"),
        6=>(4,"execution.driver_loaded","hardware.driver_service"),
        7=>(3,"execution.module_loaded","runtime.module_instance"),
        3=>(5,"network.connection_observed","network.endpoint"),
        11=>(2,"files.created","file.watched_file"),
        23=>(5,"files.deleted_archived","file.watched_file"),
        26=>(5,"files.deleted","file.watched_file"),
        _=>return None,
    };
    let nested=p.get("system").and_then(|s|s.get("Version")).and_then(|v|v.get("value"));
    if integer(nested.or_else(||p.get("version"))?)!=Some(version){return None}
    if p.get("version").is_some_and(|v|integer(v)!=Some(version)){return None}
    if let Some(system)=p.get("system") {
        if system.get("Version").is_some()&&nested.is_none(){return None}
        for (key,attr,value) in [("Provider","Name","Microsoft-Windows-Sysmon"),("Channel","value",CHANNEL)] {
            if system.get(key).is_some_and(|v|v.get(attr).and_then(Value::as_str)!=Some(value)){return None}
        }
        if system.get("EventID").is_some_and(|v|v.get("value").and_then(integer)!=Some(id)){return None}
    }
    let scope=installation_scope(&crate::semantics::business_scope(&context.collected_scope));
    let guid=process_guid(input);
    let pid=field(input,&["ProcessId"]).and_then(|v|v.parse::<u32>().ok()).filter(|pid|*pid>0).map(|pid|pid.to_string());
    let program=field(input,&["Image"]).map(str::to_owned);
    let target=match id {
        1|5=>{
            let mut keys=vec![("pid",pid),("created_at",None)];
            if let Some(guid)=guid.clone(){keys.push(("process_guid",Some(guid)));}
            // Without the stream GUID a reused PID must not bind unrelated events together.
            if guid.is_none(){ActionTarget::Unassigned{reason:"进程记录缺少实例标识。".into()}}
            else{ActionTarget::Assigned{identity:identity(typ,&scope,&input.evidence.id,&keys)}}
        }
        7=>{
            let path=field(input,&["ImageLoaded"]).map(str::to_owned);
            if guid.is_none()||path.is_none(){ActionTarget::Unassigned{reason:"模块记录缺少所属进程或模块文件。".into()}}
            else{ActionTarget::Assigned{identity:identity(typ,&scope,&input.evidence.id,&[("process_identity",guid.map(|g|format!("sysmon:{g}"))),("module_path",path)])}}
        }
        3=>ActionTarget::Assigned{identity:identity(typ,&scope,&input.evidence.id,&[
            ("local_address",field(input,&["SourceIp"]).map(str::to_owned)),("local_port",field(input,&["SourcePort"]).map(str::to_owned)),
            ("remote_address",field(input,&["DestinationIp"]).map(str::to_owned)),("remote_port",field(input,&["DestinationPort"]).map(str::to_owned)),
            ("protocol",field(input,&["Protocol"]).map(str::to_owned)),("owning_pid",pid),
            ("created_at",None),
            ("process_guid",guid),
        ])},
        // Driver events name an image, not the service Name; file events do not state FileId.
        // Preserve definite actions without attaching them to a same-named current object.
        6=>ActionTarget::Unassigned{reason:"驱动加载记录提供文件，未提供服务标识。".into()},
        _=>ActionTarget::Unassigned{reason:"文件操作记录未提供磁盘文件标识。".into()},
    };
    let occurred_at=input.source_time.as_deref().and_then(|at|crate::semantics::canonical_time(at).ok())
        .filter(|_|p["source_time_available"]!=false);
    let subject=if id==6||id==7{field(input,&["ImageLoaded"])}else if matches!(id,11|23|26){field(input,&["TargetFilename"])}else{field(input,&["Image"])};
    Some(ActionCandidate{target,action_id:action_id.into(),phase:None,
        result:if id==1{ActionOutcome::Started}else if id==5{ActionOutcome::Ended}else{ActionOutcome::Observed},
        precision:if occurred_at.is_some(){TimePrecision::Exact}else{TimePrecision::ObservationOnly},occurred_at,
        interval_from:None,interval_until:None,observed_at:input.observed_at.clone(),field_id:None,before:None,after:None,
        evidence:vec![input.evidence.clone()],actor:field(input,&["User"]).map(str::to_owned),program,
        occurrence_key:None,source_record:Some(serde_json::json!({"sysmon":{"event_id":id,"version":version,"subject":subject,"process_guid":process_guid(input)}}))})
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::adapter::{Decoding,EvidenceRef,SourceAdapter};
    use serde_json::json;
    fn context()->AdapterContext{AdapterContext{machine_id:"m".into(),installation_id:"i".into(),collected_scope:"machine:m".into(),boot_id:None,complete:true,catalog_version:crate::model::catalog_version().into(),rule_version:"test".into()}}
    fn event(id:u64,version:u64)->EvidenceInput{EvidenceInput{
        source:SOURCE.into(),item_id:"execution.sysmon_event".into(),native_record_id:"10".into(),source_scope:"".into(),structure_version:"evidence-v2".into(),
        source_time:Some("2026-10-19T01:00:00Z".into()),observed_at:"2026-10-19T01:00:01Z".into(),ingested_at:"2026-10-19T01:00:02Z".into(),
        evidence:EvidenceRef{kind:EvidenceKind::Event,id:format!("{SOURCE}|epoch|{id}")},decoding:Decoding{available:true,..Default::default()},
        payload:json!({"channel":CHANNEL,"provider":"Microsoft-Windows-Sysmon","event_id":id,"system":{"Version":{"value":version.to_string()}},"source_time_available":true,
            "fields":{"ProcessGuid":"{01234567-1234-1234-1234-123456789abc}","ProcessId":"42","Image":"C:\\Example\\example.exe","ImageLoaded":"C:\\Example\\module.dll","TargetFilename":"C:\\Example\\file.txt","Protocol":"tcp","SourceIp":"127.0.0.1","SourcePort":"50000","DestinationIp":"127.0.0.1","DestinationPort":"443"}})}}
    #[test]
    fn native_family_reaches_checked_actions_and_valid_targets(){
        for(id,version,expected)in[(1,5,"execution.process_started"),(5,3,"execution.process_stopped"),(6,4,"execution.driver_loaded"),(7,3,"execution.module_loaded"),(3,5,"network.connection_observed"),(11,2,"files.created"),(23,5,"files.deleted_archived"),(26,5,"files.deleted")]{
            let input=event(id,version);let result=super::super::EventAdapter.adapt(&input,&context());
            assert_eq!(result.actions.len(),1,"{id}");assert_eq!(result.actions[0].action_id,expected);
            assert!(crate::adapter::validate(&result,&context()).is_empty(),"event {id}: {:?}",crate::adapter::validate(&result,&context()));
        }
    }
    #[test]
    fn version_channel_provider_and_duplicate_identity_fields_cannot_be_guessed(){
        for property in ["version","channel","provider","source"]{
            let mut input=event(1,5);match property{"version"=>input.payload["system"]["Version"]["value"]=json!("99"),"channel"=>input.payload["channel"]=json!("Security"),"provider"=>input.payload["provider"]=json!("Other"),_=>input.source="pcscope.eventlog.security".into()};
            assert!(action(&input,&context()).is_none(),"{property}");
            input.item_id="execution.process_started".into();
            assert!(super::super::EventAdapter.adapt(&input,&context()).actions.is_empty(),"preclassified kind cannot bypass {property}");
        }
        let mut input=event(1,5);input.payload["fields"]["ProcessGuid#2"]=json!("other");
        assert!(matches!(action(&input,&context()).unwrap().target,ActionTarget::Unassigned{..}));
        let mut input=event(1,5);input.payload["source_time_available"]=json!(false);let result=action(&input,&context()).unwrap();
        assert_eq!(result.precision,TimePrecision::ObservationOnly);assert!(result.occurred_at.is_none());
    }
}
