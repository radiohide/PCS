//! Additional precisely qualified actions for sources whose native Kind is a broad family.
use crate::adapter::{ActionCandidate,ActionOutcome,ActionTarget,AdapterContext,EvidenceInput,TimePrecision};
use crate::event_state::{field,recognized,Rule};
use serde_json::json;

pub fn state_action(input:&EvidenceInput,context:&AdapterContext)->Option<ActionCandidate> {
    let found=recognized(input,true)?;
    let scope=crate::adapters::installation_scope(&crate::semantics::business_scope(&context.collected_scope));
    let make=|typ,key,value|ActionTarget::Assigned {identity:crate::adapters::identity(typ,&scope,&input.evidence.id,&[(key,Some(value))])};
    let (target,action_id,result,field_id,after,source_record)=match found.rule {
        Rule::Account {sid,enabled}=>(make("account.user","sid",sid),if enabled {"identity.account_enabled"}else{"identity.account_disabled"},ActionOutcome::Succeeded,None,None,None),
        Rule::TaskStarted {path}=>(make("startup.scheduled_task","path",path),"startup.task_started",ActionOutcome::Started,None,None,None),
        Rule::Terminal {session_id,connected}=>{
            // An import's machine/boot cannot be established from the receiving collector context.
            let boot=(!input.source.starts_with("pcscope.eventlog.import.")).then_some(context.boot_id.as_deref()).flatten();
            let target=match boot.filter(|b|!b.trim().is_empty()&&!b.contains('/')) {
                Some(boot)=>ActionTarget::Assigned {identity:crate::adapters::identity("runtime.terminal_session",&format!("{scope}/boot:{boot}"),&input.evidence.id,&[("native_id",Some(session_id.to_string()))])},
                None=>ActionTarget::Unassigned {reason:"已记录会话连接变化；缺少该事件的启动周期，暂不关联可能复用的会话编号。".into()},
            };
            (target,if connected {"identity.session_reconnected"}else{"identity.session_disconnected"},ActionOutcome::Observed,None,None,None)
        }
        Rule::Service {display_name,state}=>(ActionTarget::Unassigned {reason:"服务日志提供显示名称；需要在同一范围唯一匹配现有服务名称。".into()},"startup.service_state_changed",ActionOutcome::Observed,Some("state".into()),Some(json!(state)),Some(json!({"service_display_name":display_name,"rule_id":found.rule_id}))),
    };
    Some(ActionCandidate {occurrence_key:None,source_record,target,action_id:action_id.into(),phase:None,result,precision:TimePrecision::Exact,
        occurred_at:Some(found.effective_time),interval_from:None,interval_until:None,observed_at:input.observed_at.clone(),field_id,before:None,after,
        evidence:vec![input.evidence.clone()],actor:field(input,&["SubjectUserName"]).map(str::to_owned),
        program:field(input,&["NewProcessName","Image","ProcessName"]).map(str::to_owned)})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{validate,Decoding,EvidenceKind,EvidenceRef,SourceAdapter};
    use crate::event_state::{derive_event_state,service_state_match};
    fn context()->AdapterContext {AdapterContext {catalog_version:crate::model::catalog_version().into(),rule_version:"test".into(),machine_id:"m".into(),installation_id:"i".into(),boot_id:Some("boot-1".into()),collected_scope:"machine:m/install:i".into(),complete:true}}
    fn event(channel:&str,provider:&str,id:u64,fields:serde_json::Value)->EvidenceInput {EvidenceInput {
        source:format!("pcscope.eventlog.{}",channel.to_lowercase()),item_id:"windows.event".into(),structure_version:"evidence-v2".into(),source_scope:"machine:m/install:i".into(),
        evidence:EvidenceRef {kind:EvidenceKind::Event,id:format!("{channel}/1")},native_record_id:"1".into(),source_time:Some("2026-10-13T01:00:00Z".into()),observed_at:"2026-10-13T01:00:01Z".into(),ingested_at:"2026-10-13T01:00:02Z".into(),
        decoding:Decoding {available:true,..Default::default()},payload:json!({"channel":channel,"provider":provider,"event_id":id,"fields":fields,"system":{"Version":{"value":"0"}},"source_time_available":true}),
    }}
    #[test]
    fn precise_native_actions_validate_and_keep_actor_without_copying_protected_values() {
        let items=[
            event("Security","Microsoft-Windows-Security-Auditing",4722,json!({"TargetSid":"S-1-5-21-1","SubjectUserName":"operator","ProcessName":"tool.exe"})),
            event("Microsoft-Windows-TaskScheduler/Operational","Microsoft-Windows-TaskScheduler",100,json!({"TaskName":"\\Folder\\Task","InstanceId":"{12345678-1234-1234-1234-123456789abc}"})),
            event("Microsoft-Windows-TerminalServices-LocalSessionManager/Operational","Microsoft-Windows-TerminalServices-LocalSessionManager",24,json!({"EventXML/SessionID":"7"})),
            event("System","Service Control Manager",7036,json!({"param1":"Example display name","param2":"running"})),
        ];
        for (index,input) in items.iter().enumerate() {
            let result=super::super::EventAdapter.adapt(input,&context());
            assert!(validate(&result,&context()).is_empty(),"{index}: {:?}",validate(&result,&context()));
            assert_eq!(result.actions.len(),1);assert!(result.observations.is_empty());
            if index==3 {assert!(service_state_match(input,&context()).is_some());assert!(matches!(result.actions[0].target,ActionTarget::Unassigned {..}));assert_eq!(result.actions[0].action_id,"startup.service_state_changed");assert!(result.actions[0].before.is_none());assert_eq!(result.actions[0].after,Some(json!("Running")));}
            else {assert!(derive_event_state(input,&context(),&result.actions[0]).is_some(),"{index}");}
        }
        let a=state_action(&items[0],&context()).unwrap();assert_eq!(a.actor.as_deref(),Some("operator"));assert_eq!(a.program.as_deref(),Some("tool.exe"));
        let mut protected=items[0].clone();protected.payload["fields"]["SubjectUserName"]=json!({"protected":true,"ciphertext":"not a name"});
        assert_eq!(state_action(&protected,&context()).unwrap().actor,None);
    }
    #[test]
    fn imports_keep_event_meaning_without_current_fields_or_reused_session_binding() {
        let mut input=event("Security","Microsoft-Windows-Security-Auditing",4725,json!({"TargetSid":"S-1-5-21-1"}));
        input.source="pcscope.eventlog.import.archive".into();
        let a=state_action(&input,&context()).unwrap();assert_eq!(a.action_id,"identity.account_disabled");assert!(derive_event_state(&input,&context(),&a).is_none());
        let mut input=event("Microsoft-Windows-TerminalServices-LocalSessionManager/Operational","Microsoft-Windows-TerminalServices-LocalSessionManager",25,json!({"SessionID":"7"}));
        input.source="pcscope.eventlog.import.archive".into();
        let a=state_action(&input,&context()).unwrap();assert!(matches!(a.target,ActionTarget::Unassigned {..}));assert!(derive_event_state(&input,&context(),&a).is_none());
    }
    #[test]
    fn unsupported_versions_keep_existing_event_fallback_without_any_state_intent() {
        let mut input=event("Security","Microsoft-Windows-Security-Auditing",4722,json!({"TargetSid":"S-1-5-21-1"}));
        input.item_id="identity.account_enabled".into();input.payload["system"]["Version"]["value"]=json!("99");
        assert!(state_action(&input,&context()).is_none());
        let result=super::super::EventAdapter.adapt(&input,&context());assert_eq!(result.actions[0].action_id,"identity.account_enabled");
        assert!(validate(&result,&context()).is_empty());assert!(derive_event_state(&input,&context(),&result.actions[0]).is_none());
    }
}
