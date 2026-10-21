use super::*;
use crate::Store;
use pcscope_contracts::parse;
use pcscope_contracts::adapter::{Decoding,EvidenceKind,EvidenceRef};
use std::time::Duration;

fn context()->AdapterContext {AdapterContext{catalog_version:pcscope_contracts::model::catalog_version().into(),rule_version:"test".into(),machine_id:"M".into(),installation_id:"I".into(),boot_id:None,collected_scope:"machine:M".into(),complete:true}}
fn boot(store:&Store,id:&str,start:&str,until:&str) {
    let input=EvidenceInput{source:"pcscope.system.boot-state".into(),item_id:"boot-state".into(),structure_version:"v1".into(),source_scope:"machine:M".into(),
        evidence:EvidenceRef{kind:EvidenceKind::Snapshot,id:format!("boot:{id}:{until}")},native_record_id:"boot-state".into(),source_time:None,observed_at:until.into(),ingested_at:until.into(),decoding:Decoding{available:true,..Default::default()},
        payload:json!({"boot_id":id,"boot_id_basis":"SystemBootEnvironmentInformation","last_boot_time":start})};
    observe_boot(store.reader(),&input,&context()).unwrap();
}
fn event(store:&mut Store,id:u32,kind:&str,fields:Value,at:&str) {
    store.ingest(&parse(&json!({"protocol_version":1,"request_id":format!("{id}"),"type":"event_batch","source":"pcscope.eventlog.security","epoch":"one","batch_seq":id,"checkpoint":format!("{id}"),
      "events":[{"id":format!("{id}"),"object_id":format!("{id}"),"kind":kind,"occurred_at":at,"data":{"provider":"Microsoft-Windows-Security-Auditing","fields":fields}}]}).to_string()).unwrap()).unwrap();
    while store.project_pending("M","I",64).unwrap().examined>0{}
}
const A:&str="11111111-1111-1111-1111-111111111111";
const B:&str="22222222-2222-2222-2222-222222222222";

#[test]
fn boot_identity_is_stable_despite_clock_jitter_and_never_invented_for_older_events() {
    let store=Store::open(":memory:").unwrap();
    boot(&store,A,"2026-10-21T00:00:00.001Z","2026-10-21T04:00:00Z");
    boot(&store,A,"2026-10-21T00:00:00.002Z","2026-10-21T05:00:00Z");
    let c=store.reader();
    assert_eq!(boot_at(c,"M","I","2026-10-21T03:00:00Z").unwrap().as_deref(),Some(A));
    for (machine,installation,at) in [("M","I","2026-10-20T23:59:59Z"),("M","I","2026-10-21T06:00:00Z"),("X","I","2026-10-21T03:00:00Z"),("M","X","2026-10-21T03:00:00Z")] {assert!(boot_at(c,machine,installation,at).unwrap().is_none());}
    boot(&store,B,"2026-10-21T04:30:00Z","2026-10-21T07:00:00Z");
    assert!(boot_at(c,"M","I","2026-10-21T04:45:00Z").unwrap().is_none(),"overlap is ambiguous");
    assert_eq!(boot_at(c,"M","I","2026-10-21T06:00:00Z").unwrap().as_deref(),Some(B));
    assert!(boot_identifier(&json!({"boot_id":A})).is_none());
    boot(&store,B,"2026-10-21T03:00:00Z","2026-10-21T08:00:00Z");
    assert!(boot_at(c,"M","I","2026-10-21T06:00:00Z").unwrap().is_none(),"clock discontinuity must not expand historical attribution");
}

#[test]
fn same_logon_number_in_two_boots_is_two_objects_and_pruning_does_not_erase_proof() {
    let mut store=Store::open(":memory:").unwrap();
    boot(&store,A,"2026-10-20T00:00:00Z","2026-10-20T23:00:00Z");
    boot(&store,B,"2026-10-21T00:00:00Z","2026-10-21T23:00:00Z");
    event(&mut store,1,"identity.special_privileges",json!({"SubjectLogonId":"0x3e7"}),"2026-10-20T08:00:00Z");
    event(&mut store,2,"identity.special_privileges",json!({"SubjectLogonId":"0x3e7"}),"2026-10-21T08:00:00Z");
    let c=store.reader();assert_eq!(c.query_row("SELECT count(DISTINCT instance_id) FROM object_actions",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    let id:i64=c.query_row("SELECT instance_id FROM object_actions WHERE action_row=1",[],|r|r.get(0)).unwrap();
    c.execute("UPDATE evidence SET pruned_at='2026-10-22T00:00:00Z'",[]).unwrap();
    assert_eq!(stored(c,1,Some(id)).unwrap().unwrap()["status"],"verified");
    c.execute("UPDATE object_instances SET native_key='changed' WHERE instance_id=?",[id]).unwrap();
    assert!(stored(c,1,Some(id)).unwrap().is_none());
}

#[test]
fn missing_process_identity_is_independent_not_a_shared_placeholder() {
    let mut store=Store::open(":memory:").unwrap();
    for id in 1..=2 {event(&mut store,id,"execution.process_started",json!({"NewProcessId":"0x20","NewProcessName":"example.exe"}),"2026-10-21T08:00:00Z");}
    let c=store.reader();assert_eq!(c.query_row("SELECT count(*) FROM object_instances WHERE type_id='runtime.process'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    assert_eq!(c.query_row("SELECT count(*) FROM object_actions WHERE instance_id IS NULL",[],|r|r.get::<_,i64>(0)).unwrap(),2);
}

#[test]
fn historical_session_repair_is_bounded_idempotent_and_keeps_action_identity() {
    let mut store=Store::open(":memory:").unwrap();let install=store.projection_installation("M").unwrap();
    event(&mut store,1,"identity.special_privileges",json!({"SubjectLogonId":"0x3e7"}),"2026-10-21T08:00:00Z");
    boot(&store,A,"2026-10-21T00:00:00Z","2026-10-21T23:00:00Z");
    store.reader().execute("UPDATE settings SET value='I' WHERE key='projection_installation'",[]).unwrap();assert!(!install.is_empty());
    let before:(i64,String)=store.reader().query_row("SELECT action_row,occurred_at FROM object_actions",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    let result=store.repair_event_context(1,Duration::from_secs(1)).unwrap();assert_eq!(result["repaired"],1);
    let c=store.reader();let after:(i64,String)=c.query_row("SELECT action_row,occurred_at FROM object_actions",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();assert_eq!(before,after);
    assert_eq!(c.query_row("SELECT lifecycle FROM object_instances",[],|r|r.get::<_,String>(0)).unwrap(),"not_observed");
    assert_eq!(store.repair_event_context(1,Duration::from_secs(1)).unwrap()["batch_repaired"],0);
    c.execute("UPDATE projection_queue SET state='pending'",[]).unwrap();
    while store.project_pending("M","I",64).unwrap().examined>0{}
    assert_eq!(store.reader().query_row("SELECT count(*) FROM object_actions",[],|r|r.get::<_,i64>(0)).unwrap(),1);
}

#[test]
fn local_account_alias_requires_host_time_unique_sid_and_never_uses_the_actor() {
    let mut store=Store::open(":memory:").unwrap();
    store.reader().execute_batch("INSERT INTO object_instances(instance_id,type_id,category,machine_id,installation_id,scope,native_key,incarnation,identity_status,lifecycle,first_observed_at,last_observed_at)
      VALUES(1,'account.user','account','M','I','machine:M','machine:m|s-1-5-21-1-2-3-1001',1,'confirmed','active','2026-10-20T00:00:00Z','2026-10-22T00:00:00Z'),
      (2,'account.machine_identity','account','M','I','machine:M','machine:M|host',1,'confirmed','active','2026-10-20T00:00:00Z','2026-10-22T00:00:00Z');
      INSERT INTO object_facts(instance_id,field_id,field_kind,value,value_type,value_status,observed_at,derivation,rule_version) VALUES
      (1,'name','configuration','\"radiohide\"','string','known','2026-10-22T00:00:00Z','source_provided','test'),
      (2,'name','configuration','\"PC\"','string','known','2026-10-22T00:00:00Z','source_provided','test');
      INSERT INTO source_bindings(instance_id,source,item_id,scope,native_record_id,valid_from) VALUES(1,'pcscope.identity.accounts','S-1-5-21-1-2-3-1001','machine:M','SID','2026-10-20T00:00:00Z');").unwrap();
    event(&mut store,1,"identity.logon_failed",json!({"TargetUserName":"radiohide","TargetDomainName":"PC","TargetUserSid":"S-1-0-0"}),"2026-10-21T08:00:00Z");
    assert_eq!(store.reader().query_row("SELECT instance_id FROM object_actions WHERE action_row=1",[],|r|r.get::<_,i64>(0)).unwrap(),1);
    event(&mut store,2,"identity.logon_failed",json!({"TargetUserName":"radiohide","TargetDomainName":"DOMAIN"}),"2026-10-21T08:00:00Z");
    event(&mut store,3,"identity.logon_failed",json!({"SubjectUserName":"radiohide","TargetDomainName":"PC"}),"2026-10-21T08:00:00Z");
    event(&mut store,4,"identity.logon_failed",json!({"TargetUserName":"radiohide","TargetDomainName":"PC"}),"2026-10-19T08:00:00Z");
    assert_eq!(store.reader().query_row("SELECT count(*) FROM object_actions WHERE instance_id IS NULL",[],|r|r.get::<_,i64>(0)).unwrap(),3);
    store.reader().execute_batch("INSERT INTO fact_history(instance_id,field_id,before_value,before_status,after_value,after_status,observed_from,observed_until,source,rule_version) VALUES(1,'name','\"previous\"','known','\"radiohide\"','known','2026-10-20T00:00:00Z','2026-10-21T12:00:00Z','native','test');").unwrap();
    event(&mut store,5,"identity.logon_failed",json!({"TargetUserName":"radiohide","TargetDomainName":"PC"}),"2026-10-21T08:00:00Z");
    assert!(store.reader().query_row("SELECT instance_id FROM object_actions WHERE action_row=5",[],|r|r.get::<_,Option<i64>>(0)).unwrap().is_none());
}

#[test]
fn session_snapshot_decimal_id_and_event_hex_id_agree_within_boot() {
    use pcscope_contracts::adapter::SourceAdapter;
    let mut ctx=context();ctx.boot_id=Some(A.into());
    let input=EvidenceInput{source:"pcscope.identity.logon-sessions".into(),item_id:"999".into(),structure_version:"v1".into(),source_scope:"machine:M".into(),evidence:EvidenceRef{kind:EvidenceKind::Snapshot,id:"snapshot".into()},native_record_id:"999".into(),source_time:None,observed_at:"2026-10-21T08:00:00Z".into(),ingested_at:"2026-10-21T08:00:00Z".into(),decoding:Decoding{available:true,..Default::default()},payload:json!({"LogonId":"999"})};
    let snapshot=pcscope_contracts::reference::adapter_for(&input.source).unwrap().adapt(&input,&ctx);
    let mut event=input.clone();event.source="pcscope.eventlog.security".into();event.item_id="identity.special_privileges".into();event.evidence.kind=EvidenceKind::Event;event.payload=json!({"fields":{"SubjectLogonId":"0x3e7"}});
    let result=pcscope_contracts::adapters::event::EventAdapter.adapt(&event,&ctx);
    let ActionTarget::Assigned{identity}=&result.actions[0].target else{panic!("expected scoped session")};
    let strategy=pcscope_contracts::identity::TypeStrategy::for_type("runtime.logon_session").unwrap();
    assert_eq!(pcscope_contracts::identity::normalise(&strategy,identity),pcscope_contracts::identity::normalise(&strategy,&snapshot.observations[0].identity));
}

#[test]
fn late_boot_observation_retries_only_waiting_events_without_rescanning_history() {
    let mut store=Store::open(":memory:").unwrap();store.projection_installation("M").unwrap();
    store.reader().execute("UPDATE settings SET value='I' WHERE key='projection_installation'",[]).unwrap();
    boot(&store,A,"2026-10-21T00:00:00Z","2026-10-21T08:00:00Z");
    event(&mut store,1,"identity.special_privileges",json!({"SubjectLogonId":"0x11"}),"2026-10-21T07:00:00Z");
    assert_eq!(store.repair_event_context(10,Duration::from_secs(1)).unwrap()["pending"],false);
    event(&mut store,2,"identity.special_privileges",json!({"SubjectLogonId":"0x22"}),"2026-10-21T09:00:00Z");
    assert_eq!(store.repair_event_context(10,Duration::from_secs(1)).unwrap()["retry_examined"],0);
    boot(&store,A,"2026-10-21T00:00:00Z","2026-10-21T10:00:00Z");
    let result=store.repair_event_context(10,Duration::from_secs(1)).unwrap();assert_eq!(result["retry_examined"],1);assert_eq!(result["cursor"],1);
    assert_eq!(store.repair_event_context(10,Duration::from_secs(1)).unwrap()["retry_examined"],0);
    assert_eq!(store.reader().query_row("SELECT count(*) FROM object_actions WHERE instance_id IS NOT NULL",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    assert_eq!(store.reader().query_row("SELECT count(*) FROM event_association_retry_audit",[],|r|r.get::<_,i64>(0)).unwrap(),1);
}

#[test]
fn conflicting_tenth_evidence_revokes_proof_without_moving_the_historical_link() {
    let mut store=Store::open(":memory:").unwrap();boot(&store,A,"2026-10-21T00:00:00Z","2026-10-21T23:00:00Z");
    for id in 1..=10 {event(&mut store,id,"identity.special_privileges",json!({"SubjectLogonId":if id==10{"0x22"}else{"0x11"}}),&format!("2026-10-21T{id:02}:00:00Z"));}
    let c=store.reader();let original:i64=c.query_row("SELECT instance_id FROM object_actions WHERE action_row=1",[],|r|r.get(0)).unwrap();
    c.execute_batch("INSERT OR IGNORE INTO evidence_refs SELECT 'action','1',evidence_kind,evidence_id,pruned_at FROM evidence_refs WHERE subject_kind='action' AND CAST(subject_id AS INTEGER) BETWEEN 2 AND 10;").unwrap();
    let result=store.repair_event_context(100,Duration::from_secs(2)).unwrap();assert_eq!(result["unresolved"],1);
    assert_eq!(c.query_row("SELECT instance_id FROM object_actions WHERE action_row=1",[],|r|r.get::<_,i64>(0)).unwrap(),original);
    assert_eq!(stored(c,1,Some(original)).unwrap().unwrap()["status"],"mismatch");
}

#[test]
fn incomplete_secondary_evidence_keeps_its_replay_claim_after_repair() {
    let mut store=Store::open(":memory:").unwrap();boot(&store,A,"2026-10-21T00:00:00Z","2026-10-21T23:00:00Z");
    event(&mut store,1,"identity.special_privileges",json!({"SubjectLogonId":"0x11"}),"2026-10-21T07:00:00Z");
    event(&mut store,2,"identity.special_privileges",json!({}),"2026-10-21T07:00:00Z");
    let c=store.reader();c.execute_batch("INSERT OR IGNORE INTO evidence_refs SELECT 'action','1',evidence_kind,evidence_id,pruned_at FROM evidence_refs WHERE subject_kind='action' AND subject_id='2';
      DELETE FROM object_actions WHERE action_row=2;
      DELETE FROM evidence_refs WHERE subject_kind='action' AND subject_id NOT IN ('1','2');").unwrap();
    assert_eq!(store.repair_event_context(100,Duration::from_secs(1)).unwrap()["unresolved"],0);
    c.execute("UPDATE projection_queue SET state='pending'",[]).unwrap();
    while store.project_pending("M","I",64).unwrap().examined>0{}
    assert_eq!(store.reader().query_row("SELECT count(*) FROM object_actions",[],|r|r.get::<_,i64>(0)).unwrap(),1);
}
