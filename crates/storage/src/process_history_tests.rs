use crate::Store;
use serde_json::{json,Value};
use std::time::Duration;
fn event(store:&mut Store,source:&str,id:u32,kind:&str,at:&str,payload:Value) {
    store.ingest(&pcscope_contracts::parse(&json!({"protocol_version":1,"request_id":id.to_string(),"type":"event_batch","source":source,"epoch":"epoch-one","batch_seq":id,"checkpoint":id.to_string(),"events":[{"id":id.to_string(),"object_id":id.to_string(),"kind":kind,"occurred_at":at,"data":payload}]}).to_string()).unwrap()).unwrap();
    while store.project_pending("M","I",64).unwrap().examined>0{}
}
fn security()->Value {json!({"provider":"Microsoft-Windows-Security-Auditing","channel":"Security","event_id":4688,"system":{"Version":{"value":"2"}},"source_time_available":true,"fields":{"NewProcessId":"0x20","NewProcessName":"C:\\Example\\example.exe","ProcessId":"0x40"}})}
fn count(s:&Store,sql:&str)->i64{s.reader().query_row(sql,[],|r|r.get(0)).unwrap()}
#[test]
fn security_starts_are_distinct_history_and_repair_keeps_action_ids_without_inventing_creation_time() {
    let mut s=Store::open(":memory:").unwrap();s.projection_installation("M").unwrap();
    s.reader().execute("UPDATE settings SET value='I' WHERE key='projection_installation'",[]).unwrap();
    for id in [1,2] {event(&mut s,"pcscope.eventlog.security",id,"execution.process_started","2026-10-21T08:00:00Z",security())}
    assert_eq!(count(&s,"SELECT count(*) FROM object_instances WHERE type_id='runtime.process' AND lifecycle='not_observed' AND identity_status='confirmed'"),2);
    assert_eq!(count(&s,"SELECT count(*) FROM object_facts WHERE field_id='created_at'"),0);
    assert_eq!(count(&s,"SELECT count(*) FROM object_facts WHERE field_id='pid' AND value='\"32\"'"),2);
    let before:Vec<i64>=s.reader().prepare("SELECT instance_id FROM object_actions ORDER BY action_row").unwrap().query_map([],|r|r.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
    s.reader().execute("UPDATE object_actions SET instance_id=NULL,unassigned_reason='old incomplete key'",[]).unwrap();
    while s.repair_process_history(1,Duration::from_secs(1)).unwrap()["pending"]==true {}
    let after:Vec<i64>=s.reader().prepare("SELECT instance_id FROM object_actions ORDER BY action_row").unwrap().query_map([],|r|r.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
    assert_eq!(before,after);
    event(&mut s,"pcscope.eventlog.security",2,"execution.process_started","2026-10-21T08:00:00Z",security());
    assert_eq!(count(&s,"SELECT count(*) FROM object_actions"),2);
    assert_eq!(count(&s,"SELECT count(*) FROM event_associations WHERE decision='verified'"),2);
    let detail=s.query(&pcscope_contracts::query::QueryRequest::ActionDetail{action_row:1}).unwrap().data.unwrap();
    assert_eq!(detail["context"]["attribution_status"],"verified", "{detail}");
}
#[test]
fn process_header_pid_and_protection_failures_do_not_create_history_subjects() {
    for fault in ["version","provider","channel","nested_id","pid","duplicate","protected","time","import"] {
        let mut s=Store::open(":memory:").unwrap();let mut p=security();
        match fault {
            "version"=>p["system"]["Version"]["value"]=json!(99),"provider"=>p["provider"]=json!("Other"),"channel"=>p["channel"]=json!("Other"),
            "nested_id"=>p["system"]["EventID"]=json!({"value":"999"}),"pid"=>p["fields"]["NewProcessId"]=json!("0"),
            "duplicate"=>p["fields"]["NewProcessId#2"]=json!("32"),"protected"=>p["fields"]["NewProcessId"]=json!({"sensitive":true}),
            "time"=>p["source_time_available"]=json!(false),_=>{}
        }
        event(&mut s,if fault=="import"{"pcscope.eventlog.import.test"}else{"pcscope.eventlog.security"},1,"execution.process_started","2026-10-21T08:00:00Z",p);
        assert_eq!(count(&s,"SELECT count(*) FROM object_instances"),0,"{fault}");
        assert_eq!(count(&s,"SELECT count(*) FROM object_actions"),1,"event remains visible: {fault}");
    }
}
#[test]
fn process_supported_versions_and_conflicting_history_are_explicit() {
    let mut s=Store::open(":memory:").unwrap();s.projection_installation("M").unwrap();
    s.reader().execute("UPDATE settings SET value='I' WHERE key='projection_installation'",[]).unwrap();
    for version in 0..=2 {
        let mut p=security();p["system"]["Version"]["value"]=json!(version.to_string());
        event(&mut s,"pcscope.eventlog.security",version+1,"execution.process_started","2026-10-21T08:00:00Z",p);
    }
    assert_eq!(count(&s,"SELECT count(*) FROM object_actions WHERE instance_id IS NOT NULL"),3);
    s.reader().execute("INSERT INTO evidence_refs(subject_kind,subject_id,evidence_kind,evidence_id) SELECT subject_kind,'1',evidence_kind,evidence_id FROM evidence_refs WHERE subject_kind='action' AND subject_id='2'",[]).unwrap();
    s.reader().execute("UPDATE object_actions SET instance_id=NULL,unassigned_reason='old independent action' WHERE action_row=1",[]).unwrap();
    while s.repair_process_history(1,Duration::from_secs(1)).unwrap()["pending"]==true {}
    assert_eq!(crate::event_association::stored(s.reader(),1,None).unwrap().unwrap()["status"],"mismatch");
    assert_eq!(count(&s,"SELECT count(*) FROM object_instances"),3);
    assert_eq!(count(&s,"SELECT count(*) FROM object_actions WHERE action_row=1 AND instance_id IS NULL"),1);
}
#[test]
fn sysmon_guid_links_start_and_stop_even_out_of_order_and_pid_reuse_is_separate() {
    let mut s=Store::open(":memory:").unwrap();let source="pcscope.eventlog.microsoft-windows-sysmon/operational";
    for (record,id,version,guid,at) in [(1,5,3,"11111111-1111-1111-1111-111111111111","2026-10-21T08:02:00Z"),(2,1,5,"11111111-1111-1111-1111-111111111111","2026-10-21T08:00:00Z"),(3,1,5,"22222222-2222-2222-2222-222222222222","2026-10-21T08:03:00Z")] {
        event(&mut s,source,record,"execution.sysmon_event",at,json!({"provider":"Microsoft-Windows-Sysmon","channel":"Microsoft-Windows-Sysmon/Operational","event_id":id,"version":version,"fields":{"ProcessId":"32","ProcessGuid":guid,"Image":"C:\\Example\\example.exe"}}));
    }
    assert_eq!(count(&s,"SELECT count(DISTINCT instance_id) FROM object_actions"),2);
    assert_eq!(count(&s,"SELECT count(*) FROM object_actions WHERE instance_id=(SELECT instance_id FROM object_actions WHERE action_id='execution.process_stopped')"),2);
    assert_eq!(count(&s,"SELECT count(*) FROM object_facts WHERE field_id='created_at'"),0);
    assert_eq!(count(&s,"SELECT count(*) FROM object_facts WHERE field_id='ended_at'"),1);
    assert_eq!(count(&s,"SELECT count(*) FROM object_instances WHERE lifecycle='active'"),0);
}
#[test]
fn group_member_events_include_group_and_never_use_actor_sid() {
    let mut s=Store::open(":memory:").unwrap();
    for (record,group,member) in [(1,"S-1-5-32-544",json!("S-1-5-21-1")),(2,"S-1-5-32-545",json!("S-1-5-21-1")),(3,"S-1-5-32-544",json!("S-1-0-0")),(4,"S-1-5-32-544",json!({"sensitive":true}))] {
        event(&mut s,"pcscope.eventlog.security",record,"identity.group_member_added","2026-10-21T08:00:00Z",json!({"provider":"Microsoft-Windows-Security-Auditing","channel":"Security","event_id":4732,"version":0,"fields":{"TargetSid":group,"MemberSid":member,"SubjectUserSid":"S-1-5-18"}}));
    }
    assert_eq!(count(&s,"SELECT count(DISTINCT instance_id) FROM object_actions"),2);
    assert_eq!(count(&s,"SELECT count(*) FROM object_actions WHERE instance_id IS NULL"),2);
    assert_eq!(count(&s,"SELECT count(*) FROM object_instances WHERE native_key LIKE '%S-1-5-18%'"),0);
}
#[test]
fn membership_pairs_and_unknown_versions_preserve_action_without_false_identity() {
    let mut s=Store::open(":memory:").unwrap();
    for (index,id) in [4728,4729,4732,4733,4756,4757].into_iter().enumerate() {
        for version in [0,99] {
            let kind=if id%2==0{"identity.group_member_added"}else{"identity.group_member_removed"};
            event(&mut s,"pcscope.eventlog.security",(index*2+if version==0{1}else{2}) as u32,kind,"2026-10-21T08:00:00Z",json!({"provider":"Microsoft-Windows-Security-Auditing","channel":"Security","event_id":id,"version":version,"fields":{"TargetSid":format!("S-1-5-21-{}",index+1),"MemberSid":"S-1-5-21-999"}}));
        }
    }
    assert_eq!(count(&s,"SELECT count(*) FROM object_actions WHERE instance_id IS NOT NULL"),6);
    assert_eq!(count(&s,"SELECT count(*) FROM object_actions WHERE instance_id IS NULL"),6);
}
