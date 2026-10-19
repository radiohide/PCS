//! Versioned, disposable read models. Only the existing background worker writes these tables.
//! Invalidation is transactional with model changes; reads never start a rebuild or write work.
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(crate) fn revision() -> &'static str {
    static REVISION: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| crate::digest(&format!(
        "display-v1:{}:{}:{}:{}:{}:{}", crate::model::catalog_fingerprint(),
        include_str!("query.rs"), include_str!("history_labels.rs"),
        include_str!("registration_dictionary.rs"), include_str!("timeline_policy.rs"),include_str!("display.rs"))));
    &REVISION
}

pub(crate) fn available(conn:&Connection)->Result<bool> {
    Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='display_build')",[],|r|r.get(0))?)
}

pub(crate) fn status(conn:&Connection)->Result<Value> {
    let (objects,object_bound,actions,action_bound,evidence,evidence_bound):(i64,i64,i64,i64,i64,i64)=conn.query_row(
        "SELECT object_cursor,object_bound,action_cursor,action_bound,evidence_cursor,evidence_bound FROM display_build WHERE id=1",
        [],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)))?;
    let (pending,failed):(i64,i64)=conn.query_row("SELECT count(*),coalesce(sum(attempts>0),0) FROM display_work",[],|r|Ok((r.get(0)?,r.get(1)?)))?;
    let counts_ready:bool=conn.query_row("SELECT initialised AND addresses_done FROM display_totals WHERE id=1",[],|r|r.get(0))?;
    let identity_pending:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM display_identity_work)",[],|r|r.get(0))?;
    Ok(json!({"updating":pending>0||identity_pending||objects<object_bound||actions<action_bound||evidence<evidence_bound||!counts_ready,
        "pending":pending,"failed":failed,"object_cursor":objects,"object_bound":object_bound,
        "action_cursor":actions,"action_bound":action_bound,"evidence_cursor":evidence,"evidence_bound":evidence_bound,
        "counts_ready":counts_ready}))
}

fn dirty(kind: &str, id: &str) -> String {
    format!("INSERT INTO display_work(kind,id) VALUES('{kind}',{id}) ON CONFLICT(kind,id) DO UPDATE SET attempts=0,retry_at=NULL,last_error=NULL;")
}

pub(crate) fn ensure(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS display_objects(
        instance_id INTEGER PRIMARY KEY REFERENCES object_instances(instance_id) ON DELETE CASCADE,
        revision TEXT NOT NULL,sort_name TEXT NOT NULL,summary TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS display_actions(
        action_row INTEGER PRIMARY KEY REFERENCES object_actions(action_row) ON DELETE CASCADE,
        revision TEXT NOT NULL,visible_main INTEGER NOT NULL,visible_all INTEGER NOT NULL,view TEXT NOT NULL,identity_guard TEXT NOT NULL,
        anchor TEXT NOT NULL,prepared_generation INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS display_work(kind TEXT NOT NULL,id INTEGER NOT NULL,
        attempts INTEGER NOT NULL DEFAULT 0,retry_at TEXT,last_error TEXT,PRIMARY KEY(kind,id));
      CREATE TABLE IF NOT EXISTS display_identity_work(instance_id INTEGER PRIMARY KEY,
        after_time TEXT NOT NULL DEFAULT '',after_row INTEGER NOT NULL DEFAULT 0);
      CREATE TABLE IF NOT EXISTS object_collection_scopes(
        instance_id INTEGER NOT NULL REFERENCES object_instances(instance_id) ON DELETE CASCADE,
        source TEXT NOT NULL,scope TEXT NOT NULL,PRIMARY KEY(instance_id,source,scope));
      CREATE TABLE IF NOT EXISTS display_build(
        id INTEGER PRIMARY KEY CHECK(id=1),revision TEXT NOT NULL,
        object_cursor INTEGER NOT NULL DEFAULT 0,action_cursor INTEGER NOT NULL DEFAULT 0,
        evidence_cursor INTEGER NOT NULL DEFAULT 0,object_bound INTEGER NOT NULL,
        action_bound INTEGER NOT NULL,evidence_bound INTEGER NOT NULL,
        generation INTEGER NOT NULL DEFAULT 0,published INTEGER NOT NULL DEFAULT 0);")?;
    // Also accepts disposable databases built by early read-model prototypes. No raw table is
    // changed, and an empty index is populated as each bounded view batch commits.
    for (table,columns) in [("display_actions",vec![("anchor","TEXT NOT NULL DEFAULT ''"),("prepared_generation","INTEGER NOT NULL DEFAULT 0")]),
        ("display_build",vec![("generation","INTEGER NOT NULL DEFAULT 0"),("published","INTEGER NOT NULL DEFAULT 0")])] {
        let existing={let mut q=conn.prepare(&format!("PRAGMA table_info({table})"))?;let names=q.query_map([],|r|r.get::<_,String>(1))?.collect::<rusqlite::Result<Vec<_>>>()?;names};
        for (column,definition) in columns {if !existing.iter().any(|name|name==column){conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {definition};"))?;}}
    }
    conn.execute_batch("CREATE INDEX IF NOT EXISTS display_actions_anchor ON display_actions(anchor DESC,action_row DESC);")?;
    conn.execute_batch("CREATE INDEX IF NOT EXISTS display_objects_name_asc ON display_objects(revision,sort_name,instance_id);
        CREATE INDEX IF NOT EXISTS display_objects_name_desc ON display_objects(revision,sort_name DESC,instance_id);
        CREATE TRIGGER IF NOT EXISTS display_object_cache_removed AFTER DELETE ON display_objects
        WHEN EXISTS(SELECT 1 FROM object_instances WHERE instance_id=OLD.instance_id) BEGIN
          INSERT OR IGNORE INTO display_work(kind,id) VALUES('object',OLD.instance_id); END;")?;
    // The fallback timeline is used during a rule rebuild. Its effective-time expression must
    // have an index too, otherwise SQLite evaluates visibility and sorts the entire history
    // before returning the first page.
    conn.execute_batch("CREATE INDEX IF NOT EXISTS object_actions_anchor ON object_actions(coalesce(interval_until,occurred_at,observed_at) DESC,action_row DESC);")?;
    // No historical table is scanned or copied at startup. A display rule update resets only
    // the semantic cursors; the independent, exact source-scope index keeps its progress.
    let changed=conn.execute("INSERT INTO display_build(id,revision,object_bound,action_bound,evidence_bound)
        SELECT 1,?,coalesce((SELECT max(instance_id) FROM object_instances),0),
        coalesce((SELECT max(action_row) FROM object_actions),0),coalesce((SELECT max(seq) FROM evidence),0)
        ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,object_cursor=0,action_cursor=0,
        published=0,object_bound=excluded.object_bound,action_bound=excluded.action_bound
        WHERE display_build.revision<>excluded.revision", [revision()])?;
    if changed!=0 {
        conn.execute_batch("INSERT OR IGNORE INTO display_work(kind,id) SELECT 'action',action_row FROM object_actions ORDER BY action_row DESC LIMIT 200;
            INSERT OR IGNORE INTO display_work(kind,id) SELECT 'object',instance_id FROM object_instances ORDER BY instance_id DESC LIMIT 100;")?;
    }
    // Poll timestamps do not invalidate a whole display. Seen/lifecycle are overlaid from the
    // authoritative row; complete field timestamps remain available in the detail query.
    let triggers = [
        ("object_insert", "AFTER INSERT ON object_instances", dirty("object", "NEW.instance_id")),
        ("object_update", "AFTER UPDATE OF type_id,category,scope,native_key,incarnation,display_name,identity_status ON object_instances WHEN OLD.type_id IS NOT NEW.type_id OR OLD.category IS NOT NEW.category OR OLD.scope IS NOT NEW.scope OR OLD.native_key IS NOT NEW.native_key OR OLD.incarnation IS NOT NEW.incarnation OR OLD.display_name IS NOT NEW.display_name OR OLD.identity_status IS NOT NEW.identity_status", dirty("object", "NEW.instance_id")),
        ("fact_insert", "AFTER INSERT ON object_facts", dirty("object", "NEW.instance_id")),
        ("fact_update", "AFTER UPDATE ON object_facts WHEN OLD.value IS NOT NEW.value OR OLD.value_status IS NOT NEW.value_status OR OLD.conflict_sources IS NOT NEW.conflict_sources", dirty("object", "NEW.instance_id")),
        ("fact_delete", "AFTER DELETE ON object_facts", dirty("object", "OLD.instance_id")),
        ("history_insert", "AFTER INSERT ON fact_history", dirty("object", "NEW.instance_id")),
        ("binding_insert", "AFTER INSERT ON source_bindings", dirty("object", "NEW.instance_id")),
        ("binding_update", "AFTER UPDATE ON source_bindings", dirty("object", "NEW.instance_id")),
        ("binding_delete", "AFTER DELETE ON source_bindings", dirty("object", "OLD.instance_id")),
        ("action_insert", "AFTER INSERT ON object_actions", dirty("action", "NEW.action_row")),
        ("action_update", "AFTER UPDATE ON object_actions", dirty("action", "NEW.action_row")),
    ];
    for (name, event, body) in triggers {
        conn.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS display_{name} {event} BEGIN {body} END;"))?;
    }
    for (name,event,id,kind) in [
        ("ref_insert","AFTER INSERT ON evidence_refs","NEW","action"),
        ("ref_update","AFTER UPDATE ON evidence_refs","NEW","action"),
        ("ref_delete","AFTER DELETE ON evidence_refs","OLD","action"),
    ] {
        let target=if kind=="action"{"action"}else{"object"};
        conn.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS display_{name} {event}
            WHEN {id}.subject_kind='{kind}' AND {id}.subject_id=CAST(CAST({id}.subject_id AS INTEGER) AS TEXT)
            BEGIN {} END;", dirty(target,&format!("CAST({id}.subject_id AS INTEGER)"))))?;
    }
    for (table,column,kind) in [("object_instances","instance_id","object"),("object_actions","action_row","action")] {
        conn.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS display_{kind}_delete AFTER DELETE ON {table} BEGIN
            DELETE FROM display_work WHERE kind='{kind}' AND id=OLD.{column};
            DELETE FROM display_{} WHERE {column}=OLD.{column}; END;",if kind=="object"{"objects"}else{"actions"}))?;
    }
    // A normal new event binding does not change any earlier event's exact evidence address.
    // Never fan out all historical actions on every log record (nor include max(binding_id)
    // in the guard). Only actual identity corrections / binding edits invalidate old proofs.
    conn.execute_batch("DROP TRIGGER IF EXISTS display_binding_identity_add;")?;
    conn.execute_batch("CREATE TRIGGER IF NOT EXISTS display_binding_proof_recovered AFTER INSERT ON source_bindings BEGIN
        INSERT OR IGNORE INTO display_work(kind,id)
        SELECT 'action',a.action_row FROM evidence_refs r INDEXED BY evidence_refs_evidence
        CROSS JOIN object_actions a ON a.action_row=CAST(r.subject_id AS INTEGER)
        CROSS JOIN display_actions d ON d.action_row=a.action_row
        WHERE r.evidence_kind='event' AND r.evidence_id=NEW.native_record_id AND r.subject_kind='action'
          AND r.subject_id=CAST(a.action_row AS TEXT) AND a.instance_id=NEW.instance_id
          AND json_extract(d.view,'$.context.attribution_status') IN ('mismatch','unverified')
          AND EXISTS(SELECT 1 FROM evidence e WHERE e.evidence_id=r.evidence_id AND e.source=NEW.source AND e.item_id=NEW.item_id);
      END;")?;
    for (name,event,id) in [
        ("identity_change","AFTER UPDATE OF type_id,category,machine_id,installation_id,scope,native_key,incarnation,identity_status ON object_instances WHEN OLD.type_id IS NOT NEW.type_id OR OLD.category IS NOT NEW.category OR OLD.machine_id IS NOT NEW.machine_id OR OLD.installation_id IS NOT NEW.installation_id OR OLD.scope IS NOT NEW.scope OR OLD.native_key IS NOT NEW.native_key OR OLD.incarnation IS NOT NEW.incarnation OR OLD.identity_status IS NOT NEW.identity_status","NEW.instance_id"),
        ("binding_identity_update","AFTER UPDATE OF source,item_id,scope,native_record_id ON source_bindings","NEW.instance_id"),
        ("name_protection","AFTER UPDATE ON object_facts WHEN NEW.field_id IN ('name','display_name','caption','subject') AND (OLD.value_status IS NOT NEW.value_status OR json_type(OLD.value) IS NOT json_type(NEW.value) OR (OLD.value='\"******\"') IS NOT (NEW.value='\"******\"'))","NEW.instance_id"),
        ("binding_identity_remove","AFTER DELETE ON source_bindings","OLD.instance_id"),
    ] {
        conn.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS display_{name} {event} BEGIN
            INSERT INTO display_identity_work(instance_id) SELECT {id} WHERE EXISTS(SELECT 1 FROM object_actions a JOIN display_actions d ON d.action_row=a.action_row WHERE a.instance_id={id})
            ON CONFLICT(instance_id) DO UPDATE SET after_time='',after_row=0; END;"))?;
    }
    Ok(())
}

pub(crate) fn identity_guard_sql()->&'static str {
    "coalesce((SELECT json_array(i.type_id,i.category,i.machine_id,i.installation_id,i.scope,i.native_key,i.incarnation,i.identity_status)
        FROM object_instances i WHERE i.instance_id=a.instance_id),'null')"
}

pub(crate) fn bind_scope(conn: &Connection, instance: u64, input: &pcscope_contracts::adapter::EvidenceInput) -> Result<()> {
    conn.execute("INSERT OR IGNORE INTO object_collection_scopes VALUES(?,?,?)",params![instance,input.source,input.source_scope])?;
    Ok(())
}

pub(crate) fn cached(conn: &Connection, kind: &str, id: i64) -> Result<Option<Value>> {
    if !available(conn)? {return Ok(None);}
    let (table,key,column)=if kind=="object" {("display_objects","instance_id","summary")}else{("display_actions","action_row","view")};
    let guard=if kind=="action" {format!(" AND d.identity_guard=(SELECT {} FROM object_actions a WHERE a.action_row=d.action_row)
        AND NOT EXISTS(SELECT 1 FROM display_identity_work w JOIN object_actions a ON a.instance_id=w.instance_id WHERE a.action_row=d.action_row)",identity_guard_sql())}else{String::new()};
    let text:Option<String>=conn.query_row(&format!("SELECT {column} FROM {table} d WHERE {key}=? AND revision=?{guard}
        AND NOT EXISTS(SELECT 1 FROM display_work w WHERE w.kind=? AND w.id=d.{key})"),params![id,revision(),kind],|r|r.get(0)).optional()?;
    text.map(|text|Ok(serde_json::from_str(&text)?)).transpose()
}

/// Historical associations use the exact cited material's source, item and native identity.
/// Business identity scope and the collector's schema/context/shard scope are never equated.
fn historical_scopes(conn:&Connection,limit:usize,budget:Duration)->Result<usize> {
    let started=Instant::now();
    let (cursor,bound):(i64,i64)=conn.query_row("SELECT evidence_cursor,evidence_bound FROM display_build WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?)))?;
    if cursor>=bound{return Ok(0);}
    let rows={let mut q=conn.prepare("SELECT seq,evidence_id,source,item_id,native_record_id,source_scope FROM evidence WHERE seq>? AND seq<=? ORDER BY seq LIMIT ?")?;
        let rows=q.query_map(params![cursor,bound,limit as i64],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
    let mut insert=conn.prepare_cached("INSERT OR IGNORE INTO object_collection_scopes(instance_id,source,scope)
            SELECT DISTINCT b.instance_id,?,? FROM source_bindings b
            WHERE b.source=? AND b.item_id=? AND b.native_record_id IN (?,?) AND (b.scope IN (?,?) OR ?='')
            AND NOT EXISTS(SELECT 1 FROM object_collection_scopes c WHERE c.instance_id=b.instance_id AND c.source=? AND c.scope=?)
            AND (EXISTS(SELECT 1 FROM evidence_refs r WHERE r.subject_kind='observation'
                AND r.subject_id=CAST(b.instance_id AS TEXT) AND r.evidence_id=?)
              OR EXISTS(SELECT 1 FROM object_actions a JOIN evidence_refs r ON r.subject_kind='action'
                AND r.subject_id=CAST(a.action_row AS TEXT) WHERE a.instance_id=b.instance_id AND r.evidence_id=?))")?;
    let mut last=cursor;let mut examined=0;
    for (seq,evidence,source,item,native,scope) in &rows {
        if examined>0&&started.elapsed()>=budget{break;}
        let business=pcscope_contracts::semantics::business_scope(scope);
        let machine=pcscope_contracts::adapters::installation_scope(&business);
        insert.execute(
            params![source,scope,source,item,native,evidence,business,machine,scope,source,scope,evidence,evidence])?;
        last=*seq;examined+=1;
    }
    if rows.is_empty(){last=bound;}
    conn.execute("UPDATE display_build SET evidence_cursor=? WHERE id=1",[last])?;
    Ok(examined)
}

fn build_one(conn:&Connection,kind:&str,id:i64,labels:&mut crate::query::ProcessLabelCache)->Result<()> {
    if kind=="object" {
        if let Some(mut summary)=crate::query::build_summary(conn,id,labels)? {
            summary["coverage"]=json!([]); // live, exact-scope status is cheap and is not duplicated.
            conn.execute("INSERT INTO display_objects VALUES(?,?,?,?) ON CONFLICT(instance_id) DO UPDATE SET
                revision=excluded.revision,sort_name=excluded.sort_name,summary=excluded.summary",
                params![id,revision(),summary["display_name"].as_str().unwrap_or_default(),summary.to_string()])?;
        }
    } else if let Some((view,main,all))=crate::query::build_action(conn,id)? {
        let guard:String=conn.query_row(&format!("SELECT {} FROM object_actions a WHERE action_row=?",identity_guard_sql()),[id],|r|r.get(0))?;
        conn.execute("UPDATE display_build SET generation=generation+1 WHERE id=1",[])?;
        let generation:i64=conn.query_row("SELECT generation FROM display_build WHERE id=1",[],|r|r.get(0))?;
        let anchor=view["interval_until"].as_str().or(view["occurred_at"].as_str()).or(view["observed_at"].as_str()).unwrap_or_default();
        conn.execute("INSERT INTO display_actions VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(action_row) DO UPDATE SET
            revision=excluded.revision,visible_main=excluded.visible_main,visible_all=excluded.visible_all,view=excluded.view,identity_guard=excluded.identity_guard,
            anchor=excluded.anchor,prepared_generation=excluded.prepared_generation",
            params![id,revision(),main,all,view.to_string(),guard,anchor,generation])?;
    }
    conn.execute("DELETE FROM display_work WHERE kind=? AND id=?",params![kind,id])?;
    Ok(())
}

/// One maintenance slice; independent of raw projection backlog, bounded by rows AND wall time.
/// New/changed rows get priority; historical cursors still advance each round (no starvation).
pub(crate) fn advance(conn:&Connection,limit:usize,budget:Duration)->Result<Value> {
    let started=Instant::now();let mut built=0usize;let mut failed=0usize;
    let mut labels=crate::query::ProcessLabelCache::default();
    let limit=limit.clamp(1,128);
    // Rare identity/binding corrections invalidate a signature immediately. Expand their history
    // in indexed, bounded slices rather than enqueueing an object's entire history in one write.
    let job:Option<(i64,String,i64)>=conn.query_row("SELECT instance_id,after_time,after_row FROM display_identity_work ORDER BY instance_id LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    if let Some((instance,at,row))=job {
        let rows={let mut q=conn.prepare("SELECT observed_at,action_row FROM object_actions WHERE instance_id=? AND (observed_at,action_row)>(?,?) ORDER BY observed_at,action_row LIMIT ?")?;
            let rows=q.query_map(params![instance,at,row,limit as i64],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
        for (_,id) in &rows {conn.execute("INSERT OR IGNORE INTO display_work(kind,id) VALUES('action',?)",[id])?;}
        if let Some((at,row))=rows.last().filter(|_|rows.len()==limit) {
            conn.execute("UPDATE display_identity_work SET after_time=?,after_row=? WHERE instance_id=?",params![at,row,instance])?;
        }else{conn.execute("DELETE FROM display_identity_work WHERE instance_id=?",[instance])?;}
    }
    let pending={let mut q=conn.prepare("SELECT kind,id FROM display_work WHERE retry_at IS NULL OR retry_at<=strftime('%Y-%m-%dT%H:%M:%fZ','now') ORDER BY id DESC,kind LIMIT ?")?;
        let rows=q.query_map([limit as i64],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
    for (kind,id) in pending {
        // Reserve half the slice for rebuilding history. A steady stream of current changes
        // must not reduce a 150k-action rebuild to one old action per wake-up.
        if started.elapsed()>=budget/2 {break;}
        match build_one(conn,&kind,id,&mut labels) {
            Ok(())=>built+=1,
            Err(error)=>{
                conn.execute("UPDATE display_work SET attempts=attempts+1,last_error=?,retry_at=strftime('%Y-%m-%dT%H:%M:%fZ','now','+60 seconds') WHERE kind=? AND id=?",params![error.to_string(),kind,id])?;
                failed+=1;
            }
        }
    }
    // Rotate the historical tasks durably. Each gets a real time slice, including scope backfill;
    // only the rows actually completed advance a cursor. Restarting resumes the same work.
    let turn:u64=conn.query_row("SELECT coalesce((SELECT value FROM settings WHERE key='display.history_turn.v1'),'0')",[],|r|r.get::<_,String>(0))?.parse().unwrap_or(0);
    conn.execute("INSERT INTO settings(key,value) VALUES('display.history_turn.v1',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[(turn.wrapping_add(1)%4).to_string()])?;
    let historical=match turn%4 {0=>Some(("object","object_instances","instance_id")),1|2=>Some(("action","object_actions","action_row")),_=>None};
    if let Some((kind,table,key))=historical {
        let (cursor,bound):(i64,i64)=conn.query_row(&format!("SELECT {kind}_cursor,{kind}_bound FROM display_build WHERE id=1"),[],|r|Ok((r.get(0)?,r.get(1)?)))?;
        if cursor<bound {
        let ids={let mut q=conn.prepare(&format!("SELECT {key} FROM {table} WHERE {key}>? AND {key}<=? ORDER BY {key} LIMIT ?"))?;
            let rows=q.query_map(params![cursor,bound,limit as i64],|r|r.get::<_,i64>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;rows};
        let mut last=cursor;
        for id in &ids {
            if last!=cursor && started.elapsed()>=budget {break;}
            conn.execute("INSERT OR IGNORE INTO display_work(kind,id) VALUES(?,?)",params![kind,id])?;
            // A failed item gets a retry deadline; it cannot reset/hold up the history cursor.
            let eligible:bool=conn.query_row("SELECT retry_at IS NULL OR retry_at<=strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM display_work WHERE kind=? AND id=?",params![kind,id],|r|r.get(0))?;
            if eligible {
                match build_one(conn,kind,*id,&mut labels) {Ok(())=>built+=1,Err(error)=>{
                    conn.execute("UPDATE display_work SET attempts=attempts+1,last_error=?,retry_at=strftime('%Y-%m-%dT%H:%M:%fZ','now','+60 seconds') WHERE kind=? AND id=?",params![error.to_string(),kind,id])?;failed+=1;
                }}
            }
            last=*id;
        }
        if ids.is_empty(){last=bound;}
        conn.execute(&format!("UPDATE display_build SET {kind}_cursor=? WHERE id=1"),[last])?;
        }
    }
    let linked=if historical.is_none(){historical_scopes(conn,2048,budget.saturating_sub(started.elapsed()))?}else{0};
    crate::display_counts::advance(conn,limit*2)?;
    let state=status(conn)?;let pending=state["updating"]==true;
    let history_pending=["object","action","evidence"].iter().any(|kind|state[format!("{kind}_cursor")].as_i64()<state[format!("{kind}_bound")].as_i64())||state["counts_ready"]!=true;
    if !pending {conn.execute("UPDATE display_build SET published=1 WHERE id=1 AND published=0",[])?;}
    Ok(json!({"built":built,"failed":failed,"failed_total":state["failed"],"linked":linked,"pending":pending,"history_pending":history_pending,"elapsed_ms":started.elapsed().as_millis()}))
}

pub(crate) fn published(conn:&Connection)->Result<bool> {
    if !available(conn)?{return Ok(false);}
    Ok(conn.query_row("SELECT published FROM display_build WHERE id=1",[],|r|r.get(0))?)
}

pub(crate) fn generation(conn:&Connection)->Result<i64> {
    Ok(conn.query_row("SELECT generation FROM display_build WHERE id=1",[],|r|r.get(0))?)
}

#[cfg(test)]
#[path="display_tests.rs"]
mod tests;
