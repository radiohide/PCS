//! Exact common (unfiltered) counts, maintained transactionally. One row per DISTINCT evidence
//! address, not a second copy of every object citation. Arbitrary filtered/page counts keep their
//! original exact query; they cannot be summed from per-object evidence counts.
use anyhow::Result;
use rusqlite::{params,Connection};

pub(crate) fn ensure(conn:&Connection)->Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS display_totals(id INTEGER PRIMARY KEY CHECK(id=1),
        initialised INTEGER NOT NULL DEFAULT 0,instances INTEGER NOT NULL DEFAULT 0,
        actions INTEGER NOT NULL DEFAULT 0,source_records INTEGER NOT NULL DEFAULT 0,
        evidence INTEGER NOT NULL DEFAULT 0,address_cursor TEXT NOT NULL DEFAULT '',addresses_done INTEGER NOT NULL DEFAULT 0);
      INSERT OR IGNORE INTO display_totals(id) VALUES(1);
      CREATE TABLE IF NOT EXISTS display_evidence_totals(evidence_id TEXT PRIMARY KEY,ref_count INTEGER NOT NULL);
      CREATE TRIGGER IF NOT EXISTS display_address_add AFTER INSERT ON display_evidence_totals WHEN NEW.ref_count>0 BEGIN
        UPDATE display_totals SET evidence=evidence+1 WHERE id=1; END;
      CREATE TRIGGER IF NOT EXISTS display_address_remove AFTER DELETE ON display_evidence_totals BEGIN
        UPDATE display_totals SET evidence=evidence-1 WHERE id=1; END;
      CREATE TRIGGER IF NOT EXISTS display_count_object_add AFTER INSERT ON object_instances BEGIN
        UPDATE display_totals SET instances=instances+1 WHERE id=1 AND initialised=1; END;
      CREATE TRIGGER IF NOT EXISTS display_count_object_remove BEFORE DELETE ON object_instances BEGIN
        UPDATE display_totals SET instances=instances-1 WHERE id=1 AND initialised=1;
        UPDATE display_evidence_totals SET ref_count=ref_count-(SELECT count(*) FROM evidence_refs r
          WHERE r.subject_kind='observation' AND r.subject_id=CAST(OLD.instance_id AS TEXT)
            AND r.evidence_id=display_evidence_totals.evidence_id)
          WHERE evidence_id IN (SELECT evidence_id FROM evidence_refs WHERE subject_kind='observation' AND subject_id=CAST(OLD.instance_id AS TEXT));
        DELETE FROM display_evidence_totals WHERE ref_count<=0 AND evidence_id IN
          (SELECT evidence_id FROM evidence_refs WHERE subject_kind='observation' AND subject_id=CAST(OLD.instance_id AS TEXT));
      END;
      CREATE TRIGGER IF NOT EXISTS display_count_action_add AFTER INSERT ON object_actions WHEN NEW.instance_id IS NOT NULL BEGIN
        UPDATE display_totals SET actions=actions+1 WHERE id=1 AND initialised=1; END;
      CREATE TRIGGER IF NOT EXISTS display_count_action_remove AFTER DELETE ON object_actions WHEN OLD.instance_id IS NOT NULL BEGIN
        UPDATE display_totals SET actions=actions-1 WHERE id=1 AND initialised=1; END;
      CREATE TRIGGER IF NOT EXISTS display_count_action_update AFTER UPDATE OF instance_id ON object_actions BEGIN
        UPDATE display_totals SET actions=actions+(NEW.instance_id IS NOT NULL)-(OLD.instance_id IS NOT NULL) WHERE id=1 AND initialised=1; END;
      CREATE TRIGGER IF NOT EXISTS display_count_binding_add AFTER INSERT ON source_bindings WHEN NEW.valid_to IS NULL BEGIN
        UPDATE display_totals SET source_records=source_records+1 WHERE id=1 AND initialised=1; END;
      CREATE TRIGGER IF NOT EXISTS display_count_binding_remove AFTER DELETE ON source_bindings WHEN OLD.valid_to IS NULL BEGIN
        UPDATE display_totals SET source_records=source_records-1 WHERE id=1 AND initialised=1; END;
      CREATE TRIGGER IF NOT EXISTS display_count_binding_update AFTER UPDATE OF valid_to ON source_bindings BEGIN
        UPDATE display_totals SET source_records=source_records+(NEW.valid_to IS NULL)-(OLD.valid_to IS NULL) WHERE id=1 AND initialised=1; END;
      CREATE TRIGGER IF NOT EXISTS display_count_reference_add AFTER INSERT ON evidence_refs
        WHEN NEW.subject_kind='observation' AND EXISTS(SELECT 1 FROM object_instances WHERE CAST(instance_id AS TEXT)=NEW.subject_id AND instance_id=CAST(NEW.subject_id AS INTEGER)) BEGIN
        UPDATE display_evidence_totals SET ref_count=ref_count+1 WHERE evidence_id=NEW.evidence_id;
        INSERT INTO display_evidence_totals(evidence_id,ref_count)
          SELECT NEW.evidence_id,(SELECT count(*) FROM evidence_refs r WHERE r.subject_kind='observation' AND r.evidence_id=NEW.evidence_id
            AND EXISTS(SELECT 1 FROM object_instances i WHERE i.instance_id=CAST(r.subject_id AS INTEGER) AND CAST(i.instance_id AS TEXT)=r.subject_id))
          WHERE NOT EXISTS(SELECT 1 FROM display_evidence_totals WHERE evidence_id=NEW.evidence_id);
      END;
      CREATE TRIGGER IF NOT EXISTS display_count_reference_remove AFTER DELETE ON evidence_refs
        WHEN OLD.subject_kind='observation' AND EXISTS(SELECT 1 FROM object_instances WHERE instance_id=CAST(OLD.subject_id AS INTEGER) AND CAST(instance_id AS TEXT)=OLD.subject_id) BEGIN
        UPDATE display_evidence_totals SET ref_count=ref_count-1 WHERE evidence_id=OLD.evidence_id;
        DELETE FROM display_evidence_totals WHERE evidence_id=OLD.evidence_id AND ref_count<=0;
      END;")?;
    Ok(())
}

pub(crate) fn advance(conn:&Connection,limit:usize)->Result<()> {
    let (initialised,done,cursor):(bool,bool,String)=conn.query_row("SELECT initialised,addresses_done,address_cursor FROM display_totals WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    if !initialised {
        conn.execute_batch("UPDATE display_totals SET initialised=1,
            instances=(SELECT count(*) FROM object_instances),
            actions=(SELECT count(*) FROM object_actions WHERE instance_id IS NOT NULL),
            source_records=(SELECT count(*) FROM source_bindings WHERE valid_to IS NULL) WHERE id=1;")?;
    }
    if done{return Ok(());}
    // Skip all repeated citations through the pre-existing evidence-address index. Each slice
    // touches at most `limit` addresses; there is no GROUP BY over retained history at startup.
    let mut last=cursor;
    for _ in 0..limit {
        let next:Option<String>=conn.query_row("SELECT min(evidence_id) FROM evidence_refs INDEXED BY evidence_refs_observation_address WHERE subject_kind='observation' AND evidence_id>?",[&last],|r|r.get(0))?;
        let Some(next)=next else{conn.execute("UPDATE display_totals SET addresses_done=1 WHERE id=1",[])?;break};
        conn.execute("INSERT OR IGNORE INTO display_evidence_totals(evidence_id,ref_count)
            SELECT ?,count(*) FROM evidence_refs r INDEXED BY evidence_refs_observation_address
            WHERE r.subject_kind='observation' AND r.evidence_id=?
            AND EXISTS(SELECT 1 FROM object_instances i WHERE i.instance_id=CAST(r.subject_id AS INTEGER) AND CAST(i.instance_id AS TEXT)=r.subject_id)
            HAVING count(*)>0",params![next,next])?;
        last=next;
    }
    conn.execute("UPDATE display_totals SET address_cursor=? WHERE id=1",[last])?;
    Ok(())
}

pub(crate) fn counts(conn:&Connection)->Result<Option<pcscope_contracts::query::Counts>> {
    use rusqlite::OptionalExtension;
    Ok(conn.query_row("SELECT instances,actions,source_records,evidence FROM display_totals WHERE id=1 AND initialised=1 AND addresses_done=1",[],|r|Ok(pcscope_contracts::query::Counts{
        instances:r.get::<_,i64>(0)?.max(0) as u64,actions:r.get::<_,i64>(1)?.max(0) as u64,
        source_records:r.get::<_,i64>(2)?.max(0) as u64,evidence:r.get::<_,i64>(3)?.max(0) as u64,
    })).optional()?)
}
