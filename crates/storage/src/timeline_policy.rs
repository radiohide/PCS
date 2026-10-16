//! Presentation eligibility, evaluated by SQL before paging. Original actions remain retained.
use anyhow::Result;
use rusqlite::{Connection,functions::FunctionFlags};
use serde_json::Value;

const SNAPSHOT_CHANGES:&[&str]=&["observed.state_changed","observed.configuration_changed"];
const CLOCK_READINGS:&[&str]=&["SecureTimeEstimated","SecureTimeHigh","SecureTimeLow","SecureTimeTickCount","SecureTimeConfidence","LastKnownGoodTime","SpecialPollTimeRemaining","LastGoodSampleInfo"];

pub(crate) fn battery_band(value:&Value)->Option<u8> {
    let n=value.as_f64().or_else(||value.as_str()?.parse::<f64>().ok())?;
    if !n.is_finite()||!(0.0..=100.0).contains(&n){return None}
    Some(if n<=10.0{0}else if n<=20.0{1}else{2})
}

fn clock_reading_only(before:&Value,after:&Value)->bool {
    let (Some(a),Some(b))=(before.as_object(),after.as_object())else{return false};
    let mut changed=false;
    for key in a.keys().chain(b.keys()) {
        let equal=match(a.get(key),b.get(key)){(Some(a),Some(b))=>pcscope_contracts::sensitive::equivalent(a,b),_=>false};
        if equal{continue}changed=true;
        if !CLOCK_READINGS.iter().any(|allowed|key.eq_ignore_ascii_case(allowed)){return false}
    }
    changed
}

pub(crate) fn visible(action:&str,type_id:&str,field:&str,before:Option<&Value>,after:Option<&Value>,routine:bool)->bool {
    // Explicit source events and lifecycle actions never get hidden by a field-name heuristic.
    if !SNAPSHOT_CHANGES.contains(&action)||field.is_empty(){return true}
    let (Some(before),Some(after))=(before,after)else{return true};
    // The old projection compared DPAPI ciphertext. Do not present those retained derived rows
    // as real edits. Decryption is local to equality; no plaintext or digest is returned/stored.
    if pcscope_contracts::sensitive::equivalent(before,after){return false}
    if before.is_null()||after.is_null(){return true}
    if routine{return true}
    match(type_id,field) {
        ("account.user_profile","last_use") |
        ("startup.scheduled_task","next_run"|"last_run") |
        ("network.adapter","speed") => false,
        ("policy.time_sync_configuration","values") => !clock_reading_only(before,after),
        ("hardware.battery","charge") => match(battery_band(before),battery_band(after)){
            (Some(a),Some(b))=>a!=b,_=>true,
        },
        // Module load-address changes are intentionally unchanged at the user's request.
        _=>true,
    }
}

pub(crate) fn install(conn:&Connection)->Result<()> {
    conn.create_scalar_function("pcscope_action_visible",6,FunctionFlags::SQLITE_UTF8|FunctionFlags::SQLITE_DETERMINISTIC,|context|{
        let action:String=context.get(0)?;let type_id:String=context.get(1)?;let field:String=context.get(2)?;
        let before:Option<String>=context.get(3)?;let after:Option<String>=context.get(4)?;let routine:bool=context.get(5)?;
        let parse=|text:Option<String>|text.and_then(|text|serde_json::from_str::<Value>(&text).ok());
        let before=parse(before);let after=parse(after);
        Ok(visible(&action,&type_id,&field,before.as_ref(),after.as_ref(),routine))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;use serde_json::json;
    #[test]
    fn routine_values_and_real_changes_are_not_conflated(){
        let show=|t,f,a,b,r|visible("observed.state_changed",t,f,Some(&a),Some(&b),r);
        for (t,f) in [("account.user_profile","last_use"),("startup.scheduled_task","next_run"),("startup.scheduled_task","last_run"),("network.adapter","speed")]{
            assert!(!show(t,f,json!("old"),json!("new"),false));assert!(show(t,f,json!("old"),json!("new"),true));
        }
        assert!(show("runtime.module_instance","module_base",json!(4096),json!(8192),false));
        for(t,f)in[("startup.service","state"),("startup.scheduled_task","last_result"),("network.adapter","status"),("security.protection","real_time"),("file.watched_file","exists")]{assert!(show(t,f,json!(true),json!(false),false));}
        assert!(visible("startup.task_started","startup.scheduled_task","last_run",Some(&json!("old")),Some(&json!("new")),false));
    }
    #[test]
    fn clock_settings_and_unknown_fields_remain_visible(){
        let a=json!({"SecureTimeTickCount":{"value":1},"Type":{"value":"NTP"}});
        let b=json!({"SecureTimeTickCount":{"value":2},"Type":{"value":"NTP"}});
        assert!(clock_reading_only(&a,&b));
        assert!(!clock_reading_only(&a,&json!({"SecureTimeTickCount":{"value":2},"Type":{"value":"NoSync"}})));
        assert!(!clock_reading_only(&json!({"future_setting":1}),&json!({"future_setting":2})));
        assert!(!clock_reading_only(&Value::Null,&b));
    }
    #[test]
    fn battery_alerts_cross_bands_without_repeating_every_percent(){
        let show=|a,b|visible("observed.state_changed","hardware.battery","charge",Some(&json!(a)),Some(&json!(b)),false);
        for(a,b)in[(50,49),(19,18),(9,8)]{assert!(!show(a,b));}
        for(a,b)in[(21,20),(11,10),(10,11),(20,21)]{assert!(show(a,b));}
        assert!(show(-1,20));assert!(show(101,99));
    }
    fn object(store:&crate::Store,t:&str)->i64 {
        store.reader().execute("INSERT INTO object_instances(type_id,category,machine_id,installation_id,scope,native_key,incarnation,display_name,identity_status,lifecycle,first_observed_at,last_observed_at) VALUES(?,'system','fixture','fixture','machine:fixture',?,1,'示例对象','confirmed','active','2026-10-15T01:00:00Z','2026-10-15T01:00:00Z')",rusqlite::params![t,t]).unwrap();
        store.reader().last_insert_rowid()
    }
    fn action(store:&crate::Store,object:i64,t:&str,field:&str,before:Value,after:Value,at:&str){
        store.reader().execute("INSERT INTO object_actions(instance_id,target_type,action_id,title,result,precision,observed_at,field_id,before_value,after_value,rule_version) VALUES(?,?,'observed.state_changed','变化','observed','observation_only',?,?,?,?,'test')",rusqlite::params![object,t,at,field,before.to_string(),after.to_string()]).unwrap();
    }
    #[test]
    fn sql_filters_before_paging_and_object_history_keeps_routine_updates(){
        use pcscope_contracts::query::{ActionFilter,QueryRequest};
        let store=crate::Store::open(":memory:").unwrap();let service=object(&store,"startup.service");let task=object(&store,"startup.scheduled_task");let module=object(&store,"runtime.module_instance");
        action(&store,service,"startup.service","state",json!("Running"),json!("Stopped"),"2026-10-15T01:00:00Z");
        for n in 0..500{action(&store,task,"startup.scheduled_task","next_run",json!(n),json!(n+1),"2026-10-15T02:00:00Z");}
        // Real field-change rows retain the source ID in target_type. Resolve the bound object
        // type before that fallback, rather than relying on preclassified test rows.
        store.reader().execute("UPDATE object_actions SET target_type='pcscope.startup.task-details' WHERE instance_id=?",[task]).unwrap();
        action(&store,module,"runtime.module_instance","module_base",json!(4096),json!(8192),"2026-10-15T03:00:00Z");
        let first=store.query(&QueryRequest::ActionTimeline(ActionFilter{limit:1,..Default::default()})).unwrap();
        assert_eq!(first.data.as_ref().unwrap()[0]["field_id"],"module_base");assert!(first.page.as_ref().unwrap().has_more);
        let second=store.query(&QueryRequest::ActionTimeline(ActionFilter{limit:1,cursor:first.page.unwrap().next_cursor,..Default::default()})).unwrap();
        assert_eq!(second.data.as_ref().unwrap()[0]["field_id"],"state");assert!(!second.page.unwrap().has_more);
        let all=store.query(&QueryRequest::ActionTimeline(ActionFilter{include_routine:true,limit:200,..Default::default()})).unwrap();assert_eq!(all.data.unwrap().as_array().unwrap().len(),200);
        let history=store.query(&QueryRequest::InstanceActions{instance:task.to_string(),cursor:None,limit:200}).unwrap();assert_eq!(history.data.unwrap().as_array().unwrap().len(),200);
        assert_eq!(store.reader().query_row("SELECT count(*) FROM object_actions",[],|r|r.get::<_,i64>(0)).unwrap(),502);
    }
    #[test]
    fn security_update_summary_respects_the_walk_watermark(){
        use pcscope_contracts::query::{ActionFilter,QueryRequest};
        let store=crate::Store::open(":memory:").unwrap();let protection=object(&store,"security.protection");
        action(&store,protection,"security.protection","engine_version",json!("1"),json!("2"),"2026-10-15T01:00:00Z");
        action(&store,protection,"security.protection","real_time",json!(true),json!(false),"2026-10-15T02:00:00Z");
        let first=store.query(&QueryRequest::ActionTimeline(ActionFilter{limit:1,..Default::default()})).unwrap();
        action(&store,protection,"security.protection","signature_version",json!("1"),json!("2"),"2026-10-15T01:00:00Z");
        let continued=store.query(&QueryRequest::ActionTimeline(ActionFilter{limit:1,cursor:first.page.unwrap().next_cursor,..Default::default()})).unwrap();
        assert_eq!(continued.data.unwrap()[0]["field_id"],"engine_version","a later sibling cannot remove a row from an existing walk");
        let current=store.query(&QueryRequest::ActionTimeline(ActionFilter::default())).unwrap();let data=current.data.unwrap();
        assert_eq!(data.as_array().unwrap().len(),2);assert_eq!(data[1]["field_id"],"signature_version");assert_eq!(data[1]["source_record"]["protection_update"]["engine_changed"],true);
        let full=store.query(&QueryRequest::ActionTimeline(ActionFilter{include_routine:true,..Default::default()})).unwrap();assert_eq!(full.data.unwrap().as_array().unwrap().len(),3);
    }
    #[cfg(windows)]
    #[test]
    fn retained_ciphertext_only_changes_are_not_presented_as_real_edits(){
        use pcscope_contracts::{query::{ActionFilter,QueryRequest},sensitive::protect};
        let store=crate::Store::open(":memory:").unwrap();let id=object(&store,"startup.logon_initialization");
        let a=protect(&json!(14)).unwrap();let b=protect(&json!(14)).unwrap();assert_ne!(a,b);
        action(&store,id,"startup.logon_initialization","values",json!({"setting":a}),json!({"setting":b}),"2026-10-15T01:00:00Z");
        for routine in [false,true]{assert!(store.query(&QueryRequest::ActionTimeline(ActionFilter{include_routine:routine,..Default::default()})).unwrap().data.unwrap().as_array().unwrap().is_empty());}
        assert!(store.query(&QueryRequest::InstanceActions{instance:id.to_string(),cursor:None,limit:20}).unwrap().data.unwrap().as_array().unwrap().is_empty());
        assert_eq!(store.reader().query_row("SELECT count(*) FROM object_actions",[],|r|r.get::<_,i64>(0)).unwrap(),1,"the historical material is preserved");
        action(&store,id,"startup.logon_initialization","values",json!({"setting":protect(&json!(14)).unwrap()}),json!({"setting":protect(&json!(15)).unwrap()}),"2026-10-15T02:00:00Z");
        assert_eq!(store.query(&QueryRequest::ActionTimeline(ActionFilter::default())).unwrap().data.unwrap().as_array().unwrap().len(),1);
    }
    #[cfg(windows)]
    #[test]
    fn real_snapshot_projection_compares_protected_values_not_random_ciphertext(){
        use pcscope_contracts::sensitive::protect;
        let mut store=crate::Store::open(":memory:").unwrap();
        for(round,secret,shell)in[(1,14,"explorer.exe"),(2,14,"explorer.exe"),(3,14,"fixture.exe"),(4,15,"fixture.exe")]{
            let input=pcscope_contracts::parse(&json!({"protocol_version":1,"request_id":format!("fixture-{round}"),"type":"snapshot","source":"pcscope.startup.session-hooks","scope":"machine:fixture/user:fixture/schema:1/context:/shard:0","run_id":format!("fixture-{round}"),"observed_at":format!("2026-10-15T0{round}:00:00Z"),"complete":true,"items":[{"id":"fixture-winlogon","data":{"hive":"LocalMachine","view":"Registry64","path":"SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon","values":{"PasswordExpiryWarning":{"kind":"DWord","value":protect(&json!(secret)).unwrap()},"Shell":{"kind":"String","value":shell}}}}]}).to_string()).unwrap();
            store.ingest(&input).unwrap();
            while store.project_pending("fixture","fixture",64).unwrap().examined>0{}
            let changes=store.reader().query_row("SELECT count(*) FROM object_actions WHERE field_id='values'",[],|r|r.get::<_,i64>(0)).unwrap();
            assert_eq!(changes,if round<=2{0}else{round-2},"round {round}");
        }
    }
}
