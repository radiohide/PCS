//! Revisit only changed action families through the existing (action_id, action_row) index.
use anyhow::Result;
use rusqlite::{params,Connection};
use serde_json::{json,Value};
use std::time::{Duration,Instant};

pub(crate) fn advance(conn:&Connection,limit:usize,budget:Duration)->Result<Value> {
    let started=Instant::now();
    for action in ["execution.process_started","execution.process_stopped","identity.group_member_added","identity.group_member_removed"] {
        let rule=format!("process-membership-v1/{action}");
        conn.execute("INSERT OR IGNORE INTO association_repair_state(rule,cursor,bound) SELECT ?,0,coalesce(max(action_row),0) FROM object_actions WHERE action_id=?",params![rule,action])?;
        let (cursor,bound):(i64,i64)=conn.query_row("SELECT cursor,bound FROM association_repair_state WHERE rule=?",[&rule],|r|Ok((r.get(0)?,r.get(1)?)))?;
        if cursor>=bound{continue}
        let rows={let mut q=conn.prepare_cached("SELECT action_row,instance_id FROM object_actions WHERE action_id=? AND action_row>? AND action_row<=? ORDER BY action_row LIMIT ?")?;
            let rows=q.query_map(params![action,cursor,bound,limit],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,Option<u64>>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
        let mut last=cursor;let mut examined=0;let mut repaired=0;let mut unresolved=0;
        for (row,old) in &rows {
            if examined>0&&started.elapsed()>=budget{break}
            conn.execute_batch("SAVEPOINT process_history_candidate")?;
            let result=crate::event_association_repair::repair_rule(conn,*row,*old,action,&rule)?;
            if result.is_none(){
                let detail:String=conn.query_row("SELECT detail FROM association_repair_audit WHERE rule=? AND action_row=?",params![rule,row],|r|r.get(0))?;
                conn.execute_batch("ROLLBACK TO process_history_candidate")?;
                crate::event_association_repair::audit_rule(conn,&rule,*row,*old,*old,"unresolved",serde_json::from_str(&detail)?)?;
                if detail.contains("conflicting_identities") {
                    conn.execute("INSERT INTO event_associations SELECT a.action_row,a.instance_id,CASE WHEN a.instance_id IS NULL THEN 'null' ELSE json_array(i.type_id,i.category,i.machine_id,i.installation_id,i.scope,i.native_key,i.incarnation,i.identity_status) END,'mismatch','原始记录指向不同对象。','{}',? FROM object_actions a LEFT JOIN object_instances i ON i.instance_id=a.instance_id WHERE a.action_row=? ON CONFLICT(action_row) DO UPDATE SET instance_id=excluded.instance_id,identity_guard=excluded.identity_guard,decision=excluded.decision,reason=excluded.reason,context=excluded.context,rule=excluded.rule",params![rule,row])?;
                }
            }
            conn.execute_batch("RELEASE process_history_candidate")?;
            match result{Some(true)=>repaired+=1,None=>unresolved+=1,_=>{}}
            last=*row;examined+=1;
        }
        if rows.is_empty(){last=bound}
        conn.execute("UPDATE association_repair_state SET cursor=?,examined=examined+?,repaired=repaired+?,unresolved=unresolved+? WHERE rule=?",params![last,examined,repaired,unresolved,rule])?;
        return Ok(json!({"rule":rule,"cursor":last,"bound":bound,"pending":true,"examined":examined,"repaired":repaired,"unresolved":unresolved,"elapsed_ms":started.elapsed().as_millis()}))
    }
    Ok(json!({"rule":"process-membership-v1","pending":false}))
}
