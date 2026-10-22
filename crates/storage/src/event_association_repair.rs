//! One bounded history pass: prove stored links with the same context as new events.
use anyhow::Result;
use pcscope_contracts::{adapter::{AdapterContext,ActionTarget,SourceAdapter},identity::{IdentityRequest,TypeStrategy,SourceRecordRef,EvidenceTime,LifecycleHints}};
use rusqlite::{params,Connection,OptionalExtension};
use serde_json::{json,Value};
use std::time::{Duration,Instant};

const RULE:&str="event-context-v1";
pub(crate) fn advance(conn:&Connection,limit:usize,budget:Duration)->Result<Value> {
    // Other sources continue even when boot collection is disabled. Sessions without a
    // proven boot enter the indexed waiting set and are retried when evidence arrives.
    conn.execute("INSERT OR IGNORE INTO association_repair_state(rule,cursor,bound) SELECT ?,0,coalesce(max(action_row),0) FROM object_actions",[RULE])?;
    let (cursor,bound):(i64,i64)=conn.query_row("SELECT cursor,bound FROM association_repair_state WHERE rule=?",[RULE],|r|Ok((r.get(0)?,r.get(1)?)))?;
    if cursor>=bound{return retry_waiting(conn,limit,budget)}
    let started=Instant::now();
    let rows={let mut q=conn.prepare("SELECT action_row,instance_id,action_id FROM object_actions WHERE action_row>? AND action_row<=? ORDER BY action_row LIMIT ?")?;
        let rows=q.query_map(params![cursor,bound,limit],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,Option<u64>>(1)?,r.get::<_,String>(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
    let mut last=cursor;let mut examined=0;let mut repaired=0;let mut unresolved=0;
    for (row,old,action) in &rows {
        if last!=cursor && started.elapsed()>=budget{break}
        if !action.starts_with("observed.") && conn.query_row("SELECT EXISTS(SELECT 1 FROM evidence_refs WHERE subject_kind='action' AND subject_id=? AND evidence_kind='event')",[row.to_string()],|r|r.get::<_,bool>(0))? {
            examined+=1;
            conn.execute_batch("SAVEPOINT event_repair_candidate")?;
            let outcome=repair(conn,*row,*old,action)?;
            if outcome.is_none() {
                let detail:String=conn.query_row("SELECT detail FROM association_repair_audit WHERE rule=? AND action_row=?",params![RULE,row],|r|r.get(0))?;
                conn.execute_batch("ROLLBACK TO event_repair_candidate")?;
                audit(conn,*row,*old,*old,"unresolved",serde_json::from_str(&detail)?)?;
                // Conflict in the ninth or later item must not be hidden by the reader's
                // bounded label extraction. Keep the old link only as an audit reference.
                if detail.contains("conflicting_identities") {
                    conn.execute("DELETE FROM event_associations WHERE action_row=?",[row])?;
                    conn.execute("INSERT INTO event_associations SELECT ?,?,json_array(type_id,category,machine_id,installation_id,scope,native_key,incarnation,identity_status),'mismatch','原始记录指向不同对象。','{}',? FROM object_instances WHERE instance_id=?",params![row,old,RULE,old])?;
                }
            }
            conn.execute_batch("RELEASE event_repair_candidate")?;
            match outcome{Some(true)=>repaired+=1,None=>unresolved+=1,_=>{}}
        }
        last=*row;
    }
    if rows.is_empty(){last=bound;}
    conn.execute("UPDATE association_repair_state SET cursor=?,examined=examined+?,repaired=repaired+?,unresolved=unresolved+? WHERE rule=?",params![last,examined,repaired,unresolved,RULE])?;
    let totals=conn.query_row("SELECT examined,repaired,unresolved FROM association_repair_state WHERE rule=?",[RULE],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?)))?;
    Ok(json!({"rule":RULE,"cursor":last,"bound":bound,"pending":last<bound,"examined":totals.0,"repaired":totals.1,"unresolved":totals.2,"batch_repaired":repaired,"elapsed_ms":started.elapsed().as_millis()}))
}
fn retry_waiting(conn:&Connection,limit:usize,budget:Duration)->Result<Value> {
    let started=Instant::now();
    let rows={let mut q=conn.prepare("SELECT w.action_row,a.instance_id,a.action_id,max(b.observed_until) FROM installation_boots b CROSS JOIN event_association_waiting w ON w.machine_id=b.machine_id AND w.installation_id=b.installation_id AND w.source_time>=b.start_at AND w.source_time<=b.observed_until JOIN object_actions a ON a.action_row=w.action_row WHERE b.clock_consistent=1 AND b.observed_until>w.last_attempt_until GROUP BY w.action_row LIMIT ?")?;
        let rows=q.query_map([limit.min(128)],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,Option<u64>>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
    let mut examined=0;let mut repaired=0;
    for (row,old,action,until) in &rows {
        if examined>0&&started.elapsed()>=budget{break}
        // A session waiting for its boot has no attributed target, so no speculative
        // bindings exist to roll back. Retry only when a new boot observation covers it.
        if repair(conn,*row,*old,action)?==Some(true){repaired+=1;}
        conn.execute("INSERT OR IGNORE INTO event_association_retry_audit SELECT ?,?,?,instance_id,strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM object_actions WHERE action_row=?",params![row,until,old,row])?;
        conn.execute("UPDATE event_association_waiting SET last_attempt_until=? WHERE action_row=?",params![until,row])?;
        examined+=1;
    }
    let (cursor,bound,total,changed,unresolved):(i64,i64,i64,i64,i64)=conn.query_row("SELECT cursor,bound,examined,repaired,unresolved FROM association_repair_state WHERE rule=?",[RULE],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
    Ok(json!({"rule":RULE,"cursor":cursor,"bound":bound,"examined":total,"repaired":changed,"unresolved":unresolved,"batch_repaired":repaired,"retry_examined":examined,"pending":examined>0,"elapsed_ms":started.elapsed().as_millis()}))
}
fn audit(conn:&Connection,row:i64,old:Option<u64>,new:Option<u64>,outcome:&str,detail:Value)->Result<()> {
    audit_rule(conn,RULE,row,old,new,outcome,detail)
}
pub(crate) fn audit_rule(conn:&Connection,rule:&str,row:i64,old:Option<u64>,new:Option<u64>,outcome:&str,detail:Value)->Result<()> {
    conn.execute("INSERT OR IGNORE INTO association_repair_audit VALUES(?,?,?,?,?,?,strftime('%Y-%m-%dT%H:%M:%fZ','now'))",params![rule,row,old,new,outcome,detail.to_string()])?;Ok(())
}
fn repair(conn:&Connection,row:i64,old:Option<u64>,action_id:&str)->Result<Option<bool>> {
    repair_rule(conn,row,old,action_id,RULE)
}
pub(crate) fn repair_rule(conn:&Connection,row:i64,old:Option<u64>,action_id:&str,rule:&str)->Result<Option<bool>> {
    let environment=old.map(|id|conn.query_row("SELECT machine_id,installation_id FROM object_instances WHERE instance_id=?",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()).transpose()?.flatten();
    let environment=match environment {Some(v)=>Some(v),None=>conn.query_row("SELECT m.value,i.value FROM settings m JOIN settings i ON i.key='projection_installation' WHERE m.key='projection_machine'",[],|r|Ok((r.get(0)?,r.get(1)?))).optional()?};
    let Some((machine,installation))=environment else{audit_rule(conn,rule,row,old,old,"unresolved",json!({"reason":"historical_environment_missing"}))?;return Ok(None)};
    let addresses={let mut q=conn.prepare("SELECT e.evidence_id,e.item_id FROM evidence_refs r CROSS JOIN evidence e ON e.kind=r.evidence_kind AND e.evidence_id=r.evidence_id WHERE r.subject_kind='action' AND r.subject_id=? AND r.evidence_kind='event' AND r.pruned_at IS NULL AND e.pruned_at IS NULL ORDER BY e.seq LIMIT 129")?;
        let rows=q.query_map([row.to_string()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
    if addresses.len()>128{audit_rule(conn,rule,row,old,old,"unresolved",json!({"reason":"evidence_limit"}))?;return Ok(None)}
    let mut cache=crate::evidence::SnapshotCache::default();let mut parsed=Vec::new();let mut expected=None;
    for (address,item) in addresses {
        let Some(input)=crate::evidence::input_at_cached(conn,&address,&item,&mut cache)? else {continue};
        if input.source.starts_with("pcscope.eventlog.import"){continue}
        let scope=if input.source_scope.trim().is_empty(){format!("machine:{machine}")}else{pcscope_contracts::semantics::business_scope(&input.source_scope)};
        let context=AdapterContext{catalog_version:pcscope_contracts::model::catalog_version().into(),rule_version:rule.into(),machine_id:machine.clone(),installation_id:installation.clone(),boot_id:None,collected_scope:scope,complete:true};
        let Some(mut action)=pcscope_contracts::adapters::event::EventAdapter.adapt(&input,&context).actions.into_iter().find(|a|a.action_id==action_id) else {continue};
        let context=crate::event_association::qualify(conn,&input,&context,&mut action)?;
        if let Some((identity,_))=crate::projection::service_state_target(conn,&input,&context)? {action.target=ActionTarget::Assigned{identity};}
        if let ActionTarget::Assigned{identity}=&action.target {
            let Some(strategy)=TypeStrategy::for_type(&identity.type_id) else {continue};
            let (key,missing)=pcscope_contracts::identity::normalise(&strategy,identity);if !missing.is_empty(){continue}
            let key=(identity.type_id.clone(),identity.scope.clone(),key);
            if expected.as_ref().is_some_and(|v|v!=&key){audit_rule(conn,rule,row,old,old,"unresolved",json!({"reason":"conflicting_identities"}))?;return Ok(None)}
            expected=Some(key);
        }
        parsed.push((input,context,action));
    }
    if parsed.is_empty(){audit_rule(conn,rule,row,old,old,"unresolved",json!({"reason":"material_or_rule_missing"}))?;return Ok(None)}
    // Missing identity does not veto complete evidence. Keep the incomplete inputs too:
    // their independent replay claims and any old bindings still need reconciliation.
    let mut target=None;
    for (input,context,action) in &parsed {
        if let ActionTarget::Assigned{identity}=&action.target {
            let request=IdentityRequest{candidate:identity.clone(),strategy:TypeStrategy::for_type(&identity.type_id).unwrap(),
                binding:SourceRecordRef{source:input.source.clone(),item_id:input.item_id.clone(),native_record_id:input.evidence.id.clone()},
                machine_id:machine.clone(),installation_id:installation.clone(),evidence_time:EvidenceTime{source_time:input.source_time.clone(),observed_at:input.observed_at.clone()},
                hints:LifecycleHints{complete_observation:false,collected_scope:context.collected_scope.clone(),configured:true,history_retained_from:None,states_object_ended:false,boot_id:context.boot_id.clone()},rule_version:rule.into()};
            let Some(id)=crate::identity::attribute_history(conn,&request)? else{audit_rule(conn,rule,row,old,old,"unresolved",json!({"reason":"ambiguous_identity"}))?;return Ok(None)};
            anyhow::ensure!(target.is_none_or(|prior|prior==id),"historical evidence resolved to different objects");target=Some(id);crate::display::bind_scope(conn,id,input)?;
            crate::projection::record_process_history_facts(conn,id,input,context,action)?;
        }
    }
    let (_,context,action)=parsed.iter().find(|(_,_,a)|target.is_none()||matches!(a.target,ActionTarget::Assigned{..})).expect("parsed evidence is nonempty");
    let reason=if target.is_none(){match &action.target{ActionTarget::Unassigned{reason}=>Some(reason.clone()),_=>None}}else{None};
    let changed=old!=target;
    if changed {
        conn.execute("UPDATE object_actions SET instance_id=?,unassigned_reason=? WHERE action_row=?",params![target,reason,row])?;
        if let Some(old)=old {
            let scope:String=conn.query_row("SELECT scope FROM object_instances WHERE instance_id=?",[old],|r|r.get(0))?;
            for (input,_,_) in &parsed {
                conn.execute("UPDATE source_bindings SET valid_to=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE instance_id=? AND source=? AND item_id=? AND scope=? AND native_record_id=? AND valid_to IS NULL",params![old,input.source,input.item_id,scope,input.evidence.id])?;
            }
        }
    }
    for (input,_,action) in &parsed {
        let claim_target=if matches!(action.target,ActionTarget::Assigned{..}){target}else{None};
        conn.execute("INSERT OR IGNORE INTO evidence_refs(subject_kind,subject_id,evidence_kind,evidence_id) VALUES('action',?,'event',?)",params![crate::projection::action_key(claim_target,action),input.evidence.id])?;
    }
    crate::event_association::save(conn,row,target,context,action)?;
    audit_rule(conn,rule,row,old,target,if changed{"repaired"}else if target.is_none(){"independent"}else{"verified"},json!({"identity":expected,"reason":reason,"boot_id":context.boot_id}))?;
    Ok(Some(changed))
}
