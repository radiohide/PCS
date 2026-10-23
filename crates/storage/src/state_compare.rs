//! Read-only historical inventory comparison. Re-decode retained observations, never copy today's
//! facts into yesterday. Pages are bounded; raw snapshot decoding is shared within each cutoff.
use super::*;
use pcscope_contracts::adapter::{AdapterContext, ObservationCandidate};
use std::collections::{BTreeMap, HashMap};

fn time(raw:&str)->Result<String>{Ok(chrono::DateTime::parse_from_rfc3339(raw)?.with_timezone(&chrono::Utc).to_rfc3339_opts(chrono::SecondsFormat::Nanos,true))}

// Snapshot items share one timestamp and are inserted atomically by record_snapshot_items.
// Read each run's timestamp once per page, then verify the selected item's own row and bound.
// The item check is essential: a reference to a sibling's snapshot is not this object's evidence.
const ITEM_AT_ADDRESS: &str = "SELECT 1 FROM evidence INDEXED BY evidence_source_address
    WHERE source=? AND evidence_id=? AND item_id=? AND kind='snapshot' AND seq<=?";

#[derive(Default)]
struct Cache {
    snapshots: crate::evidence::SnapshotCache,
    observations: HashMap<(String,String), Vec<ObservationCandidate>>,
    headers: HashMap<(String,String),Option<(String,i64)>>,
    timings: [std::time::Duration;5],
}
fn latest_address(conn:&Connection,object:i64,source:&str,item:&str,cutoff:&str,bound:i64,cache:&mut Cache)->Result<Option<String>> {
    let mut refs=conn.prepare_cached("SELECT evidence_id FROM evidence_refs WHERE subject_kind='observation' AND subject_id=? AND evidence_kind='snapshot'")?;
    let addresses=refs.query_map([object.to_string()],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let mut header=conn.prepare_cached("SELECT observed_at,seq FROM evidence INDEXED BY evidence_source_address WHERE source=? AND evidence_id=? AND kind='snapshot' ORDER BY item_id LIMIT 1")?;
    let mut candidates=Vec::new();
    for address in addresses {
        let key=(source.to_owned(),address.clone());
        if !cache.headers.contains_key(&key) {
            let row:Option<(String,i64)>=header.query_row(params![source,address],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            let row=row.and_then(|(raw,seq)|time(&raw).ok().map(|at|(at,seq)));
            cache.headers.insert(key.clone(),row);
        }
        if let Some((at,seq))=&cache.headers[&key] {if at.as_str()<=cutoff {candidates.push((at.clone(),*seq,address));}}
    }
    candidates.sort_unstable_by(|a,b|b.cmp(a));
    let mut item_row=conn.prepare_cached(ITEM_AT_ADDRESS)?;
    for (_,_,address) in candidates {
        if item_row.query_row(params![source,address,item,bound],|_|Ok(())).optional()?.is_some(){return Ok(Some(address))}
    }
    Ok(None)
}
struct Object { id:i64, typ:String, category:String, machine:String, installation:String, scope:String, key:String, first:String, identity:String }

#[cfg(test)]
fn answer(conn:&Connection,from:&str,until:&str,typ:Option<&str>,cursor:Option<&str>,limit:usize)->Result<QueryResponse>{answer_at(conn,from,until,typ,cursor,limit,None)}

pub(super) fn answer_at(conn:&Connection,from:&str,until:&str,typ:Option<&str>,cursor:Option<&str>,limit:usize,shared:Option<&str>)->Result<QueryResponse> {
    // SQLite's julianday rounds sub-millisecond boundaries. Preserve source precision and
    // normalize explicit offsets before comparing; invalid stored dates do not match a cutoff.
    conn.create_scalar_function("pcscope_compare_time",1,rusqlite::functions::FunctionFlags::SQLITE_UTF8|rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,|ctx|{let raw=ctx.get::<String>(0)?;Ok(time(&raw).ok())})?;
    let from=time(from)?;
    let until=time(until)?;
    anyhow::ensure!(from<=until,"基准时间不能晚于对比时间");
    let fp=fingerprint(&("state_compare",&from,&until));
    let (mark,after)=if let Some(cursor)=cursor {
        let (mark,key,id)=decode_cursor(cursor).ok_or_else(||anyhow::anyhow!("对比分页已失效，请重新对比"))?;
        anyhow::ensure!(key==typ.unwrap_or_default()&&watermark_fingerprint(&mark)==fp&&shared.is_none_or(|s|s==mark),"对比条件已改变，请重新对比");(mark,id)
    } else {(shared.map(str::to_owned).unwrap_or(watermark(conn,&fp)?),0)};
    anyhow::ensure!(watermark_fingerprint(&mark)==fp,"对比条件已改变，请重新对比");
    let evidence_bound:i64=mark.split("-e").nth(1).and_then(|s|s.split('i').next()).and_then(|s|s.parse().ok()).unwrap_or(0);
    let bound=instance_bound(&mark);
    let Some(typ)=typ else {
        let mut stmt=conn.prepare_cached("SELECT i.type_id,i.category,count(*) FROM object_instances i
            WHERE i.instance_id<=? AND pcscope_compare_time(i.first_observed_at)<=pcscope_compare_time(?) GROUP BY i.type_id,i.category ORDER BY i.category,i.type_id")?;
        let rows=stmt.query_map(params![bound,until],|r|Ok(json!({"type_id":r.get::<_,String>(0)?,"category":r.get::<_,String>(1)?,"count":r.get::<_,i64>(2)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
        return Ok(response(if rows.is_empty(){ResponseStatus::Empty}else{ResponseStatus::Loaded},json!({"from":from,"until":until,"watermark":mark,"branches":rows}),None,None));
    };
    let limit=limit.clamp(1,50);
    let mut stmt=conn.prepare_cached("SELECT instance_id,type_id,category,machine_id,installation_id,scope,native_key,first_observed_at,identity_status FROM object_instances
        WHERE type_id=? AND instance_id>? AND instance_id<=? AND pcscope_compare_time(first_observed_at)<=pcscope_compare_time(?) ORDER BY instance_id LIMIT ?")?;
    let objects=stmt.query_map(params![typ,after,bound,until,limit+1],|r|Ok(Object{id:r.get(0)?,typ:r.get(1)?,category:r.get(2)?,machine:r.get(3)?,installation:r.get(4)?,scope:r.get(5)?,key:r.get(6)?,first:r.get(7)?,identity:r.get(8)?}))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let mut left_cache=Cache::default();let mut right_cache=Cache::default();let mut rows=Vec::new();let mut last=after;let mut bytes=0;
    for object in objects.iter().take(limit) {
        let left=at(conn,object,&from,evidence_bound,action_bound(&mark),&mut left_cache)?;
        let right=if from==until {left.clone()}else{at(conn,object,&until,evidence_bound,action_bound(&mark),&mut right_cache)?};
        let row=json!({"instance_id":object.id,"type_id":object.typ,"category":object.category,"left":left,"right":right});
        let size=serde_json::to_vec(&row)?.len();
        if !rows.is_empty()&&bytes+size>2*1024*1024 {break}
        bytes+=size;last=object.id;rows.push(row);
    }
    let more=objects.len()>rows.len();
    if std::env::var_os("PCSCOPE_COMPARE_PROFILE").is_some(){eprintln!("compare phases ms (bindings, lookup, decode, fields, lifecycle): {:?}",(0..5).map(|i|(left_cache.timings[i]+right_cache.timings[i]).as_millis()).collect::<Vec<_>>());}
    Ok(response(if rows.is_empty(){ResponseStatus::Empty}else{ResponseStatus::Loaded},json!({"from":from,"until":until,"objects":rows}),Some(Page{next_cursor:more.then(||encode_cursor(&mark,typ,last)),limit,watermark:mark,has_more:more}),None))
}

fn at(conn:&Connection,object:&Object,cutoff:&str,evidence_bound:i64,action_bound:i64,cache:&mut Cache)->Result<Value> {
    let start=std::time::Instant::now();
    let first=time(&object.first)?;
    if first.as_str()>cutoff {return Ok(json!({"availability":"not_recorded","fields":[]}))}
    // One indexed address lookup per binding. A snapshot address contains many items, so the
    // item join is mandatory: a sibling's evidence must never stand in for this object.
    let mut stmt=conn.prepare_cached("SELECT DISTINCT b.source,b.item_id FROM source_bindings b WHERE b.instance_id=? AND pcscope_compare_time(b.valid_from)<=pcscope_compare_time(?)")?;
    let bindings=stmt.query_map(params![object.id,cutoff],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    cache.timings[0]+=start.elapsed();
    // Legacy provisional identities can group several source records. Never compare unrelated
    // records as one object. A single stable binding can still be compared as its own record.
    if object.identity!="confirmed"&&bindings.len()>1 {return Ok(json!({"availability":"ambiguous","source_record_count":bindings.len(),"fields":[],"summary":{"type_id":object.typ,"identity_status":object.identity}}))}
    let single_binding=bindings.len()==1;
    let mut fields:BTreeMap<String,Value>=BTreeMap::new();let mut observed:Option<String>=None;
    for (source,item) in bindings {
        let lookup=std::time::Instant::now();
        let address=latest_address(conn,object.id,&source,&item,cutoff,evidence_bound,cache)?;
        cache.timings[1]+=lookup.elapsed();let Some(address)=address else{continue};
        let decode=std::time::Instant::now();
        let key=(address.clone(),item.clone());
        if !cache.observations.contains_key(&key) {
            let decoded=if let (Some(input),Some(adapter))=(crate::evidence::input_at_cached(conn,&address,&item,&mut cache.snapshots)?.filter(|input|!input.payload.is_null()&&input.decoding.available),pcscope_contracts::reference::adapter_for(&source)) {
                let context=AdapterContext{catalog_version:pcscope_contracts::model::catalog_version().into(),rule_version:pcscope_contracts::model::catalog_fingerprint(),machine_id:object.machine.clone(),installation_id:object.installation.clone(),boot_id:crate::event_association::boot_identifier(&input.payload),collected_scope:if input.source_scope.is_empty(){format!("machine:{}",object.machine)}else{pcscope_contracts::semantics::business_scope(&input.source_scope)},complete:true};
                adapter.adapt(&input,&context).observations
            } else {Vec::new()};
            cache.observations.insert(key.clone(),decoded);
        }
        cache.timings[2]+=decode.elapsed();let fields_time=std::time::Instant::now();
        for observation in &cache.observations[&key] {
            if observation.identity.type_id!=object.typ||observation.identity.scope!=object.scope {continue}
            let Some(strategy)=pcscope_contracts::identity::TypeStrategy::for_type(&object.typ)else{continue};
            let (native,missing)=pcscope_contracts::identity::normalise(&strategy,&observation.identity);
            if native!=object.key||(!missing.is_empty()&&!single_binding)||observation.identity.ambiguous.is_some(){continue}
            let time=time(&observation.observed_at)?;
            if time.as_str()>cutoff {continue}
            if observed.as_ref().is_none_or(|t|t<&time){observed=Some(time.clone())}
            for field in &observation.fields {
                let mut view=serde_json::to_value(field)?;
                view["kind"]=view["field_kind"].clone();
                view["observed_at"]=json!(time);
                view["label"]=json!(pcscope_contracts::model::field_definition(&object.typ,&field.field_id).map(|(label,_)|label).unwrap_or_default());
                // Same-time contradictory sources are visible as a conflict, never chosen by order.
                if let Some(prior)=fields.get_mut(&field.field_id) {
                    if matches!(view["status"].as_str(),Some("not_provided"|"not_applicable")) {continue}
                    let old=prior["observed_at"].as_str().unwrap_or("");
                    if old>time.as_str(){continue}
                    if old==time&&prior["status"]=="known"&&view["status"]=="known"&&prior["value"]!=view["value"] {prior["status"]=json!("conflicting");prior["value"]=Value::Null;continue}
                }
                fields.insert(field.field_id.clone(),view);
            }
        }
        cache.timings[3]+=fields_time.elapsed();
    }
    let lifecycle_time=std::time::Instant::now();
    let absent:Option<String>=conn.query_row("SELECT observed_at FROM object_actions INDEXED BY object_actions_instance WHERE instance_id=? AND action_id='observed.no_longer_observed' AND action_row<=? AND pcscope_compare_time(observed_at)<=pcscope_compare_time(?) ORDER BY pcscope_compare_time(observed_at) DESC,action_row DESC LIMIT 1",params![object.id,action_bound,cutoff],|r|r.get(0)).optional()?;
    let absent=absent.map(|s|time(&s)).transpose()?;
    let mut lifecycle=if absent.as_ref().is_some_and(|a|observed.as_ref().is_none_or(|t|a>=t)){"not_observed"}else if observed.is_some(){"active"}else{"unknown"};
    let mut checked=if lifecycle=="not_observed"{absent.clone()}else{observed.clone()};
    // The same explicit end actions used by projection may close an instance between inventories.
    let ended:Option<(String,String)>=conn.query_row("SELECT action_id,observed_at FROM object_actions INDEXED BY object_actions_instance WHERE instance_id=? AND action_row<=? AND action_id IN ('startup.task_deleted','identity.account_deleted','files.deleted','files.deleted_archived','execution.process_stopped') AND pcscope_compare_time(observed_at)<=pcscope_compare_time(?) AND pcscope_compare_time(coalesce(occurred_at,observed_at))<=pcscope_compare_time(?) ORDER BY pcscope_compare_time(observed_at) DESC,action_row DESC LIMIT 1",params![object.id,action_bound,cutoff,cutoff],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    if let Some((action,at))=ended {let at=time(&at)?;if checked.as_ref().is_none_or(|c|&at>c){lifecycle=if action=="execution.process_stopped"{"ended"}else{"removed"};checked=Some(at)}}
    let mut event_name=None;let mut event_only=false;
    if checked.is_none() {
        let event:Option<(i64,String,String)>=conn.query_row("SELECT action_row,action_id,observed_at FROM object_actions INDEXED BY object_actions_instance WHERE instance_id=? AND action_row<=? AND action_id NOT LIKE 'observed.%' AND pcscope_compare_time(observed_at)<=pcscope_compare_time(?) ORDER BY pcscope_compare_time(observed_at) DESC,action_row DESC LIMIT 1",params![object.id,action_bound,cutoff],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        if let Some((id,action,at))=event {
            let context=query_views::action_context(conn,id,Some(object.id),Some(&object.typ),&action,None,&mut HistoryLabelCache::new())?;
            if context["subject_basis"]=="event_evidence" {event_name=context["subject_label"].as_str().map(str::to_owned)}
            event_only=true;checked=Some(time(&at)?);
        }
    }
    let fields:Vec<Value>=fields.into_values().collect();
    let name=fields.iter().find(|f|matches!(f["field_id"].as_str(),Some("display_name"|"name"))&&f["status"]=="known").and_then(|f|f["value"].as_str()).or(event_name.as_deref());
    cache.timings[4]+=lifecycle_time.elapsed();
    Ok(json!({"availability":if event_only{"event_only"}else if checked.is_some(){"recorded"}else{"unavailable"},"checked_at":checked,"summary":{"instance_id":object.id,"type_id":object.typ,"category":object.category,"display_name":name,"identity_status":object.identity,"lifecycle":lifecycle,"record_basis":if event_only{"event_only"}else{"observation"},"key_facts":fields},"fields":fields}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Store;
    fn ingest(store:&mut Store,run:&str,at:&str,items:Value,complete:bool){
        let envelope=pcscope_contracts::parse(&json!({"protocol_version":1,"request_id":run,"type":"snapshot","source":"pcscope.startup.services","scope":"machine:M/install:I/schema:1/context:/shard:0","run_id":run,"observed_at":at,"complete":complete,"items":items}).to_string()).unwrap();
        let result=store.ingest(&envelope);if !complete {assert!(result.is_err());return}result.unwrap();store.project_pending("M","I",1000).unwrap();
    }
    fn item(id:&str,name:&str,state:&str)->Value{json!({"id":id,"data":{"name":id,"display_name":name,"state":state,"start_mode":"Manual"}})}
    fn read(store:&Store,a:&str,b:&str)->Value{answer(&store.conn,a,b,Some("startup.service"),None,50).unwrap().data.unwrap()["objects"].clone()}
    fn value(side:&Value,id:&str)->Value{side["fields"].as_array().unwrap().iter().find(|f|f["field_id"]==id).unwrap()["value"].clone()}
    #[test]
    fn historical_cutoffs_use_original_names_states_and_exact_boundaries(){
        let mut s=Store::open(":memory:").unwrap();
        ingest(&mut s,"one","2026-10-22T00:00:00Z",json!([item("VSS","Old name","Stopped")]),true);
        ingest(&mut s,"two","2026-10-22T01:00:00Z",json!([item("VSS","New name","Running")]),true);
        let rows=read(&s,"2026-10-22T00:30:00Z","2026-10-22T01:00:00Z");
        assert_eq!(rows.as_array().unwrap().len(),1);
        assert_eq!(value(&rows[0]["left"],"state"),"Stopped");assert_eq!(value(&rows[0]["right"],"state"),"Running");
        assert_eq!(value(&rows[0]["left"],"display_name"),"Old name");
        assert_eq!(rows[0]["left"]["checked_at"],"2026-10-22T00:00:00.000000000Z");
        let same=read(&s,"2026-10-22T01:00:00Z","2026-10-22T01:00:00Z");assert_eq!(same[0]["left"],same[0]["right"]);
    }
    #[test]
    fn history_lookup_addresses_one_item_in_a_shared_inventory(){
        let mut s=Store::open(":memory:").unwrap();
        let items:Vec<Value>=(0..128).map(|n|item(&format!("service-{n}"),&format!("Service {n}"),"Stopped")).collect();
        ingest(&mut s,"one","2026-10-22T00:00:00Z",json!(items),true);
        // Later material for siblings must not replace the missing item's own earlier state.
        ingest(&mut s,"two","2026-10-22T01:00:00Z",json!([item("service-1","Service 1","Running")]),true);
        let rows=read(&s,"2026-10-22T00:00:00Z","2026-10-22T01:30:00Z");
        let target=rows.as_array().unwrap().iter().find(|p|value(&p["left"],"name")=="service-0").unwrap();
        assert_eq!(value(&target["right"],"state"),"Stopped");
        assert_eq!(target["right"]["summary"]["lifecycle"],"not_observed");
        let mut plan=s.conn.prepare(&format!("EXPLAIN QUERY PLAN {ITEM_AT_ADDRESS}")).unwrap();
        let steps=plan.query_map(params!["pcscope.startup.services","address","service-0",i64::MAX],|r|r.get::<_,String>(3)).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();
        assert!(steps.iter().any(|s|s.contains("evidence_source_address")&&s.contains("item_id=?")),"{steps:?}");
    }
    #[test]
    fn shared_headers_preserve_same_time_order_watermarks_and_missing_items(){
        let mut s=Store::open(":memory:").unwrap();
        let at="2026-10-22T00:00:00Z";
        ingest(&mut s,"one",at,json!([item("A","A","Stopped"),item("B","B","Running")]),true);
        let directory=answer(&s.conn,at,at,None,None,50).unwrap();let mark=directory.data.unwrap()["watermark"].as_str().unwrap().to_owned();
        ingest(&mut s,"two",at,json!([item("B","B","Stopped"),item("A","A","Running")]),true);
        let rows=read(&s,at,at);assert_eq!(value(&rows[0]["right"],"state"),"Running");
        let bounded=answer_at(&s.conn,at,at,Some("startup.service"),None,50,Some(&mark)).unwrap().data.unwrap();assert_eq!(value(&bounded["objects"][0]["right"],"state"),"Stopped");
        ingest(&mut s,"three","2026-10-22T01:00:00Z",json!([item("B","B","Paused")]),true);
        let a=rows[0]["instance_id"].as_i64().unwrap();
        s.conn.execute("INSERT OR IGNORE INTO evidence_refs(subject_kind,subject_id,evidence_kind,evidence_id) SELECT 'observation',?,'snapshot',evidence_id FROM evidence WHERE item_id='B'",[a.to_string()]).unwrap();
        let rows=read(&s,at,"2026-10-22T01:30:00Z");assert_eq!(value(&rows[0]["right"],"state"),"Running");
        assert_eq!(rows[0]["right"]["summary"]["lifecycle"],"not_observed");
    }
    #[test]
    fn absent_reappearing_and_first_observation_are_distinct(){
        let mut s=Store::open(":memory:").unwrap();
        ingest(&mut s,"one","2026-10-22T00:00:00Z",json!([item("VSS","Backup","Stopped")]),true);
        ingest(&mut s,"two","2026-10-22T01:00:00Z",json!([]),true);
        let rows=read(&s,"2026-10-22T00:00:00Z","2026-10-22T01:30:00Z");assert_eq!(rows[0]["left"]["summary"]["lifecycle"],"active");assert_eq!(rows[0]["right"]["summary"]["lifecycle"],"not_observed");
        ingest(&mut s,"three","2026-10-22T02:00:00Z",json!([item("VSS","Backup","Running")]),true);
        let rows=read(&s,"2026-10-22T01:30:00Z","2026-10-22T02:00:00Z");assert_eq!(rows[0]["right"]["summary"]["lifecycle"],"active");
        let first=read(&s,"2026-10-21T00:00:00Z","2026-10-22T00:00:00Z");assert_eq!(first[0]["left"]["availability"],"not_recorded");
    }
    #[test]
    fn incomplete_inventory_does_not_remove_objects_and_missing_payload_never_uses_current_values(){
        let mut s=Store::open(":memory:").unwrap();
        ingest(&mut s,"one","2026-10-22T00:00:00Z",json!([item("VSS","Backup","Stopped")]),true);
        ingest(&mut s,"two","2026-10-22T01:00:00Z",json!([]),false);
        let rows=read(&s,"2026-10-22T00:00:00Z","2026-10-22T01:00:00Z");assert_eq!(rows[0]["right"]["summary"]["lifecycle"],"active");
        s.conn.execute("DELETE FROM snapshots",[]).unwrap();
        let rows=read(&s,"2026-10-22T00:00:00Z","2026-10-22T01:00:00Z");assert_eq!(rows[0]["left"]["availability"],"unavailable");assert_eq!(rows[0]["left"]["fields"],json!([]));
    }
    #[test]
    fn paging_is_bounded_and_rejects_changed_cutoffs(){
        let mut s=Store::open(":memory:").unwrap();
        ingest(&mut s,"one","2026-10-22T00:00:00Z",json!([item("A","A","Stopped"),item("B","B","Running")]),true);
        let first=answer(&s.conn,"2026-10-22T00:00:00Z","2026-10-22T01:00:00Z",Some("startup.service"),None,1).unwrap();
        let cursor=first.page.unwrap().next_cursor.unwrap();
        ingest(&mut s,"two","2026-10-22T00:30:00Z",json!([item("A","A","Running"),item("B","B","Stopped"),item("C","C","Running")]),true);
        let next=answer(&s.conn,"2026-10-22T00:00:00Z","2026-10-22T01:00:00Z",Some("startup.service"),Some(&cursor),1).unwrap();
        assert!(!next.page.unwrap().has_more);assert_eq!(value(&next.data.unwrap()["objects"][0]["right"],"state"),"Running");
        assert!(answer(&s.conn,"2026-10-22T00:00:00Z","2026-10-22T02:00:00Z",Some("startup.service"),Some(&cursor),1).is_err());
        assert!(answer(&s.conn,"bad","bad",None,None,50).is_err());
        assert!(answer(&s.conn,"2026-10-22T02:00:00Z","2026-10-22T00:00:00Z",None,None,50).is_err());
    }
    #[test]
    fn offset_timestamps_and_explicit_end_events_are_historical(){
        let mut s=Store::open(":memory:").unwrap();
        ingest(&mut s,"one","2026-10-22T08:00:00+08:00",json!([item("VSS","Backup","Stopped")]),true);
        let rows=read(&s,"2026-10-22T00:00:00Z","2026-10-22T00:30:00Z");assert_eq!(value(&rows[0]["right"],"state"),"Stopped");
        s.conn.execute("INSERT INTO object_actions(instance_id,action_id,title,result,observed_at,precision,rule_version) SELECT instance_id,'execution.process_stopped','Ended','ended','2026-10-22T01:00:00Z','exact','test' FROM object_instances",[]).unwrap();
        let rows=read(&s,"2026-10-22T00:30:00Z","2026-10-22T01:00:00Z");assert_eq!(rows[0]["left"]["summary"]["lifecycle"],"active");assert_eq!(rows[0]["right"]["summary"]["lifecycle"],"ended");
        assert!(time("2026-02-30T00:00:00Z").is_err());
    }
    #[test]
    fn submillisecond_cutoffs_do_not_select_future_observations(){
        let mut s=Store::open(":memory:").unwrap();
        ingest(&mut s,"one","2026-10-22T00:00:00Z",json!([item("VSS","Backup","Stopped")]),true);
        ingest(&mut s,"two","2026-10-22T01:00:00.000000001Z",json!([item("VSS","Backup","Running")]),true);
        let rows=read(&s,"2026-10-22T01:00:00Z","2026-10-22T01:00:00.000000001Z");assert_eq!(value(&rows[0]["left"],"state"),"Stopped");assert_eq!(value(&rows[0]["right"],"state"),"Running");
    }
    #[test]
    fn provisional_records_keep_their_identity_status_and_multiple_bindings_are_not_blended(){
        let mut s=Store::open(":memory:").unwrap();
        ingest(&mut s,"one","2026-10-22T00:00:00Z",json!([item("VSS","Backup","Stopped")]),true);
        s.conn.execute("UPDATE object_instances SET identity_status='provisional'",[]).unwrap();
        let rows=read(&s,"2026-10-22T00:00:00Z","2026-10-22T00:30:00Z");assert_eq!(rows[0]["left"]["availability"],"recorded");assert_eq!(rows[0]["left"]["summary"]["identity_status"],"provisional");
        s.conn.execute("INSERT INTO source_bindings(instance_id,source,item_id,scope,native_record_id,valid_from) SELECT instance_id,source,'other',scope,'other','2026-10-22T01:00:00Z' FROM source_bindings LIMIT 1",[]).unwrap();
        let rows=read(&s,"2026-10-22T00:30:00Z","2026-10-22T01:00:00Z");assert_eq!(rows[0]["left"]["availability"],"recorded");assert_eq!(rows[0]["right"]["availability"],"ambiguous");assert_eq!(rows[0]["right"]["fields"],json!([]));
    }
}
