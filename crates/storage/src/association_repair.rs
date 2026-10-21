//! Bounded, restartable repairs of derived links. Original records and action IDs stay intact.
use anyhow::Result;
use pcscope_contracts::{adapter::{AdapterContext,ActionTarget,SourceAdapter},identity::{IdentityRequest,TypeStrategy,SourceRecordRef,EvidenceTime,LifecycleHints}};
use rusqlite::{params,Connection,OptionalExtension};
use serde_json::{json,Value};
use std::time::{Duration,Instant};

const RULE:&str="task-path-v1";

pub(crate) fn ensure(conn:&Connection)->Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS association_repair_state(
        rule TEXT PRIMARY KEY,cursor INTEGER NOT NULL,bound INTEGER NOT NULL,
        examined INTEGER NOT NULL DEFAULT 0,repaired INTEGER NOT NULL DEFAULT 0,unresolved INTEGER NOT NULL DEFAULT 0);
      CREATE TABLE IF NOT EXISTS association_repair_audit(
        rule TEXT NOT NULL,action_row INTEGER NOT NULL,old_instance INTEGER,new_instance INTEGER,
        outcome TEXT NOT NULL,detail TEXT NOT NULL,at TEXT NOT NULL,
        PRIMARY KEY(rule,action_row));")?;
    Ok(())
}

pub(crate) fn advance(conn:&Connection,limit:usize,budget:Duration)->Result<Value> {
    anyhow::ensure!((1..=1024).contains(&limit),"association repair limit must be 1..1024");
    let started=Instant::now();
    conn.execute("INSERT OR IGNORE INTO association_repair_state(rule,cursor,bound) SELECT ?,0,coalesce(max(action_row),0) FROM object_actions",[RULE])?;
    let (cursor,bound):(i64,i64)=conn.query_row("SELECT cursor,bound FROM association_repair_state WHERE rule=?",[RULE],|r|Ok((r.get(0)?,r.get(1)?)))?;
    // Scan a bounded primary-key window, not a selective query that can walk all history.
    let rows={let mut q=conn.prepare("SELECT action_row,instance_id,action_id,target_type FROM object_actions WHERE action_row>? AND action_row<=? ORDER BY action_row LIMIT ?")?;
        let rows=q.query_map(params![cursor,bound,limit],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,Option<u64>>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
    let mut last=cursor;let mut examined=0;let mut repaired=0;let mut unresolved=0;
    for (row,old,action,ty) in &rows {
        if last!=cursor && started.elapsed()>=budget {break}
        if ty.as_deref()==Some("startup.scheduled_task") && action.starts_with("startup.task_") {
            examined+=1;
            match repair_task(conn,*row,*old,action)? {Some(true)=>repaired+=1,None=>unresolved+=1,_=>{}}
        }
        last=*row;
    }
    if rows.is_empty(){last=bound;}
    conn.execute("UPDATE association_repair_state SET cursor=?,examined=examined+?,repaired=repaired+?,unresolved=unresolved+? WHERE rule=?",params![last,examined,repaired,unresolved,RULE])?;
    let totals=conn.query_row("SELECT examined,repaired,unresolved FROM association_repair_state WHERE rule=?",[RULE],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?)))?;
    Ok(json!({"rule":RULE,"cursor":last,"bound":bound,"pending":last<bound,"examined":totals.0,"repaired":totals.1,"unresolved":totals.2,"batch_repaired":repaired,"elapsed_ms":started.elapsed().as_millis()}))
}

fn audit(conn:&Connection,row:i64,old:Option<u64>,new:Option<u64>,outcome:&str,detail:Value)->Result<()> {
    conn.execute("INSERT OR IGNORE INTO association_repair_audit VALUES(?,?,?,?,?,?,strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        params![RULE,row,old,new,outcome,detail.to_string()])?;Ok(())
}

fn repair_task(conn:&Connection,row:i64,old:Option<u64>,action_id:&str)->Result<Option<bool>> {
    if conn.query_row("SELECT EXISTS(SELECT 1 FROM association_repair_audit WHERE rule=? AND action_row=?)",params![RULE,row],|r|r.get::<_,bool>(0))? {return Ok(Some(false))}
    // Never borrow the current installation for an unscoped historical/imported action.
    let environment:Option<(String,String,String)>=old.map(|id|conn.query_row("SELECT machine_id,installation_id,scope FROM object_instances WHERE instance_id=?",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()).transpose()?.flatten();
    let Some((machine,installation,scope))=environment else {
        audit(conn,row,old,None,"unresolved",json!({"reason":"historical_environment_missing"}))?;return Ok(None)
    };
    let context=AdapterContext{catalog_version:pcscope_contracts::model::catalog_version().into(),rule_version:RULE.into(),machine_id:machine.clone(),installation_id:installation.clone(),boot_id:None,collected_scope:scope,complete:true};
    let addresses={let mut q=conn.prepare("SELECT e.evidence_id,e.item_id FROM evidence_refs r CROSS JOIN evidence e ON e.kind=r.evidence_kind AND e.evidence_id=r.evidence_id WHERE r.subject_kind='action' AND r.subject_id=? AND r.evidence_kind='event' AND r.pruned_at IS NULL AND e.pruned_at IS NULL ORDER BY e.seq LIMIT 129")?;
        let rows=q.query_map([row.to_string()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
    if addresses.len()>128 {audit(conn,row,old,None,"unresolved",json!({"reason":"evidence_limit"}))?;return Ok(None)}
    let mut cache=crate::evidence::SnapshotCache::default();
    let mut candidates=Vec::new();let mut expected=None;
    for (address,item) in addresses {
        let Some(input)=crate::evidence::input_at_cached(conn,&address,&item,&mut cache)? else {continue};
        if input.source.starts_with("pcscope.eventlog.import") {continue}
        let parsed=pcscope_contracts::adapters::event::EventAdapter.adapt(&input,&context);
        for action in parsed.actions.into_iter().filter(|a|a.action_id==action_id) {
            let ActionTarget::Assigned{identity}=&action.target else {continue};
            let Some(strategy)=TypeStrategy::for_type(&identity.type_id) else {continue};
            let (key,missing)=pcscope_contracts::identity::normalise(&strategy,identity);
            if !missing.is_empty(){continue}
            if expected.as_ref().is_some_and(|prior|prior!=&key) {
                audit(conn,row,old,None,"unresolved",json!({"reason":"conflicting_task_paths"}))?;return Ok(None)
            }
            expected=Some(key);
            candidates.push((input.clone(),action,strategy));
        }
    }
    if candidates.is_empty(){audit(conn,row,old,None,"unresolved",json!({"reason":"task_path_or_material_missing"}))?;return Ok(None)}
    let mut target=None;
    for (input,action,strategy) in &candidates {
        let ActionTarget::Assigned{identity}=&action.target else {unreachable!()};
        let request=IdentityRequest{candidate:identity.clone(),strategy:strategy.clone(),
            binding:SourceRecordRef{source:input.source.clone(),item_id:input.item_id.clone(),native_record_id:input.evidence.id.clone()},
            machine_id:machine.clone(),installation_id:installation.clone(),
            evidence_time:EvidenceTime{source_time:input.source_time.clone(),observed_at:action.observed_at.clone()},
            hints:LifecycleHints{complete_observation:false,collected_scope:context.collected_scope.clone(),configured:true,history_retained_from:None,states_object_ended:false,boot_id:None},rule_version:RULE.into()};
        let Some(id)=crate::identity::attribute_history(conn,&request)? else {
            audit(conn,row,old,None,"unresolved",json!({"reason":"ambiguous_task_identity"}))?;return Ok(None)
        };
        anyhow::ensure!(target.is_none_or(|prior|prior==id),"task evidence resolved to different instances");target=Some(id);
        crate::display::bind_scope(conn,id,input)?;
        // Preserve replay claims for the corrected target as well as the old audit trail.
        let claim=crate::projection::action_key(Some(id),action);
        conn.execute("INSERT OR IGNORE INTO evidence_refs(subject_kind,subject_id,evidence_kind,evidence_id) VALUES('action',?,'event',?)",params![claim,input.evidence.id])?;
    }
    let changed=old!=target;
    if changed {
        let bindings={let mut q=conn.prepare("SELECT binding_id,instance_id,source,item_id,scope,native_record_id,valid_from,valid_to FROM source_bindings WHERE instance_id=? AND native_record_id IN (SELECT evidence_id FROM evidence_refs WHERE subject_kind='action' AND subject_id=? AND evidence_kind='event')")?;
            let rows=q.query_map(params![old,row.to_string()],|r|Ok(json!({"binding_id":r.get::<_,i64>(0)?,"instance_id":r.get::<_,i64>(1)?,"source":r.get::<_,String>(2)?,"item_id":r.get::<_,String>(3)?,"scope":r.get::<_,String>(4)?,"native_record_id":r.get::<_,String>(5)?,"valid_from":r.get::<_,String>(6)?,"valid_to":r.get::<_,Option<String>>(7)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
        audit(conn,row,old,target,"repaired",json!({"native_key":expected,"old_bindings":bindings}))?;
        conn.execute("UPDATE object_actions SET instance_id=?,unassigned_reason=NULL WHERE action_row=?",params![target,row])?;
        // Close only this event's old binding. Never rename the shared provisional object.
        conn.execute("UPDATE source_bindings SET valid_to=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE instance_id=? AND valid_to IS NULL AND native_record_id IN (SELECT evidence_id FROM evidence_refs WHERE subject_kind='action' AND subject_id=? AND evidence_kind='event')",params![old,row.to_string()])?;
    } else {audit(conn,row,old,target,"unchanged",json!({"native_key":expected}))?;}
    Ok(Some(changed))
}

#[cfg(test)]
#[path="association_repair_tests.rs"]
mod tests;
