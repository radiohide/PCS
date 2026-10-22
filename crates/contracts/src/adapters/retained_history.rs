//! Historical subjects are independent of live inventory and never assert presence or removal.
use super::*;

pub fn guid(value:&Value)->Option<String> {
    let raw=value.as_str()?.trim();
    let id=raw.strip_prefix('{').and_then(|s|s.strip_suffix('}')).unwrap_or(raw);
    if id.len()!=36||!id.bytes().enumerate().all(|(n,c)|if [8,13,18,23].contains(&n){c==b'-'}else{c.is_ascii_hexdigit()})||id=="00000000-0000-0000-0000-000000000000" {return None}
    Some(id.to_ascii_lowercase())
}
pub fn is_source(source:&str)->bool {matches!(source,"pcscope.system.update-history"|"pcscope.security.detections")}
pub fn fields(input:&EvidenceInput)->Vec<FieldCandidate> {
    let p=&input.payload;
    if input.source=="pcscope.system.update-history" {
        vec![field(p,"name","title",FieldKind::Configuration),field(p,"update_id","update_id",FieldKind::Identity),field(p,"revision","revision",FieldKind::Identity)]
    } else if input.source=="pcscope.security.detections" {
        vec![field(p,"name","ThreatName",FieldKind::Configuration),field(p,"detection_id","DetectionID",FieldKind::Identity),field(p,"threat_id","ThreatID",FieldKind::Identity)]
    } else {vec![]}
}
pub fn update_target(input:&EvidenceInput,context:&AdapterContext)->ActionTarget {
    match (guid(&input.payload["update_id"]),input.payload["revision"].as_u64()) {
        (Some(id),Some(revision))=>ActionTarget::Assigned{identity:identity("software.update_history_subject",&installation_scope(&context.collected_scope),&input.evidence.id,&[("update_id",Some(id)),("revision",Some(revision.to_string()))])},
        _=>ActionTarget::Unassigned{reason:"更新记录缺少有效更新标识或修订号。".into()}
    }
}
pub fn detections(input:&EvidenceInput,context:&AdapterContext)->AdapterResult {
    let p=&input.payload;
    if !p.is_object(){return AdapterResult::failed("detection record is not structured",false)}
    let target=match (guid(&p["DetectionID"]),p["ThreatID"].as_u64()) {
        (Some(id),Some(threat))=>ActionTarget::Assigned{identity:identity("security.detection_history",&installation_scope(&context.collected_scope),&input.evidence.id,&[("detection_id",Some(id)),("threat_id",Some(threat.to_string()))])},
        _=>ActionTarget::Unassigned{reason:"检测记录缺少有效检测标识或威胁标识。".into()}
    };
    let identity_key=match &target {ActionTarget::Assigned{identity}=>serde_json::to_string(&identity.fields.iter().map(|v|&v.value).collect::<Vec<_>>()).unwrap(),_=>format!("{}|{}",input.evidence.id,input.item_id)};
    let time=|key:&str|p[key].as_str().and_then(|v|crate::semantics::canonical_time(v).ok());
    let initial=time("InitialDetectionTime");let changed=time("LastThreatStatusChangeTime");
    let mut result=AdapterResult::adapted();
    let mut emit=|action_id:&str,phase:&str,at:Option<String>,outcome:ActionOutcome| {
        let stage=if phase=="detected" {serde_json::json!([phase,at])}else{serde_json::json!([phase,at,p["ThreatStatusID"],p["CleaningActionID"],p["ActionSuccess"]])};
        result.actions.push(ActionCandidate{target:target.clone(),action_id:action_id.into(),phase:Some(phase.into()),result:outcome,
            occurrence_key:Some(serde_json::json!([input.source,installation_scope(&context.collected_scope),identity_key,stage]).to_string()),source_record:Some(p.clone()),
            precision:if at.is_some(){TimePrecision::Exact}else{TimePrecision::ObservationOnly},occurred_at:at,interval_from:None,interval_until:None,
            observed_at:input.observed_at.clone(),field_id:None,before:None,after:None,evidence:vec![input.evidence.clone()],actor:None,program:None});
    };
    emit("security.threat_detected","detected",initial.clone(),ActionOutcome::Observed);
    // ActionSuccess alone defaults to false. Only a timestamped status change describes a
    // response; status codes, rather than a missing bool, decide whether it failed.
    if changed.as_ref().is_some_and(|at|initial.as_ref().is_none_or(|start|at>=start))&&p["ThreatStatusID"].as_u64().is_some_and(|v|v>1) {
        let outcome=match p["ThreatStatusID"].as_u64(){Some(2..=6) if p["ActionSuccess"]==true=>ActionOutcome::Succeeded,Some(101|102|103|104|107)=>ActionOutcome::Failed,Some(105)=>ActionOutcome::Ended,_=>ActionOutcome::Observed};
        emit("security.threat_status_recorded","response",changed,outcome);
    }
    result
}

#[cfg(test)] mod tests {
    use super::*;
    use serde_json::json;
    fn context()->AdapterContext {AdapterContext{catalog_version:crate::model::catalog_version().into(),rule_version:"test".into(),machine_id:"M".into(),installation_id:"I".into(),boot_id:None,collected_scope:"machine:M/user:U".into(),complete:true}}
    fn input(payload:Value)->EvidenceInput {EvidenceInput{source:"pcscope.security.detections".into(),item_id:"one".into(),structure_version:"v1".into(),source_scope:"machine:M/user:U".into(),evidence:EvidenceRef::snapshot("detections","machine:M","run"),native_record_id:"one".into(),source_time:None,observed_at:"2026-10-22T00:00:00Z".into(),ingested_at:"2026-10-22T00:00:00Z".into(),decoding:Decoding{available:true,..Default::default()},payload}}
    #[test] fn retained_identity_rejects_protected_or_malformed_components() {
        for value in [json!(null),json!(true),json!({"sensitive":true}),json!("not-a-guid"),json!("00000000-0000-0000-0000-000000000000")] {
            let i=input(json!({"update_id":value,"revision":1,"DetectionID":value,"ThreatID":1}));
            assert!(matches!(update_target(&i,&context()),ActionTarget::Unassigned{..}));
            let result=detections(&i,&context());assert!(matches!(result.actions[0].target,ActionTarget::Unassigned{..}));
            assert!(crate::adapter::validate(&result,&context()).is_empty());
        }
        for revision in [json!(null),json!(true),json!(-1),json!(1.2)] {
            assert!(matches!(update_target(&input(json!({"update_id":"11111111-1111-1111-1111-111111111111","revision":revision})),&context()),ActionTarget::Unassigned{..}));
        }
    }
    #[test] fn default_false_does_not_invent_cleaning_failure_and_backdated_response_is_rejected() {
        let mut i=input(json!({"DetectionID":"11111111-1111-1111-1111-111111111111","ThreatID":1,"ActionSuccess":false,"ThreatStatusID":1,"InitialDetectionTime":"2026-10-21T00:00:00Z"}));
        let r=detections(&i,&context());assert_eq!(r.actions.len(),1);assert_eq!(r.actions[0].result,ActionOutcome::Observed);
        i.payload["ThreatStatusID"]=json!(102);i.payload["LastThreatStatusChangeTime"]=json!("2026-10-20T00:00:00Z");assert_eq!(detections(&i,&context()).actions.len(),1);
        i.payload["LastThreatStatusChangeTime"]=json!("2026-10-21T00:01:00Z");let r=detections(&i,&context());assert_eq!(r.actions[1].result,ActionOutcome::Failed);
        let mut other=i.clone();other.payload["ThreatID"]=json!(2);assert_ne!(r.actions[0].target,detections(&other,&context()).actions[0].target);
        i.payload["ThreatStatusID"]=json!(106);assert_eq!(detections(&i,&context()).actions[1].result,ActionOutcome::Observed);
    }
}
