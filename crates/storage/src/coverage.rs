use anyhow::Result;
use rusqlite::{params,Connection,OptionalExtension};
use serde_json::json;

pub fn ensure(conn:&Connection)->Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS coverage_state(source TEXT NOT NULL,reason TEXT NOT NULL,first_seen TEXT NOT NULL,last_seen TEXT NOT NULL,observations INTEGER NOT NULL,active INTEGER NOT NULL,PRIMARY KEY(source,reason));")?;
    Ok(())
}

// Reasons are kept separately: one source may report several simultaneous failures.
pub fn gap(conn:&Connection,source:&str,reason:&str,time:&str)->Result<usize> {
    let active:Option<bool>=conn.query_row("SELECT active FROM coverage_state WHERE source=? AND reason=?",params![source,reason],|r|r.get(0)).optional()?;
    conn.execute("INSERT INTO coverage_state VALUES(?,?,?,?,1,1) ON CONFLICT(source,reason) DO UPDATE SET first_seen=CASE WHEN active=1 THEN first_seen ELSE excluded.first_seen END,last_seen=excluded.last_seen,observations=CASE WHEN active=1 THEN observations+1 ELSE 1 END,active=1",params![source,reason,time,time])?;
    if active==Some(true) {return Ok(0);}
    conn.execute("INSERT INTO records(source,kind,object_id,occurred_at,data) VALUES(?,'coverage.gap','',?,?)",params![source,time,json!({"reason":reason,"basis":"source availability transition"}).to_string()])?;
    Ok(1)
}

/// What one collection attempt did for a source and the scope it covered.
///
/// A failure recorded here is a **collection** failure: the material was never obtained. A parse
/// failure is a different thing — the material arrived and could not be turned into objects — and
/// it lives with the projection work item that hit it. Keeping the two apart is what lets the
/// health view say which of them a source is actually suffering from; a source that is being
/// collected perfectly but whose material no adapter understands looks nothing like one that is
/// denied access.
///
/// `history_state` never claims more than is known. Re-enabling a disabled item does not mean the
/// interval it was off is now covered, so that transition opens a gap the same way a failed
/// collection does — the record of a missing interval is a fact about the history, not a reason to
/// pretend the interval was observed.
pub fn record_attempt(conn:&Connection,source:&str,scope:&str,configured:bool,ok:bool,at:&str)->Result<()> {
    let previous:Option<String>=conn.query_row("SELECT history_state FROM coverage_scopes WHERE source=? AND scope=?",params![source,scope],|r|r.get(0)).optional()?;
    let last_result=if ok{"success"}else{"collection_failed"};
    // A recorded gap — a declared one, a failure, or the interval an item spent switched off — is a
    // fact about the retained history and outlives the recovery that follows it. A successful
    // collection means collection is working again; it does not mean the missing interval was seen.
    let active_gap:i64=conn.query_row("SELECT EXISTS(SELECT 1 FROM coverage_state WHERE source=? AND active=1)",[source],|r|r.get(0))?;
    let history_state=if active_gap==1{
        "gap"
    }else if ok{
        "caught_up"
    }else{
        previous.as_deref().unwrap_or("unknown")
    };
    conn.execute(
        "INSERT INTO coverage_scopes(source,scope,configured,last_result,last_success_at,last_attempt_at,history_state,detail) VALUES(?,?,?,?,?,?,?,NULL)
         ON CONFLICT(source,scope) DO UPDATE SET configured=excluded.configured,last_result=excluded.last_result,
             last_success_at=CASE WHEN excluded.last_result='success' THEN excluded.last_success_at ELSE coverage_scopes.last_success_at END,
             last_attempt_at=excluded.last_attempt_at,history_state=excluded.history_state",
        params![source,scope,i64::from(configured),last_result,if ok{Some(at)}else{None},at,history_state],
    )?;
    Ok(())
}

/// Records that the collection items covering these sources were switched off or on.
///
/// This is written when the switch moves, not when material arrives, because material does not
/// arrive while an item is off — the collection is skipped entirely. Recording it here is what lets
/// the health view tell "switched off" apart from "silent", which look identical in the data and
/// mean opposite things for the history.
pub fn set_configured(conn:&Connection,sources:&[String],configured:bool,at:&str)->Result<()> {
    for source in sources {
        conn.execute("UPDATE coverage_scopes SET configured=? WHERE source=?",params![i64::from(configured),source])?;
        if !configured {
            gap(conn,source,DISABLED_INTERVAL,at)?;
        }
    }
    Ok(())
}

/// Reason recorded for the interval an item spent switched off.
pub const DISABLED_INTERVAL:&str="采集项停用期间的区间未被覆盖";

/// Derived counts, maintained in the same transaction as the evidence/queue changes. An initial
/// backfill runs only once on the writer; read-only pages never scan the retained queue history.
/// Triggers on both tables preserve the original INNER JOIN semantics even when retention deletes
/// evidence before queue entries, or a replay inserts evidence for an existing orphan queue entry.
pub(crate) fn ensure_projection_counts(conn:&Connection)->Result<()> {
    let exists:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='coverage_projection_counts')",[],|r|r.get(0))?;
    if exists {return Ok(());}
    let tx=conn.unchecked_transaction()?;
    tx.execute_batch(
        "CREATE TABLE coverage_projection_counts(source TEXT NOT NULL,state TEXT NOT NULL,item_count INTEGER NOT NULL CHECK(item_count>=0),PRIMARY KEY(source,state));
         INSERT INTO coverage_projection_counts
           SELECT e.source,q.state,count(*) FROM evidence e JOIN projection_queue q
           ON e.evidence_id=q.evidence_id AND e.item_id=q.item_id GROUP BY e.source,q.state;
         CREATE TRIGGER coverage_queue_insert AFTER INSERT ON projection_queue BEGIN
           INSERT INTO coverage_projection_counts SELECT source,NEW.state,1 FROM evidence WHERE evidence_id=NEW.evidence_id AND item_id=NEW.item_id
           ON CONFLICT(source,state) DO UPDATE SET item_count=item_count+excluded.item_count;
         END;
         CREATE TRIGGER coverage_queue_delete AFTER DELETE ON projection_queue BEGIN
           UPDATE coverage_projection_counts SET item_count=item_count-1
           WHERE source=(SELECT source FROM evidence WHERE evidence_id=OLD.evidence_id AND item_id=OLD.item_id) AND state=OLD.state;
         END;
         CREATE TRIGGER coverage_queue_update AFTER UPDATE OF evidence_id,item_id,state ON projection_queue BEGIN
           UPDATE coverage_projection_counts SET item_count=item_count-1
           WHERE source=(SELECT source FROM evidence WHERE evidence_id=OLD.evidence_id AND item_id=OLD.item_id) AND state=OLD.state;
           INSERT INTO coverage_projection_counts SELECT source,NEW.state,1 FROM evidence WHERE evidence_id=NEW.evidence_id AND item_id=NEW.item_id
           ON CONFLICT(source,state) DO UPDATE SET item_count=item_count+excluded.item_count;
         END;
         CREATE TRIGGER coverage_evidence_insert AFTER INSERT ON evidence BEGIN
           INSERT INTO coverage_projection_counts SELECT NEW.source,state,count(*) FROM projection_queue WHERE evidence_id=NEW.evidence_id AND item_id=NEW.item_id GROUP BY state
           ON CONFLICT(source,state) DO UPDATE SET item_count=item_count+excluded.item_count;
         END;
         CREATE TRIGGER coverage_evidence_delete AFTER DELETE ON evidence BEGIN
           UPDATE coverage_projection_counts SET item_count=item_count-(SELECT count(*) FROM projection_queue q WHERE q.evidence_id=OLD.evidence_id AND q.item_id=OLD.item_id AND q.state=coverage_projection_counts.state)
           WHERE source=OLD.source;
         END;
         CREATE TRIGGER coverage_evidence_update AFTER UPDATE OF source,evidence_id,item_id ON evidence BEGIN
           UPDATE coverage_projection_counts SET item_count=item_count-(SELECT count(*) FROM projection_queue q WHERE q.evidence_id=OLD.evidence_id AND q.item_id=OLD.item_id AND q.state=coverage_projection_counts.state)
           WHERE source=OLD.source;
           INSERT INTO coverage_projection_counts SELECT NEW.source,state,count(*) FROM projection_queue WHERE evidence_id=NEW.evidence_id AND item_id=NEW.item_id GROUP BY state
           ON CONFLICT(source,state) DO UPDATE SET item_count=item_count+excluded.item_count;
         END;")?;
    tx.commit()?;
    Ok(())
}

pub fn recovered(conn:&Connection,source:&str,time:&str)->Result<usize> {
    let mut q=conn.prepare("SELECT reason,first_seen,last_seen,observations FROM coverage_state WHERE source=? AND active=1 ORDER BY reason")?;
    let previous=q.query_map([source],|r|Ok(json!({"reason":r.get::<_,String>(0)?,"first_seen":r.get::<_,String>(1)?,"last_seen":r.get::<_,String>(2)?,"observations":r.get::<_,i64>(3)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
    drop(q);
    if previous.is_empty(){return Ok(0);}
    // A declared availability gap is retired by a success. The interval an item spent switched off
    // is not: no amount of later success observes it.
    conn.execute("UPDATE coverage_state SET active=0,last_seen=? WHERE source=? AND active=1 AND reason<>?",params![time,source,DISABLED_INTERVAL])?;
    conn.execute("INSERT INTO records(source,kind,object_id,occurred_at,data) VALUES(?,'coverage.recovered','',?,?)",params![source,time,json!({"reason":"来源已恢复成功采集；不表示历史缺失数据已补齐。","previous_gaps":previous,"basis":"successful source collection"}).to_string()])?;
    Ok(1)
}

/// Every source's coverage, optionally narrowed to one source.
///
/// A free function rather than a method so the query layer reads the same rows the health page does,
/// instead of a second implementation of the same report.
pub fn report(conn:&Connection,source:Option<&str>)->Result<Vec<serde_json::Value>> {
    let cached:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='coverage_projection_counts')",[],|r|r.get(0))?;
    // A reader opened before the writer has performed the additive migration keeps the previous
    // exact query. It must not invent zero failures while the count table is unavailable.
    let compact:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='compact_counts')",[],|r|r.get(0))?;
    let aggregate=if cached && compact {
        "SELECT source,sum(CASE WHEN state='pending' THEN item_count ELSE 0 END) AS pending,
          sum(CASE WHEN state='failed' THEN item_count ELSE 0 END) AS failed,
          sum(CASE WHEN state='done' THEN item_count ELSE 0 END) AS done
          FROM (SELECT * FROM coverage_projection_counts UNION ALL SELECT * FROM compact_counts) GROUP BY source"
    }else if cached {
        "SELECT source,sum(CASE WHEN state='pending' THEN item_count ELSE 0 END) AS pending,
          sum(CASE WHEN state='failed' THEN item_count ELSE 0 END) AS failed,
          sum(CASE WHEN state='done' THEN item_count ELSE 0 END) AS done
          FROM coverage_projection_counts GROUP BY source"
    }else{
        "SELECT e.source,sum(CASE WHEN q.state='pending' THEN 1 ELSE 0 END) AS pending,
          sum(CASE WHEN q.state='failed' THEN 1 ELSE 0 END) AS failed,
          sum(CASE WHEN q.state='done' THEN 1 ELSE 0 END) AS done
          FROM projection_queue q JOIN evidence e ON e.evidence_id=q.evidence_id AND e.item_id=q.item_id GROUP BY e.source"
    };
    let mut sql=format!(
        "SELECT c.source,c.scope,c.configured,c.last_result,c.last_success_at,c.last_attempt_at,c.history_state,c.detail,
                coalesce(q.pending,0),coalesce(q.failed,0),coalesce(q.done,0)
         FROM coverage_scopes c
         LEFT JOIN ({aggregate}) q ON q.source=c.source");
    if source.is_some(){sql.push_str(" WHERE c.source=?");}
    sql.push_str(" ORDER BY c.source,c.scope");
    let mut statement=conn.prepare(&sql)?;
    let map=|row:&rusqlite::Row<'_>|Ok(json!({
        "source":row.get::<_,String>(0)?,"scope":row.get::<_,String>(1)?,"configured":row.get::<_,i64>(2)?!=0,
        "last_result":row.get::<_,String>(3)?,"last_success_at":row.get::<_,Option<String>>(4)?,
        "last_attempt_at":row.get::<_,Option<String>>(5)?,"history_state":row.get::<_,String>(6)?,
        "detail":row.get::<_,Option<String>>(7)?,
        "projection":{"pending":row.get::<_,i64>(8)?,"failed":row.get::<_,i64>(9)?,"done":row.get::<_,i64>(10)?},
    }));
    let rows=match source{Some(source)=>statement.query_map(params![source],map)?.collect::<rusqlite::Result<Vec<_>>>()?,
                          None=>statement.query_map([],map)?.collect::<rusqlite::Result<Vec<_>>>()?};
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture()->Connection {
        let conn=Connection::open_in_memory().unwrap();
        crate::evidence::ensure(&conn).unwrap();crate::evidence::ensure_query_indexes(&conn).unwrap();
        crate::model::create_schema(&conn).unwrap();ensure(&conn).unwrap();
        conn
    }
    fn evidence(conn:&Connection,id:&str,source:&str){
        conn.execute("INSERT INTO evidence(evidence_id,item_id,kind,source,structure_version,source_scope,native_record_id,observed_at,ingested_at,decoding,payload_ref) VALUES(?,'i','event',?,'v1','scope','native','now','now','{}','ref')",params![id,source]).unwrap();
    }
    fn queue(conn:&Connection,id:&str,projection:&str,state:&str){
        conn.execute("INSERT INTO projection_queue(evidence_id,item_id,projection,state,enqueued_at,updated_at) VALUES(?,'i',?,?,'now','now')",params![id,projection,state]).unwrap();
    }
    fn same_as_history(conn:&Connection){
        let sql="SELECT e.source,q.state,count(*) FROM evidence e JOIN projection_queue q ON e.evidence_id=q.evidence_id AND e.item_id=q.item_id GROUP BY e.source,q.state ORDER BY e.source,q.state";
        let rows=|sql:&str|conn.prepare(sql).unwrap().query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?))).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();
        assert_eq!(rows(sql),rows("SELECT source,state,item_count FROM coverage_projection_counts WHERE item_count>0 ORDER BY source,state"));
    }
    #[test]
    fn projection_counts_backfill_follow_transactions_and_both_retention_orders(){
        let conn=fixture();evidence(&conn,"a","one");queue(&conn,"a","objects","pending");queue(&conn,"a","other","failed");
        queue(&conn,"orphan","objects","done");
        record_attempt(&conn,"one","scope",true,true,"now").unwrap();
        let old=report(&conn,None).unwrap();ensure_projection_counts(&conn).unwrap();
        assert_eq!(report(&conn,None).unwrap(),old);same_as_history(&conn);
        ensure_projection_counts(&conn).unwrap();same_as_history(&conn);
        evidence(&conn,"orphan","two");same_as_history(&conn);
        conn.execute("UPDATE projection_queue SET state='done' WHERE evidence_id='a'",[]).unwrap();same_as_history(&conn);
        conn.execute("UPDATE evidence SET source='two' WHERE evidence_id='a'",[]).unwrap();same_as_history(&conn);
        conn.execute_batch("BEGIN; UPDATE projection_queue SET state='failed';").unwrap();same_as_history(&conn);
        conn.execute_batch("ROLLBACK;").unwrap();same_as_history(&conn);
        conn.execute("DELETE FROM evidence WHERE evidence_id='a'",[]).unwrap();same_as_history(&conn);
        conn.execute("DELETE FROM projection_queue WHERE evidence_id='a'",[]).unwrap();same_as_history(&conn);
        conn.execute("DELETE FROM projection_queue WHERE evidence_id='orphan'",[]).unwrap();same_as_history(&conn);
        conn.execute("DELETE FROM evidence WHERE evidence_id='orphan'",[]).unwrap();same_as_history(&conn);
        evidence(&conn,"new","one");queue(&conn,"new","objects","pending");same_as_history(&conn);
        conn.execute("UPDATE projection_queue SET evidence_id='not-yet'",[]).unwrap();same_as_history(&conn);
        conn.execute("UPDATE evidence SET evidence_id='not-yet'",[]).unwrap();same_as_history(&conn);
        assert_eq!(report(&conn,Some("one")).unwrap()[0]["projection"]["pending"],1);
        assert!(report(&conn,Some("absent")).unwrap().is_empty());
    }
    #[test]
    fn coverage_read_work_is_independent_of_queue_history_size(){
        let conn=fixture();ensure_projection_counts(&conn).unwrap();
        conn.execute_batch("BEGIN;").unwrap();
        for n in 0..10000 {let id=n.to_string();evidence(&conn,&id,"one");queue(&conn,&id,"objects","done");}
        conn.execute_batch("COMMIT;").unwrap();
        record_attempt(&conn,"one","scope-1",true,true,"now").unwrap();
        record_attempt(&conn,"one","scope-2",true,true,"now").unwrap();
        // A source's counts are shared across its scopes, as in the original query.
        use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
        let steps=Arc::new(AtomicUsize::new(0));let used=steps.clone();
        conn.progress_handler(100,Some(move||used.fetch_add(100,Ordering::Relaxed)>=5000));
        let rows=report(&conn,None).expect("coverage must not scan the retained queue");
        assert_eq!(rows.len(),2);assert!(rows.iter().all(|r|r["projection"]["done"]==10000));
        assert!(steps.load(Ordering::Relaxed)<5000);
    }
}
