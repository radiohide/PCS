//! One indexed pass per retained-history source; no full raw-history scan or new collector.
use anyhow::Result;
use pcscope_contracts::adapter::AdapterContext;
use rusqlite::{params,Connection,OptionalExtension};
use serde_json::{json,Value};
use std::time::{Duration,Instant};

pub(crate) fn advance(conn:&Connection,limit:usize,budget:Duration)->Result<Value> {
    let started=Instant::now();
    let environment:Option<(String,String)>=conn.query_row("SELECT m.value,i.value FROM settings m JOIN settings i ON i.key='projection_installation' WHERE m.key='projection_machine'",[],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    let Some((machine,installation))=environment else{return Ok(json!({"pending":false,"waiting":"projection_environment"}))};
    let mut cache=crate::evidence::SnapshotCache::default();
    for source in ["pcscope.system.update-history","pcscope.security.detections"] {
        let rule=format!("retained-history-v1/{source}");
        conn.execute("INSERT OR IGNORE INTO association_repair_state(rule,cursor,bound) SELECT ?,0,coalesce(max(seq),0) FROM evidence WHERE source=?",params![rule,source])?;
        let (cursor,bound):(i64,i64)=conn.query_row("SELECT cursor,bound FROM association_repair_state WHERE rule=?",[&rule],|r|Ok((r.get(0)?,r.get(1)?)))?;
        if cursor>=bound {continue}
        let rows={let mut q=conn.prepare_cached("SELECT seq,evidence_id,item_id FROM evidence WHERE source=? AND seq>? AND seq<=? ORDER BY seq LIMIT ?")?;
            let rows=q.query_map(params![source,cursor,bound,limit],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
        let mut last=cursor;let mut examined=0;let mut actions=0;let mut missing=0;
        for (seq,address,item) in &rows {
            if examined>0&&started.elapsed()>=budget {break}
            if let Some(input)=crate::evidence::input_at_cached(conn,address,item,&mut cache)? {
                // Derived snapshot appearance/absence records are not original retained items.
                if input.evidence.kind==pcscope_contracts::adapter::EvidenceKind::Snapshot {
                    let context=AdapterContext{catalog_version:pcscope_contracts::model::catalog_version().into(),rule_version:rule.clone(),machine_id:machine.clone(),installation_id:installation.clone(),boot_id:None,collected_scope:pcscope_contracts::semantics::business_scope(&input.source_scope),complete:true};
                    let adapter=pcscope_contracts::adapters::adapter_for(source).unwrap();let result=adapter.adapt(&input,&context);
                    anyhow::ensure!(pcscope_contracts::adapter::validate(&result,&context).is_empty(),"retained history contract failed");
                    for action in &result.actions {crate::projection::record_action(conn,&input,&context,action)?;actions+=1;}
                }
            }else{missing+=1;}
            last=*seq;examined+=1;
        }
        if rows.is_empty(){last=bound;}
        conn.execute("UPDATE association_repair_state SET cursor=?,examined=examined+?,repaired=repaired+?,unresolved=unresolved+? WHERE rule=?",params![last,examined,actions,missing,rule])?;
        return Ok(json!({"rule":rule,"cursor":last,"bound":bound,"pending":true,"examined":examined,"actions_processed":actions,"material_unavailable":missing,"elapsed_ms":started.elapsed().as_millis()}));
    }
    crate::process_history_repair::advance(conn,limit,budget)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Store;
    fn snapshot(source:&str,run:&str,items:Value)->pcscope_contracts::Envelope {
        pcscope_contracts::parse(&json!({"protocol_version":1,"request_id":run,"type":"snapshot","source":source,"scope":"machine:M/user:U","run_id":run,"observed_at":"2026-10-22T00:00:00Z","complete":true,"items":items}).to_string()).unwrap()
    }
    fn drain(store:&Store) {while store.project_pending("M","I",100).unwrap().examined>0 {}}
    fn count(store:&Store,sql:&str)->i64 {store.conn.query_row(sql,[],|r|r.get(0)).unwrap()}
    #[test]
    fn retained_updates_link_history_replay_without_current_installation() {
        let mut store=Store::open(":memory:").unwrap();
        store.conn.execute("INSERT INTO settings VALUES('projection_machine','M'),('projection_installation','I')",[]).unwrap();
        let payload=json!({"update_id":"11111111-1111-1111-1111-111111111111","revision":1,"date":"2026-10-10T00:00:00Z","operation":1,"result":4,"title":"测试更新"});
        let mut retry=payload.clone();retry["date"]=json!("2026-10-11T00:00:00Z");retry["result"]=json!(2);
        let mut uninstall=retry.clone();uninstall["date"]=json!("2026-10-12T00:00:00Z");uninstall["operation"]=json!(2);
        let items=json!([{"id":"attempt-1","data":payload},{"id":"attempt-2","data":retry},{"id":"uninstall","data":uninstall}]);
        store.ingest(&snapshot("pcscope.system.update-history","r1",items.clone())).unwrap();drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE type_id='software.update_history_subject' AND lifecycle='not_observed'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE type_id='software.system_update'"),0);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='software.update_history' AND instance_id IS NOT NULL"),3);
        let rows:Vec<i64>=store.conn.prepare("SELECT action_row FROM object_actions ORDER BY action_row").unwrap().query_map([],|r|r.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        // Simulate the old adapter's independent links; repair must reuse all action IDs.
        store.conn.execute("UPDATE object_actions SET instance_id=NULL,target_type='software.system_update',unassigned_reason='legacy independent history' WHERE action_id='software.update_history'",[]).unwrap();
        while store.repair_retained_history(2,Duration::from_secs(1)).unwrap()["pending"]==true {}
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='software.update_history' AND instance_id IS NOT NULL"),3);
        store.ingest(&snapshot("pcscope.system.update-history","r2",items)).unwrap();drain(&store);
        store.ingest(&snapshot("pcscope.system.update-history","empty",json!([]))).unwrap();drain(&store);
        let after:Vec<i64>=store.conn.prepare("SELECT action_row FROM object_actions ORDER BY action_row").unwrap().query_map([],|r|r.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        assert_eq!(after,rows);assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE lifecycle='not_observed'"),1);
        assert_eq!(store.repair_retained_history(2,Duration::from_secs(1)).unwrap()["pending"],false);
    }
    #[test]
    fn conflicting_update_identity_is_not_resolved_by_replaying_either_item() {
        let mut store=Store::open(":memory:").unwrap();
        let original=json!({"update_id":"11111111-1111-1111-1111-111111111111","revision":1,"date":"2026-10-10T00:00:00Z","operation":1,"result":4,"title":"测试更新"});
        let mut conflicting=original.clone();conflicting["update_id"]=json!("22222222-2222-2222-2222-222222222222");
        for (run,payload) in [("original",original.clone()),("conflicting",conflicting.clone()),("original-replay",original),("conflicting-replay",conflicting)] {
            store.ingest(&snapshot("pcscope.system.update-history",run,json!([{"id":"same-occurrence","data":payload}]))).unwrap();drain(&store);
        }
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE type_id='software.update_history_subject'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM event_associations WHERE decision='mismatch' AND rule='retained-history-conflict-v1'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM association_repair_audit WHERE rule='retained-history-conflict-v1'"),1);
        for run in ["original","conflicting","original-replay","conflicting-replay"] {
            let reference=pcscope_contracts::adapter::EvidenceRef::snapshot("pcscope.system.update-history","machine:M/user:U",run);
            assert!(store.conn.query_row("SELECT EXISTS(SELECT 1 FROM evidence_refs WHERE subject_kind='action' AND evidence_id=?)",[reference.id],|r|r.get::<_,bool>(0)).unwrap(),"missing retained citation for {run}");
        }
    }
    #[test]
    fn defender_repeats_do_not_repeat_detection_and_retention_does_not_clear_threat() {
        let mut store=Store::open(":memory:").unwrap();
        let mut payload=json!({"DetectionID":"{22222222-2222-2222-2222-222222222222}","ThreatID":123,"InitialDetectionTime":"2026-10-10T00:00:00Z","ThreatStatusID":1,"ActionSuccess":false});
        store.ingest(&snapshot("pcscope.security.detections","detected",json!([{"id":"id","data":payload}]))).unwrap();drain(&store);
        payload["LastThreatStatusChangeTime"]=json!("2026-10-10T00:01:00Z");payload["ThreatStatusID"]=json!(3);payload["ActionSuccess"]=json!(true);
        for run in ["isolated","repeated"] {store.ingest(&snapshot("pcscope.security.detections",run,json!([{"id":"id","data":payload}]))).unwrap();drain(&store);}
        store.ingest(&snapshot("pcscope.security.detections","gone",json!([]))).unwrap();drain(&store);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='security.threat_detected'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id='security.threat_status_recorded' AND result='succeeded'"),1);
        assert_eq!(count(&store,"SELECT count(*) FROM object_actions WHERE action_id LIKE 'observed.%'"),0);
        assert_eq!(count(&store,"SELECT count(*) FROM object_instances WHERE type_id='security.detection_history' AND lifecycle='not_observed'"),1);
    }
}
