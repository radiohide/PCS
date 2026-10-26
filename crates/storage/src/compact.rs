//! Compact snapshot evidence. A run owns its timestamps and completion cursor; membership
//! and source/scope strings are shared. Legacy evidence is retained verbatim.
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub(crate) fn ensure(c: &Connection) -> Result<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS storage_strings(id INTEGER PRIMARY KEY,value TEXT NOT NULL UNIQUE);
      CREATE TABLE IF NOT EXISTS observation_sets(id INTEGER PRIMARY KEY,digest TEXT NOT NULL UNIQUE,members TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS observation_members(set_id INTEGER NOT NULL,ordinal INTEGER NOT NULL,item_key INTEGER NOT NULL REFERENCES storage_strings(id),PRIMARY KEY(set_id,ordinal),UNIQUE(set_id,item_key)) WITHOUT ROWID;
      CREATE TABLE IF NOT EXISTS observation_runs(
        id INTEGER PRIMARY KEY,evidence_id TEXT NOT NULL UNIQUE,source_key INTEGER NOT NULL REFERENCES storage_strings(id),scope_key INTEGER NOT NULL REFERENCES storage_strings(id),run_id TEXT NOT NULL,
        set_id INTEGER NOT NULL REFERENCES observation_sets(id),first_seq INTEGER NOT NULL,item_count INTEGER NOT NULL,next_item INTEGER NOT NULL DEFAULT 0,
        observed_at TEXT NOT NULL,ingested_at TEXT NOT NULL,decoding TEXT NOT NULL,pruned_at TEXT,
        CHECK(next_item>=0 AND next_item<=item_count));
      CREATE INDEX IF NOT EXISTS observation_runs_sequence ON observation_runs(first_seq);
      CREATE INDEX IF NOT EXISTS observation_runs_source ON observation_runs(source_key,first_seq);
      CREATE INDEX IF NOT EXISTS observation_runs_pending ON observation_runs(first_seq) WHERE next_item<item_count;
      CREATE TABLE IF NOT EXISTS observation_errors(run INTEGER NOT NULL REFERENCES observation_runs(id),ordinal INTEGER NOT NULL,attempts INTEGER NOT NULL DEFAULT 1,error TEXT NOT NULL,PRIMARY KEY(run,ordinal)) WITHOUT ROWID;
      CREATE VIEW IF NOT EXISTS all_evidence AS
        SELECT * FROM evidence UNION ALL
        SELECT r.first_seq+m.ordinal,r.evidence_id,i.value,'snapshot',s.value,'v1',sc.value,i.value,NULL,r.observed_at,r.ingested_at,r.decoding,
          'snapshot:'||s.value||'|'||sc.value||'|'||r.run_id||'|'||i.value,r.pruned_at
        FROM observation_runs r JOIN observation_members m ON m.set_id=r.set_id
        JOIN storage_strings i ON i.id=m.item_key JOIN storage_strings s ON s.id=r.source_key JOIN storage_strings sc ON sc.id=r.scope_key;
      CREATE TABLE IF NOT EXISTS completed_work(evidence_seq INTEGER PRIMARY KEY,queue_id INTEGER NOT NULL UNIQUE,attempts INTEGER NOT NULL,finished_at TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS compact_counts(source TEXT NOT NULL,state TEXT NOT NULL,item_count INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(source,state));
      CREATE TRIGGER IF NOT EXISTS compact_run_add AFTER INSERT ON observation_runs BEGIN
        INSERT INTO compact_counts VALUES((SELECT value FROM storage_strings WHERE id=NEW.source_key),'pending',NEW.item_count) ON CONFLICT(source,state) DO UPDATE SET item_count=item_count+excluded.item_count;
      END;
      CREATE TRIGGER IF NOT EXISTS compact_run_progress AFTER UPDATE OF next_item ON observation_runs BEGIN
        UPDATE compact_counts SET item_count=item_count-(NEW.next_item-OLD.next_item) WHERE source=(SELECT value FROM storage_strings WHERE id=NEW.source_key) AND state='pending';
        INSERT INTO compact_counts VALUES((SELECT value FROM storage_strings WHERE id=NEW.source_key),'done',NEW.next_item-OLD.next_item) ON CONFLICT(source,state) DO UPDATE SET item_count=item_count+excluded.item_count;
      END;
      CREATE TRIGGER IF NOT EXISTS compact_error_add AFTER INSERT ON observation_errors BEGIN
        INSERT INTO compact_counts SELECT s.value,'failed',1 FROM observation_runs r JOIN storage_strings s ON s.id=r.source_key WHERE r.id=NEW.run ON CONFLICT(source,state) DO UPDATE SET item_count=item_count+1;
        INSERT INTO compact_counts SELECT s.value,'done',-1 FROM observation_runs r JOIN storage_strings s ON s.id=r.source_key WHERE r.id=NEW.run ON CONFLICT(source,state) DO UPDATE SET item_count=item_count-1;
      END;
      CREATE TRIGGER IF NOT EXISTS compact_error_remove AFTER DELETE ON observation_errors BEGIN
        UPDATE compact_counts SET item_count=item_count+CASE WHEN state='done' THEN 1 ELSE -1 END WHERE state IN ('done','failed') AND source=(SELECT s.value FROM observation_runs r JOIN storage_strings s ON s.id=r.source_key WHERE r.id=OLD.run);
      END;
      CREATE TRIGGER IF NOT EXISTS compact_completed_add AFTER INSERT ON completed_work BEGIN
        INSERT INTO compact_counts SELECT source,'done',1 FROM evidence WHERE seq=NEW.evidence_seq ON CONFLICT(source,state) DO UPDATE SET item_count=item_count+1;
      END;
      CREATE TRIGGER IF NOT EXISTS compact_completed_remove BEFORE DELETE ON completed_work BEGIN
        UPDATE compact_counts SET item_count=item_count-1 WHERE source=(SELECT source FROM evidence WHERE seq=OLD.evidence_seq) AND state='done';
      END;
      CREATE TRIGGER IF NOT EXISTS compact_evidence_remove BEFORE DELETE ON evidence BEGIN
        DELETE FROM completed_work WHERE evidence_seq=OLD.seq;
      END;
      CREATE TRIGGER IF NOT EXISTS all_evidence_prune INSTEAD OF UPDATE OF pruned_at ON all_evidence BEGIN
        UPDATE evidence SET pruned_at=NEW.pruned_at WHERE seq=OLD.seq;
        UPDATE observation_runs SET pruned_at=NEW.pruned_at WHERE evidence_id=OLD.evidence_id AND OLD.kind='snapshot';
      END;
      CREATE VIEW IF NOT EXISTS all_projection_queue AS
        SELECT * FROM projection_queue UNION ALL
        SELECT d.queue_id,e.evidence_id,e.item_id,'object_model','done',d.attempts,NULL,e.ingested_at,d.finished_at FROM completed_work d JOIN evidence e ON e.seq=d.evidence_seq UNION ALL
        SELECT -(r.first_seq+m.ordinal),r.evidence_id,i.value,'object_model',
          CASE WHEN e.run IS NOT NULL THEN 'failed' WHEN m.ordinal<r.next_item THEN 'done' ELSE 'pending' END,
          coalesce(e.attempts,CASE WHEN m.ordinal<r.next_item THEN 1 ELSE 0 END),e.error,r.ingested_at,r.ingested_at
        FROM observation_runs r JOIN observation_members m ON m.set_id=r.set_id JOIN storage_strings i ON i.id=m.item_key
        LEFT JOIN observation_errors e ON e.run=r.id AND e.ordinal=m.ordinal;
      CREATE TRIGGER IF NOT EXISTS all_queue_requeue INSTEAD OF UPDATE ON all_projection_queue BEGIN
        INSERT OR IGNORE INTO projection_queue(queue_id,evidence_id,item_id,projection,state,attempts,last_error,enqueued_at,updated_at)
          SELECT d.queue_id,OLD.evidence_id,OLD.item_id,OLD.projection,NEW.state,d.attempts,NEW.last_error,OLD.enqueued_at,NEW.updated_at FROM completed_work d WHERE d.queue_id=OLD.queue_id AND NEW.state<>'done';
        DELETE FROM completed_work WHERE queue_id=OLD.queue_id AND NEW.state<>'done';
        UPDATE projection_queue SET state=NEW.state,attempts=NEW.attempts,last_error=NEW.last_error,updated_at=NEW.updated_at WHERE queue_id=OLD.queue_id AND OLD.queue_id>0;
        UPDATE observation_runs SET next_item=CASE WHEN NEW.state='pending' THEN min(next_item,-OLD.queue_id-first_seq) ELSE max(next_item,-OLD.queue_id-first_seq+1) END WHERE OLD.queue_id<0 AND evidence_id=OLD.evidence_id;
        DELETE FROM observation_errors WHERE run=(SELECT id FROM observation_runs WHERE evidence_id=OLD.evidence_id) AND (ordinal=-OLD.queue_id-(SELECT first_seq FROM observation_runs WHERE evidence_id=OLD.evidence_id) OR (NEW.state='pending' AND ordinal>=(SELECT next_item FROM observation_runs WHERE evidence_id=OLD.evidence_id))) AND NEW.state<>'failed';
      END;
      CREATE TRIGGER IF NOT EXISTS all_queue_remove INSTEAD OF DELETE ON all_projection_queue WHEN OLD.queue_id>0 BEGIN
        DELETE FROM projection_queue WHERE queue_id=OLD.queue_id;
        DELETE FROM completed_work WHERE queue_id=OLD.queue_id;
      END;")?;
    Ok(())
}

fn intern(c:&Connection,value:&str)->Result<i64>{
    c.execute("INSERT INTO storage_strings(value) VALUES(?) ON CONFLICT(value) DO NOTHING",[value])?;
    Ok(c.query_row("SELECT id FROM storage_strings WHERE value=?",[value],|r|r.get(0))?)
}

pub(crate) fn store(c:&Connection,source:&str,scope:&str,run:&str,at:&str,ingested:&str,items:&[pcscope_contracts::Item],decoding:&str)->Result<usize>{
    let source_key=intern(c,source)?;let scope_key=intern(c,scope)?;
    let ids:Vec<&str>=items.iter().map(|i|i.id.as_str()).collect();
    let members=serde_json::to_string(&ids)?;let hash=crate::digest(&members);
    let found:Option<(i64,String)>=c.query_row("SELECT id,members FROM observation_sets WHERE digest=?",[&hash],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    let set=if let Some((id,text))=found{anyhow::ensure!(text==members,"observation membership digest conflict");id}else{
        c.execute("INSERT INTO observation_sets(digest,members) VALUES(?,?)",params![hash,members])?;let id=c.last_insert_rowid();
        for (ordinal,item) in items.iter().enumerate(){let key=intern(c,&item.id)?;c.execute("INSERT INTO observation_members VALUES(?,?,?)",params![id,ordinal as i64,key])?;}id
    };
    // Reserve evidence sequence numbers without materialising a row for every member. Legacy
    // event inserts continue using AUTOINCREMENT, so read watermarks share one total order.
    let last:i64=c.query_row("SELECT coalesce((SELECT seq FROM sqlite_sequence WHERE name='evidence'),0)",[],|r|r.get(0))?;
    let next=last.checked_add(items.len() as i64).ok_or_else(||anyhow::anyhow!("evidence sequence overflow"))?;
    c.execute("INSERT INTO sqlite_sequence(name,seq) SELECT 'evidence',? WHERE NOT EXISTS(SELECT 1 FROM sqlite_sequence WHERE name='evidence')",[next])?;
    c.execute("UPDATE sqlite_sequence SET seq=? WHERE name='evidence'",[next])?;
    let address=pcscope_contracts::adapter::EvidenceRef::snapshot(source,scope,run).id;
    c.execute("INSERT INTO observation_runs(evidence_id,source_key,scope_key,run_id,set_id,first_seq,item_count,observed_at,ingested_at,decoding) VALUES(?,?,?,?,?,?,?,?,?,?)",params![address,source_key,scope_key,run,set,last+1,items.len() as i64,at,ingested,decoding])?;
    Ok(items.len())
}

pub(crate) fn pending(c:&Connection,projection:&str,limit:usize)->Result<Vec<(i64,String,String)>>{
    let mut q=c.prepare_cached("SELECT e.seq,q.queue_id,q.evidence_id,q.item_id FROM projection_queue q JOIN evidence e ON e.evidence_id=q.evidence_id AND e.item_id=q.item_id WHERE q.projection=? AND q.state='pending' ORDER BY q.queue_id LIMIT ?")?;
    let mut rows=q.query_map(params![projection,limit as i64],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    if projection==crate::evidence::OBJECT_PROJECTION {
        let mut runs=c.prepare_cached("SELECT first_seq,evidence_id,set_id,next_item FROM observation_runs WHERE next_item<item_count ORDER BY first_seq LIMIT ?")?;
        let runs=runs.query_map([limit as i64],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut remaining=limit;
        let mut members=c.prepare_cached("SELECT ordinal,i.value FROM observation_members m JOIN storage_strings i ON i.id=m.item_key WHERE m.set_id=? AND m.ordinal>=? ORDER BY ordinal LIMIT ?")?;
        for (first,address,set,next) in runs {
            if remaining==0 {break}
            let items=members.query_map(params![set,next,remaining as i64],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            remaining-=items.len();
            for (ordinal,item) in items {rows.push((first+ordinal,-(first+ordinal),address.clone(),item));}
        }
    }
    rows.sort_unstable_by_key(|r|r.0);rows.truncate(limit);
    Ok(rows.into_iter().map(|(_,id,address,item)|(id,address,item)).collect())
}

pub(crate) fn finish(c:&Connection,queue:i64,state:&str,error:Option<&str>)->Result<()> {
    finish_many(c,&[(queue,state.to_owned(),error.map(str::to_owned))])
}

/// One cursor update per processed run, committed with all resulting facts and failures.
/// The pending reader returns a contiguous prefix of each run, so no unprocessed item is skipped.
pub(crate) fn finish_many(c:&Connection,entries:&[(i64,String,Option<String>)])->Result<()> {
    let mut lookup=c.prepare_cached("SELECT id,first_seq,item_count FROM observation_runs WHERE first_seq<=?1 AND first_seq+item_count>?1 ORDER BY first_seq DESC LIMIT 1")?;
    let mut add=c.prepare_cached("INSERT INTO observation_errors(run,ordinal,error) VALUES(?,?,?) ON CONFLICT(run,ordinal) DO UPDATE SET attempts=attempts+1,error=excluded.error")?;
    let mut remove=c.prepare_cached("DELETE FROM observation_errors WHERE run=? AND ordinal=?")?;
    let mut advance=c.prepare_cached("UPDATE observation_runs SET next_item=max(next_item,?) WHERE id=?")?;
    let mut current:Option<(i64,i64,i64)>=None;let mut progress=std::collections::BTreeMap::<i64,i64>::new();
    for (queue,state,error) in entries {
        let seq=-queue;
        if current.is_none_or(|(_,first,count)|seq<first||seq>=first+count) {
            current=Some(lookup.query_row([seq],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?);
        }
        let (id,first,_)=current.unwrap();let ordinal=seq-first;
        if state=="failed"{add.execute(params![id,ordinal,error.as_deref().unwrap_or("projection failed")])?;}
        else{remove.execute(params![id,ordinal])?;}
        progress.entry(id).and_modify(|n|*n=(*n).max(ordinal+1)).or_insert(ordinal+1);
    }
    for (id,next) in progress {advance.execute(params![next,id])?;}
    Ok(())
}

pub(crate) fn compact_done(c:&Connection,limit:usize)->Result<()> {
    let mut q=c.prepare_cached("SELECT e.seq,q.queue_id,q.attempts,q.updated_at FROM projection_queue q JOIN evidence e ON e.evidence_id=q.evidence_id AND e.item_id=q.item_id WHERE q.state='done' AND q.projection='object_model' ORDER BY q.queue_id LIMIT ?")?;
    let rows=q.query_map([limit as i64],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,String>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    for (seq,id,attempts,at) in rows {
        c.execute("INSERT OR IGNORE INTO completed_work VALUES(?,?,?,?)",params![seq,id,attempts,at])?;
        c.execute("DELETE FROM projection_queue WHERE queue_id=?",[id])?;
    }
    Ok(())
}

pub(crate) fn watermark(c:&Connection)->Result<i64>{Ok(c.query_row("SELECT coalesce((SELECT seq FROM sqlite_sequence WHERE name='evidence'),0)",[],|r|r.get(0))?)}
