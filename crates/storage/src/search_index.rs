use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use crate::{collection, digest};

// Persist only narrow classification metadata. Original evidence remains in records.
pub fn ensure(conn: &mut Connection) -> Result<()> {
    let fingerprint=digest(&serde_json::to_string(collection::definitions())?);
    let current:Option<String>=conn.query_row("SELECT value FROM settings WHERE key='search_catalog_v1'",[],|r|r.get(0)).optional()?;
    if current.as_deref()==Some(&fingerprint) {return Ok(());}
    let tx=conn.transaction()?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS collection_definitions(key TEXT PRIMARY KEY,name TEXT NOT NULL,category TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS record_search(seq INTEGER PRIMARY KEY REFERENCES records(seq) ON DELETE CASCADE,item TEXT NOT NULL,item_name TEXT NOT NULL,category TEXT NOT NULL,kind TEXT NOT NULL,kind_group TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS event_actions(item TEXT NOT NULL,event_id INTEGER NOT NULL,action TEXT NOT NULL,PRIMARY KEY(item,event_id));
        CREATE INDEX IF NOT EXISTS record_search_item ON record_search(item,seq);
        CREATE INDEX IF NOT EXISTS record_search_category ON record_search(category,seq);
        CREATE INDEX IF NOT EXISTS record_search_kind ON record_search(kind,seq);
        CREATE INDEX IF NOT EXISTS record_search_group ON record_search(kind_group,seq);
        CREATE INDEX IF NOT EXISTS record_search_facets ON record_search(item,category,item_name);
        DROP TRIGGER IF EXISTS records_search_insert;
        DROP TRIGGER IF EXISTS records_search_update;
        DELETE FROM record_search;
        DELETE FROM event_actions;
        DELETE FROM collection_definitions;")?;
    let has_action:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('record_search') WHERE name='action')",[],|r|r.get(0))?;
    if !has_action {tx.execute_batch("ALTER TABLE record_search ADD COLUMN action TEXT NOT NULL DEFAULT ''; CREATE INDEX record_search_action ON record_search(action,seq);")?;}
    for definition in collection::definitions() {
        tx.execute("INSERT INTO collection_definitions VALUES(?,?,?)",params![definition["id"].as_str(),definition["name"].as_str(),definition["category"].as_str()])?;
        if let Some(actions)=definition["event_rule"]["actions"].as_object(){for (id,action) in actions{tx.execute("INSERT INTO event_actions VALUES(?,?,?)",params![definition["id"].as_str(),id.parse::<u64>()?,action["id"].as_str()])?;}}
    }
    let select=|alias:&str|{
        let item=collection::event_item(&format!("{alias}.source"),&format!("{alias}.object_id"),&format!("{alias}.kind"),&format!("{alias}.data"));
        format!("SELECT {alias}.seq,COALESCE(c.key,{alias}.source),COALESCE(c.name,{alias}.source),COALESCE(c.category,'other'),{alias}.kind,
            CASE WHEN substr({alias}.kind,1,9)='coverage.' THEN 'coverage' WHEN substr({alias}.kind,1,7)='object.' OR {alias}.kind='baseline.observed' THEN 'snapshot' ELSE 'events' END,
            COALESCE((SELECT action FROM event_actions a WHERE a.item=c.key AND a.event_id=json_extract({alias}.data,'$.event_id')),{alias}.kind)
            FROM records {alias} LEFT JOIN collection_definitions c ON c.key=({item})")
    };
    tx.execute_batch(&format!("INSERT INTO record_search {};",select("r")))?;
    let insertion=format!("INSERT INTO record_search {} WHERE r.seq=NEW.seq;",select("r"));
    // This rebuild is fingerprint-driven and may run at any time, so it must not write the schema
    // version: the migration in Store::open owns that number alone, otherwise a rebuild here would
    // silently step the database back to an older version.
    tx.execute_batch(&format!("CREATE TRIGGER records_search_insert AFTER INSERT ON records BEGIN {insertion} END;
        CREATE TRIGGER records_search_update AFTER UPDATE OF source,object_id,kind,data ON records BEGIN DELETE FROM record_search WHERE seq=OLD.seq; {insertion} END;"))?;
    tx.execute("INSERT INTO settings VALUES('search_catalog_v1',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[fingerprint])?;
    tx.commit()?;
    Ok(())
}
