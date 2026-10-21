//! Durable attribution decisions. Presentation reads the decision; it does not reconstruct
//! the event's machine/boot context from whichever object happens to exist today.
use anyhow::Result;
use pcscope_contracts::adapter::{AdapterContext,ActionCandidate,ActionTarget,EvidenceInput};
use rusqlite::{params,Connection,OptionalExtension};
use serde_json::{json,Value};

pub(crate) fn ensure(conn:&Connection)->Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS installation_boots(machine_id TEXT NOT NULL,installation_id TEXT NOT NULL,boot_id TEXT NOT NULL,start_at TEXT NOT NULL,observed_until TEXT NOT NULL,evidence_id TEXT NOT NULL,clock_consistent INTEGER NOT NULL DEFAULT 1,PRIMARY KEY(machine_id,installation_id,boot_id));
      CREATE TABLE IF NOT EXISTS event_association_retry_audit(action_row INTEGER NOT NULL,boot_until TEXT NOT NULL,old_instance INTEGER,new_instance INTEGER,at TEXT NOT NULL,PRIMARY KEY(action_row,boot_until));
      CREATE TABLE IF NOT EXISTS event_association_waiting(action_row INTEGER PRIMARY KEY REFERENCES object_actions(action_row) ON DELETE CASCADE,machine_id TEXT NOT NULL,installation_id TEXT NOT NULL,source_time TEXT NOT NULL,last_attempt_until TEXT NOT NULL DEFAULT '');
      CREATE INDEX IF NOT EXISTS event_association_waiting_time ON event_association_waiting(machine_id,installation_id,source_time);
      CREATE TABLE IF NOT EXISTS event_associations(
        action_row INTEGER PRIMARY KEY REFERENCES object_actions(action_row) ON DELETE CASCADE,
        instance_id INTEGER,identity_guard TEXT NOT NULL,decision TEXT NOT NULL,
        reason TEXT,context TEXT NOT NULL,rule TEXT NOT NULL);
      CREATE TRIGGER IF NOT EXISTS event_association_removed AFTER DELETE ON event_associations BEGIN
        INSERT INTO display_work(kind,id) VALUES('action',OLD.action_row) ON CONFLICT DO NOTHING; END;
      CREATE TRIGGER IF NOT EXISTS event_association_added AFTER INSERT ON event_associations BEGIN
        INSERT INTO display_work(kind,id) VALUES('action',NEW.action_row) ON CONFLICT DO NOTHING; END;
      CREATE TRIGGER IF NOT EXISTS event_association_changed AFTER UPDATE ON event_associations BEGIN
        INSERT INTO display_work(kind,id) VALUES('action',NEW.action_row) ON CONFLICT DO NOTHING; END;
      CREATE TRIGGER IF NOT EXISTS event_association_binding_removed AFTER DELETE ON source_bindings BEGIN
        DELETE FROM event_associations WHERE instance_id=OLD.instance_id AND action_row IN
          (SELECT CAST(subject_id AS INTEGER) FROM evidence_refs WHERE evidence_kind='event' AND evidence_id=OLD.native_record_id AND subject_kind='action'); END;
      CREATE TRIGGER IF NOT EXISTS event_association_binding_changed AFTER UPDATE OF source,item_id,scope,native_record_id,instance_id ON source_bindings BEGIN
        DELETE FROM event_associations WHERE instance_id=OLD.instance_id AND action_row IN
          (SELECT CAST(subject_id AS INTEGER) FROM evidence_refs WHERE evidence_kind='event' AND evidence_id=OLD.native_record_id AND subject_kind='action'); END;")?;Ok(())
}

/// Persist only native boot identities with an observed time interval. Wall-clock boot
/// estimates can jitter; they delimit an interval but never identify a boot.
pub(crate) fn observe_boot(conn:&Connection,input:&EvidenceInput,context:&AdapterContext)->Result<()> {
    if input.source!="pcscope.system.boot-state" {return Ok(())}
    let Some(id)=boot_identifier(&input.payload) else {return Ok(())};
    let Some(start)=input.payload["last_boot_time"].as_str().and_then(|s|pcscope_contracts::semantics::canonical_time(s).ok()) else {return Ok(())};
    let Ok(until)=pcscope_contracts::semantics::canonical_time(&input.observed_at) else {return Ok(())};
    if start>until{return Ok(())}
    conn.execute("INSERT INTO installation_boots(machine_id,installation_id,boot_id,start_at,observed_until,evidence_id) VALUES(?,?,?,?,?,?) ON CONFLICT(machine_id,installation_id,boot_id) DO UPDATE SET clock_consistent=clock_consistent AND abs(julianday(start_at)-julianday(excluded.start_at))*86400<=5,start_at=min(start_at,excluded.start_at),observed_until=max(observed_until,excluded.observed_until),evidence_id=excluded.evidence_id",
        params![context.machine_id,context.installation_id,id,start,until,input.evidence.id])?;Ok(())
}
pub(crate) fn boot_identifier(payload:&Value)->Option<String> {
    if payload["boot_id_basis"]!="SystemBootEnvironmentInformation" {return None}
    let id=payload["boot_id"].as_str()?;
    if id.len()!=36 || !id.bytes().enumerate().all(|(n,c)|if [8,13,18,23].contains(&n){c==b'-'}else{c.is_ascii_hexdigit()}) || id=="00000000-0000-0000-0000-000000000000" {return None}
    Some(id.to_ascii_lowercase())
}
pub(crate) fn boot_at(conn:&Connection,machine:&str,installation:&str,at:&str)->Result<Option<String>> {
    let Ok(at)=pcscope_contracts::semantics::canonical_time(at) else {return Ok(None)};
    let mut q=conn.prepare_cached("SELECT boot_id,clock_consistent FROM installation_boots WHERE machine_id=? AND installation_id=? AND start_at<=? AND observed_until>=? LIMIT 2")?;
    let rows=q.query_map(params![machine,installation,at,at],|r|Ok((r.get::<_,String>(0)?,r.get::<_,bool>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(if rows.len()==1 && rows[0].1 {rows.into_iter().next().map(|r|r.0)}else{None})
}

pub(crate) fn qualify(conn:&Connection,input:&EvidenceInput,context:&AdapterContext,action:&mut ActionCandidate)->Result<AdapterContext> {
    let mut context=context.clone();
    let ty=pcscope_contracts::model::action_definition(&action.action_id).and_then(|v|v["target_type"].as_str()).unwrap_or("");
    if ty=="account.user" && matches!(action.target,ActionTarget::Unassigned{..}) && !input.source.starts_with("pcscope.eventlog.import") {
        if let Some(identity)=local_account_at(conn,input,&context)? {action.target=ActionTarget::Assigned{identity};}
    }
    if matches!(ty,"runtime.logon_session"|"runtime.terminal_session") && input.evidence.kind==pcscope_contracts::adapter::EvidenceKind::Event {
        context.boot_id=if input.source.starts_with("pcscope.eventlog.import")||input.payload["source_time_available"]==false {None}
            else if let Some(at)=&input.source_time {boot_at(conn,&context.machine_id,&context.installation_id,at)?}else{None};
        if context.boot_id.is_none() {
            action.target=ActionTarget::Unassigned{reason:"事件所属的启动周期尚未建立。".into()};
        } else {
            use pcscope_contracts::adapter::SourceAdapter;
            if let Some(parsed)=pcscope_contracts::adapters::event::EventAdapter.adapt(input,&context).actions.into_iter().find(|a|a.action_id==action.action_id){action.target=parsed.target;}
        }
    }
    if input.evidence.kind==pcscope_contracts::adapter::EvidenceKind::Event {
        if let ActionTarget::Assigned{identity}=&action.target {
            if let Some(strategy)=pcscope_contracts::identity::TypeStrategy::for_type(&identity.type_id) {
                let (_,missing)=pcscope_contracts::identity::normalise(&strategy,identity);
                if !missing.is_empty() {action.target=ActionTarget::Unassigned{reason:format!("事件未提供完整对象标识：{}。",missing.join("、"))};}
            }
        }
    }
    Ok(context)
}

/// A name is an alias, never an identity. This conservative local-account lookup requires
/// a unique SID, the explicitly named local host, and native observations bracketing the
/// event. Rename histories and domain accounts require a richer alias interval and stay
/// independent here. In particular SubjectUserName (the actor) is never consulted.
fn local_account_at(conn:&Connection,input:&EvidenceInput,context:&AdapterContext)->Result<Option<pcscope_contracts::adapter::IdentityCandidate>> {
    if input.payload["source_time_available"]==false{return Ok(None)}
    let Some(at)=input.source_time.as_deref() else{return Ok(None)};
    let fields=&input.payload["fields"];
    let (Some(name),Some(domain))=(fields["TargetUserName"].as_str(),fields["TargetDomainName"].as_str()) else{return Ok(None)};
    if name.is_empty()||domain.is_empty()||name=="-"||pcscope_contracts::semantics::is_mask_sentinel(&json!(name)){return Ok(None)}
    let host:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM object_instances i JOIN object_facts f ON f.instance_id=i.instance_id AND f.field_id='name' AND f.value_status='known' WHERE i.type_id='account.machine_identity' AND i.machine_id=? AND i.installation_id=? AND json_extract(f.value,'$')=? COLLATE NOCASE AND julianday(i.first_observed_at)<=julianday(?) AND julianday(f.observed_at)>=julianday(?) AND NOT EXISTS(SELECT 1 FROM fact_history h WHERE h.instance_id=i.instance_id AND h.field_id='name' AND h.before_status='known' AND h.after_status='known' AND h.before_value<>h.after_value))",params![context.machine_id,context.installation_id,domain,at,at],|r|r.get(0))?;
    if !host{return Ok(None)}
    let mut q=conn.prepare_cached("SELECT i.scope,i.native_key FROM object_instances i JOIN object_facts f ON f.instance_id=i.instance_id AND f.field_id='name' AND f.value_status='known' WHERE i.type_id='account.user' AND i.machine_id=? AND i.installation_id=? AND i.identity_status='confirmed' AND json_extract(f.value,'$')=? COLLATE NOCASE AND julianday(f.observed_at)>=julianday(?) AND EXISTS(SELECT 1 FROM source_bindings b WHERE b.instance_id=i.instance_id AND b.source='pcscope.identity.accounts' AND julianday(b.valid_from)<=julianday(?) AND (b.valid_to IS NULL OR julianday(b.valid_to)>=julianday(?))) AND NOT EXISTS(SELECT 1 FROM fact_history h WHERE h.instance_id=i.instance_id AND h.field_id='name' AND h.before_status='known' AND h.after_status='known' AND h.before_value<>h.after_value) LIMIT 2")?;
    let rows=q.query_map(params![context.machine_id,context.installation_id,name,at,at,at],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    if rows.len()!=1{return Ok(None)}
    let (scope,key)=&rows[0];let Some(sid)=key.rsplit('|').next() else{return Ok(None)};
    if !pcscope_contracts::adapters::event::is_account_sid(sid){return Ok(None)}
    let candidate=pcscope_contracts::adapters::identity("account.user",scope,&input.evidence.id,&[("sid",Some(sid.to_owned()))]);
    let strategy=pcscope_contracts::identity::TypeStrategy::for_type("account.user").unwrap();
    if pcscope_contracts::identity::normalise(&strategy,&candidate).0!=*key{return Ok(None)}
    Ok(Some(candidate))
}

fn guard(conn:&Connection,instance:Option<u64>)->Result<String> {
    let Some(id)=instance else{return Ok("null".into())};
    Ok(conn.query_row("SELECT json_array(type_id,category,machine_id,installation_id,scope,native_key,incarnation,identity_status) FROM object_instances WHERE instance_id=?",[id],|r|r.get(0)).optional()?.unwrap_or_else(||"null".into()))
}

pub(crate) fn save(conn:&Connection,row:i64,instance:Option<u64>,context:&AdapterContext,action:&ActionCandidate)->Result<()> {
    let decision=if instance.is_some() {
        let confirmed=conn.query_row("SELECT identity_status='confirmed' FROM object_instances WHERE instance_id=?",[instance],|r|r.get::<_,bool>(0))?;
        if !confirmed {return Ok(())} "verified"
    }else{"unassigned"};
    let reason=match &action.target {ActionTarget::Unassigned{reason}=>Some(reason.as_str()),_=>None};
    if instance.is_none() && reason==Some("事件所属的启动周期尚未建立。") {
        if let Some(at)=action.occurred_at.as_deref().and_then(|s|pcscope_contracts::semantics::canonical_time(s).ok()) {
            conn.execute("INSERT INTO event_association_waiting(action_row,machine_id,installation_id,source_time) VALUES(?,?,?,?) ON CONFLICT(action_row) DO NOTHING",params![row,context.machine_id,context.installation_id,at])?;
        }
    } else {conn.execute("DELETE FROM event_association_waiting WHERE action_row=?",[row])?;}
    conn.execute("INSERT INTO event_associations VALUES(?,?,?,?,?,?,?) ON CONFLICT(action_row) DO UPDATE SET
        instance_id=excluded.instance_id,identity_guard=excluded.identity_guard,decision=excluded.decision,
        reason=excluded.reason,context=excluded.context,rule=excluded.rule",
        params![row,instance,guard(conn,instance)?,decision,reason,json!({"machine_id":context.machine_id,"installation_id":context.installation_id,"boot_id":context.boot_id,"scope":context.collected_scope}).to_string(),"event-association-v1"])?;Ok(())
}

pub(crate) fn stored(conn:&Connection,row:i64,instance:Option<i64>)->Result<Option<Value>> {
    let exists:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='event_associations')",[],|r|r.get(0))?;
    if !exists {return Ok(None)}
    let value:Option<(Option<i64>,String,String,Option<String>)>=conn.query_row("SELECT instance_id,identity_guard,decision,reason FROM event_associations WHERE action_row=?",[row],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
    let Some((saved,expected,status,reason))=value else{return Ok(None)};
    if saved!=instance||expected!=guard(conn,instance.map(|id|id as u64))? {return Ok(None)}
    Ok(Some(json!({"status":status,"reason":reason})))
}

#[cfg(test)]
#[path="event_association_tests.rs"]
mod tests;
