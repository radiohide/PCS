//! Bounded, read-only labels from collected metadata and exactly cited snapshots.
//! Historical process labels stay qualified as records and never establish current identity.
use anyhow::Result;
use pcscope_contracts::adapter::{AdapterContext, Decoding, EvidenceInput, EvidenceKind, EvidenceRef, ValueStatus};
use rusqlite::{params, Connection};
use serde_json::Value;
use std::collections::{HashMap,VecDeque};

#[derive(Clone)]
struct ProcessLabel {pid:Option<u64>,started:Option<String>,name:Option<String>}
type SnapshotLabels=HashMap<String,Option<ProcessLabel>>;
/// One query/maintenance transaction only. Retain compact labels, never whole decoded snapshots.
/// The limits bound both retained entries and a single source material's parsing work.
#[derive(Default)]
pub(crate) struct ProcessLabelCache {
    snapshots:VecDeque<((String,String,String),Option<SnapshotLabels>)>,
    entries:usize,
    #[cfg(test)] loads:usize,
}
impl ProcessLabelCache {
    fn get(&mut self,conn:&Connection,source:&str,scope:&str,run:&str,item:&str)->Result<Option<ProcessLabel>> {
        let key=(source.to_owned(),scope.to_owned(),run.to_owned());
        if let Some(index)=self.snapshots.iter().position(|(candidate,_)|candidate==&key){
            let cached=self.snapshots.remove(index).unwrap();
            let result=cached.1.as_ref().and_then(|labels|labels.get(item)).cloned().flatten();
            self.snapshots.push_back(cached);return Ok(result);
        }
        #[cfg(test)]{self.loads+=1;}
        use rusqlite::OptionalExtension;
        let payload:Option<Option<String>>=conn.prepare_cached("SELECT CASE WHEN length(payload)<=2097152 THEN payload ELSE NULL END FROM snapshots WHERE source=? AND scope=? AND run_id=?")?
            .query_row(params![source,scope,run],|r|r.get(0)).optional()?;
        let labels=payload.flatten().and_then(|raw|crate::snapshot_content::resolve_bounded(conn,&raw,Some(2097152)).ok()).and_then(|whole|{
            let items=whole["items"].as_array().filter(|items|items.len()<=2048)?;
            let mut labels=HashMap::new();
            for item in items {
                let id=item["id"].as_str().filter(|id|id.len()<=512)?;
                if labels.contains_key(id){labels.insert(id.to_owned(),None);continue;}
                let value=&item["data"];
                let protected=[&value["name"],&value["path"]].iter().any(|v|v.is_object()||pcscope_contracts::semantics::is_mask_sentinel(v));
                let name=if protected{None}else{value["name"].as_str().map(str::trim).filter(|s|!s.is_empty()&&s.len()<=512&&!s.contains(['\\','/','\n','\r'])).map(str::to_owned)};
                labels.insert(id.to_owned(),Some(ProcessLabel{pid:value["pid"].as_u64(),started:value["start_time"].as_str().filter(|s|s.len()<=128).map(str::to_owned),name}));
            }Some(labels)
        });
        let count=labels.as_ref().map_or(0,HashMap::len);
        while self.snapshots.len()>=8||self.entries+count>8192 {
            if let Some((_,old))=self.snapshots.pop_front(){self.entries-=old.as_ref().map_or(0,HashMap::len);}else{break;}
        }
        let result=labels.as_ref().and_then(|labels|labels.get(item)).cloned().flatten();
        self.entries+=count;self.snapshots.push_back((key,labels));Ok(result)
    }
}

const SOURCE: &str = "pcscope.shell.registrations";
const MAX_SCOPES: usize = 256;

/// A legacy process omitted creation time during projection. Recover only a label from its
/// exact collected item, never from the current PID. Multiple generations remain unresolved.
pub(super) fn legacy_process_label_cached(conn:&Connection, instance:i64, scope:&str, pid:u64, expected_started:Option<&str>,cache:&mut ProcessLabelCache)->Result<Option<String>> {
    let source="pcscope.execution.processes";
    let mut statement=conn.prepare_cached("SELECT DISTINCT item_id,native_record_id FROM source_bindings WHERE instance_id=? AND source=? AND scope=? LIMIT 3")?;
    let bindings=statement.query_map(params![instance,source,scope],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    if bindings.len()!=1 || bindings[0].0!=bindings[0].1 {return Ok(None);}
    let item=&bindings[0].0;
    let Some((number,started))=item.split_once('/') else{return Ok(None);};
    if number.parse::<u64>().ok()!=Some(pid) || pcscope_contracts::semantics::canonical_time(started).is_err() {return Ok(None);}
    if expected_started.is_some_and(|expected|expected!=started) {return Ok(None);}
    let mut statement=conn.prepare_cached("SELECT scope FROM scopes WHERE source=? LIMIT 257")?;
    let scopes=statement.query_map([source],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    if scopes.len()>MAX_SCOPES {return Ok(None);}
    let mut result=None;
    let mut statement=conn.prepare_cached("SELECT CASE WHEN length(data)<=262144 THEN data ELSE NULL END FROM objects WHERE source=? AND scope=? AND id=?")?;
    for collected in scopes {
        if pcscope_contracts::adapters::installation_scope(&pcscope_contracts::semantics::business_scope(&collected))!=scope {continue;}
        for raw in statement.query_map(params![source,collected,item],|r|r.get::<_,Option<String>>(0))? {
            let Some(raw)=raw? else{return Ok(None);};
            let Ok(value)=serde_json::from_str::<Value>(&raw) else{return Ok(None);};
            if value["pid"].as_u64()!=Some(pid) || value["start_time"].as_str()!=Some(started) {return Ok(None);}
            // The source's name is safe when path access was denied, but a protected name/path
            // must never be reconstructed using a less protected copy.
            if [&value["name"],&value["path"]].iter().any(|v|v.is_object()||pcscope_contracts::semantics::is_mask_sentinel(v)) {return Ok(None);}
            let Some(name)=value["name"].as_str().map(str::trim).filter(|s|!s.is_empty()&&s.len()<=512&&!s.contains(['\\','/','\n','\r'])) else{return Ok(None);};
            if result.as_ref().is_some_and(|old|old!=name) {return Ok(None);}
            result=Some(name.to_owned());
        }
    }
    if result.is_none() {
        // The latest inventory removes exited processes. Their immutable, exactly cited snapshot
        // can still name the old record. Read at most eight indexed references and bounded payloads.
        let mut refs=conn.prepare_cached("SELECT evidence_id,pruned_at FROM evidence_refs WHERE subject_kind='observation' AND subject_id=? AND evidence_kind='snapshot' ORDER BY evidence_id DESC LIMIT 8")?;
        let refs=refs.query_map([instance.to_string()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,Option<String>>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        use rusqlite::OptionalExtension;
        for (reference,pruned) in refs {
            if pruned.is_some() {return Ok(None);}
            let evidence=conn.query_row("SELECT source_scope,payload_ref,decoding,pruned_at FROM all_evidence WHERE evidence_id=? AND item_id=? AND source=? AND kind='snapshot'",params![reference,item,source],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?))).optional()?;
            let Some((collected,address,decoding,pruned))=evidence else{continue;};
            if pruned.is_some() {return Ok(None);}
            let Ok(decoded)=serde_json::from_str::<Decoding>(&decoding) else{return Ok(None);};
            if !decoded.available || decoded.masked_fields.iter().chain(decoded.unreadable_fields.iter()).any(|f|["name","path","pid","start_time"].contains(&f.as_str())) {return Ok(None);}
            if pcscope_contracts::adapters::installation_scope(&pcscope_contracts::semantics::business_scope(&collected))!=scope {continue;}
            let parts:Vec<_>=address.strip_prefix("snapshot:").unwrap_or_default().splitn(4,'|').collect();
            if parts.len()!=4 || parts[0]!=source || parts[1]!=collected || parts[3]!=item {return Ok(None);}
            let Some(label)=cache.get(conn,source,&collected,parts[2],item)?else{return Ok(None)};
            if label.pid!=Some(pid)||label.started.as_deref()!=Some(started){return Ok(None)}
            let Some(name)=label.name else{return Ok(None)};
            if result.as_ref().is_some_and(|old|old!=&name){return Ok(None)}
            result=Some(name);
        }
    }
    Ok(result)
}

/// Resolve a membership SID against collected accounts in the same installation and scope.
/// No OS lookup and no name-based identity join; missing/protected/conflicting values stay absent.
pub(super) fn member_label(conn:&Connection, instance:i64, scope:&str, sid:&str)->Result<Option<String>> {
    let Some(strategy)=pcscope_contracts::identity::TypeStrategy::for_type("account.user") else{return Ok(None);};
    let candidate=pcscope_contracts::adapters::identity("account.user",scope,"label",&[("sid",Some(sid.into()))]);
    let (key,missing)=pcscope_contracts::identity::normalise(&strategy,&candidate);
    if !missing.is_empty() {return Ok(None);}
    let mut statement=conn.prepare_cached("SELECT f.value,f.value_status FROM object_instances m JOIN object_instances u ON u.type_id='account.user' AND u.machine_id=m.machine_id AND u.installation_id=m.installation_id AND u.scope=m.scope AND u.native_key=? AND u.identity_status='confirmed' JOIN object_facts f ON f.instance_id=u.instance_id AND f.field_id='name' WHERE m.instance_id=? LIMIT 3")?;
    let rows=statement.query_map(params![key,instance],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    if rows.len()!=1 || rows[0].1!="known" {return Ok(None);}
    let value:Value=serde_json::from_str(&rows[0].0)?;
    Ok(value.as_str().map(str::trim).filter(|s|!s.is_empty()&&s.len()<=512&&!pcscope_contracts::semantics::is_mask_sentinel(&value)).map(str::to_owned))
}
pub(super) const LEGACY_COM_RULES: &[&str] = &[
    "143e914bf62024e7681f6b36f511220e2b11845eb69f3af8253c76140cfcb0c7",
    "154b198b4b59a0f3c2c26a39105bf11037f57e1baf44561bff1b11be6d8c181b",
    "b7415b4f9ffce3ef675787bef5ebdb450f58215262e6f1edc01257e92000f635",
];

#[derive(Clone, Default)]
pub(super) struct ComMetadata {
    pub label: Option<String>,
    pub mixed_roles: bool,
}

pub(super) fn com_metadata(conn: &Connection, instance: i64, current: &Value) -> Result<ComMetadata> {
    let mut result=ComMetadata::default();
    if current["type_id"]!="shell.com_registration" || current["identity_status"]!="confirmed" { return Ok(result); }
    let Some(key)=current["native_key"].as_str() else { return Ok(result); };
    let parts:Vec<_>=key.split('|').collect();
    if parts.len()!=3 || !["localmachine","currentuser","classesroot","users","currentconfig"].contains(&parts[0].to_ascii_lowercase().as_str())
        || !["registry32","registry64"].contains(&parts[1].to_ascii_lowercase().as_str()) { return Ok(result); }
    let clsid=parts[2];
    if !clsid.is_ascii() || clsid.len()!=38 || !clsid.starts_with('{') || !clsid.ends_with('}')
        || !clsid[1..37].bytes().enumerate().all(|(index,byte)|if [8,13,18,23].contains(&index){byte==b'-'}else{byte.is_ascii_hexdigit()}) { return Ok(result); }
    let root=format!("software\\classes\\clsid\\{}",clsid.to_ascii_lowercase());
    let scope=current["scope"].as_str().unwrap_or_default();
    let mut bindings=conn.prepare_cached("SELECT item_id,native_record_id FROM source_bindings WHERE instance_id=? AND source=? AND scope=? LIMIT 65")?;
    let records=bindings.query_map(params![instance,SOURCE,scope],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    if records.len()>64 { return Ok(result); }
    let bound=|path:&str|records.iter().any(|(item,native)|item.eq_ignore_ascii_case(path)&&native.eq_ignore_ascii_case(path));
    if !bound(&root) { return Ok(result); }
    result.mixed_roles=bound(&format!("{root}\\inprocserver32"))||bound(&format!("{root}\\localserver32"));

    // The objects primary key is (source,scope,id). Starting at the much smaller scopes table
    // avoids scanning every shell record for a single class id. Refuse an abnormal scope count.
    let mut scopes=conn.prepare_cached("SELECT scope FROM scopes WHERE source=? ORDER BY scope LIMIT 257")?;
    let scopes=scopes.query_map([SOURCE],|row|row.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    if scopes.len()>MAX_SCOPES { return Ok(result); }
    let Some(adapter)=pcscope_contracts::adapters::adapter_for(SOURCE) else {return Ok(result);};
    let Some(strategy)=pcscope_contracts::identity::TypeStrategy::for_type("shell.com_registration") else {return Ok(result);};
    let mut statement=conn.prepare_cached("SELECT CASE WHEN length(data)<=262144 THEN data ELSE NULL END FROM objects WHERE source=? AND scope=? AND id=?")?;
    let mut blocked=false;
    for collected_scope in scopes {
        let business=pcscope_contracts::semantics::business_scope(&collected_scope);
        let registration_scope=if parts[0].eq_ignore_ascii_case("CurrentUser") {business.clone()} else {pcscope_contracts::adapters::installation_scope(&business)};
        if registration_scope!=scope {continue;}
        let context=AdapterContext{catalog_version:pcscope_contracts::model::catalog_version().into(),rule_version:String::new(),
            machine_id:current["machine_id"].as_str().unwrap_or_default().into(),installation_id:current["installation_id"].as_str().unwrap_or_default().into(),
            boot_id:None,collected_scope:business,complete:true};
        let rows=statement.query_map(params![SOURCE,collected_scope,root],|row|row.get::<_,Option<String>>(0))?;
        for raw in rows {
            let Some(raw)=raw? else {blocked=true;continue;};
            let Ok(payload)=serde_json::from_str(&raw) else {blocked=true;continue;};
            let input=EvidenceInput{source:SOURCE.into(),item_id:root.clone(),native_record_id:root.clone(),source_scope:collected_scope.clone(),
                evidence:EvidenceRef{kind:EvidenceKind::Snapshot,id:"collected registry metadata".into()},source_time:None,
                observed_at:String::new(),ingested_at:String::new(),structure_version:String::new(),decoding:Decoding{available:true,..Default::default()},payload};
            for observation in adapter.adapt(&input,&context).observations {
                let (native_key,missing)=pcscope_contracts::identity::normalise(&strategy,&observation.identity);
                if observation.identity.type_id!="shell.com_registration" || observation.identity.scope!=scope || native_key!=key || !missing.is_empty() {continue;}
                let Some(name)=observation.fields.iter().find(|field|field.field_id=="name") else {continue;};
                if matches!(name.status,ValueStatus::Masked|ValueStatus::Unreadable|ValueStatus::Conflicting|ValueStatus::EvidencePruned) {blocked=true;continue;}
                let Some(text)=name.value.as_ref().filter(|_|name.status==ValueStatus::Known).and_then(Value::as_str).map(str::trim)
                    .filter(|text|!text.is_empty()&&*text!=pcscope_contracts::sensitive::MASK&&text.len()<=1024) else {continue;};
                if result.label.as_ref().is_some_and(|previous|previous!=text) {blocked=true;} else {result.label=Some(text.into());}
            }
        }
    }
    if blocked {result.label=None;}
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    const ROOT:&str="software\\classes\\clsid\\{11111111-1111-1111-1111-111111111111}";
    fn setup()->(crate::Store,Value) {
        let store=crate::Store::open(":memory:").unwrap();
        let current=json!({"type_id":"shell.com_registration","identity_status":"confirmed","scope":"machine:M/LocalMachine/Registry64",
            "native_key":"LocalMachine|Registry64|{11111111-1111-1111-1111-111111111111}","machine_id":"M","installation_id":"I"});
        store.conn.execute("INSERT INTO object_instances(instance_id,type_id,category,machine_id,installation_id,scope,native_key,identity_status,lifecycle,first_observed_at,last_observed_at) VALUES(1,'shell.com_registration','shell','M','I',?,?,'confirmed','active','2026-10-12','2026-10-12')",params![current["scope"].as_str().unwrap(),current["native_key"].as_str().unwrap()]).unwrap();
        for path in [ROOT.to_owned(),format!("{ROOT}\\inprocserver32")] {
            store.conn.execute("INSERT INTO source_bindings(instance_id,source,item_id,scope,native_record_id,valid_from) VALUES(1,?,?,?,?,'2026-10-12')",params![SOURCE,path,current["scope"].as_str().unwrap(),path]).unwrap();
        }
        (store,current)
    }
    fn metadata(store:&crate::Store,scope:&str,label:Value) {
        store.conn.execute("INSERT OR IGNORE INTO scopes(source,scope) VALUES(?,?)",params![SOURCE,scope]).unwrap();
        store.conn.execute("INSERT OR REPLACE INTO objects(source,scope,id,data) VALUES(?,?,?,?)",params![SOURCE,scope,ROOT,json!({"registry_path":ROOT,"values":{"":{"value":label}}}).to_string()]).unwrap();
    }
    #[test]
    fn collected_com_names_require_matching_scope_view_key_and_root_binding() {
        let (store,mut current)=setup();
        metadata(&store,"machine:Other/user:A/schema:1/LocalMachine/Registry64/shard:0",json!("another machine"));
        metadata(&store,"machine:M/user:A/schema:1/CurrentUser/Registry64/shard:0",json!("another hive"));
        metadata(&store,"machine:M/user:A/schema:1/LocalMachine/Registry32/shard:0",json!("another view"));
        assert!(com_metadata(&store.conn,1,&current).unwrap().label.is_none());
        metadata(&store,"machine:M/user:A/schema:1/LocalMachine/Registry64/shard:0",json!("图片预览组件"));
        let found=com_metadata(&store.conn,1,&current).unwrap();
        assert_eq!(found.label.as_deref(),Some("图片预览组件"));assert!(found.mixed_roles);
        current["identity_status"]=json!("provisional");assert!(com_metadata(&store.conn,1,&current).unwrap().label.is_none());
        current["identity_status"]=json!("confirmed");
        store.conn.execute("DELETE FROM source_bindings WHERE item_id=?",[ROOT]).unwrap();
        assert!(com_metadata(&store.conn,1,&current).unwrap().label.is_none());
    }
    #[test]
    fn collected_com_name_conflicts_masks_and_scope_budget_never_pick_a_convenient_value() {
        for other in [json!("conflicting description"),json!(pcscope_contracts::sensitive::MASK),json!({"sensitive":true})] {
            let (store,current)=setup();
            metadata(&store,"machine:M/user:A/schema:1/LocalMachine/Registry64/shard:0",json!("图片预览组件"));
            metadata(&store,"machine:M/user:B/schema:1/LocalMachine/Registry64/shard:0",other);
            assert!(com_metadata(&store.conn,1,&current).unwrap().label.is_none());
        }
        let (store,current)=setup();
        metadata(&store,"machine:M/user:A/schema:1/LocalMachine/Registry64/shard:0",json!("图片预览组件"));
        for n in 0..256 {store.conn.execute("INSERT INTO scopes(source,scope) VALUES(?,?)",params![SOURCE,format!("unrelated-{n}")]).unwrap();}
        assert!(com_metadata(&store.conn,1,&current).unwrap().label.is_none());
    }
    #[test]
    fn current_user_com_names_do_not_cross_users() {
        let (store,mut current)=setup();
        current["scope"]=json!("machine:M/user:A/CurrentUser/Registry64");
        current["native_key"]=json!("CurrentUser|Registry64|{11111111-1111-1111-1111-111111111111}");
        store.conn.execute("UPDATE source_bindings SET scope=?",[current["scope"].as_str().unwrap()]).unwrap();
        metadata(&store,"machine:M/user:B/schema:1/CurrentUser/Registry64/shard:0",json!("another user's component"));
        assert!(com_metadata(&store.conn,1,&current).unwrap().label.is_none());
        metadata(&store,"machine:M/user:A/schema:1/CurrentUser/Registry64/shard:0",json!("本账户组件"));
        assert_eq!(com_metadata(&store.conn,1,&current).unwrap().label.as_deref(),Some("本账户组件"));
    }
}

#[cfg(test)]
mod process_cache_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn compact_snapshot_labels_reuse_material_and_keep_protected_duplicates_unusable() {
        let store=crate::Store::open(":memory:").unwrap();let conn=store.reader();
        let material=json!({"items":[
            {"id":"42/start","data":{"pid":42,"start_time":"start","name":"one.exe","path":"C:\\one.exe"}},
            {"id":"43/start","data":{"pid":43,"start_time":"start","name":"two.exe","path":"C:\\two.exe"}},
            {"id":"hidden","data":{"pid":44,"name":"secret.exe","path":{"sensitive":true}}},
            {"id":"duplicate","data":{"name":"first.exe"}},{"id":"duplicate","data":{"name":"second.exe"}}
        ]});
        conn.execute("INSERT INTO snapshots(source,scope,run_id,payload) VALUES('source','scope','run',?)",[material.to_string()]).unwrap();
        let mut cache=ProcessLabelCache::default();
        assert_eq!(cache.get(conn,"source","scope","run","42/start").unwrap().unwrap().name.as_deref(),Some("one.exe"));
        assert_eq!(cache.get(conn,"source","scope","run","43/start").unwrap().unwrap().pid,Some(43));
        assert!(cache.get(conn,"source","scope","run","hidden").unwrap().unwrap().name.is_none());
        assert!(cache.get(conn,"source","scope","run","duplicate").unwrap().is_none());
        assert_eq!(cache.loads,1);
        assert!(cache.get(conn,"source","another-scope","run","42/start").unwrap().is_none());
        assert_eq!(cache.loads,2);
    }
    #[test]
    fn compact_label_cache_evicts_old_snapshots_instead_of_growing_with_history() {
        let store=crate::Store::open(":memory:").unwrap();let conn=store.reader();let mut cache=ProcessLabelCache::default();
        let material=json!({"items":(0..1200).map(|id|json!({"id":id.to_string(),"data":{"pid":id,"name":"example.exe"}})).collect::<Vec<_>>()});
        for run in 0..12 {
            conn.execute("INSERT INTO snapshots(source,scope,run_id,payload) VALUES('source','scope',?,?)",params![run.to_string(),material.to_string()]).unwrap();
            assert!(cache.get(conn,"source","scope",&run.to_string(),"1").unwrap().is_some());
            assert!(cache.entries<=8192);assert!(cache.snapshots.len()<=8);
        }
        assert_eq!(cache.loads,12);
        cache.get(conn,"source","scope","0","1").unwrap();assert_eq!(cache.loads,13);
    }
}
