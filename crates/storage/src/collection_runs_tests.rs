use super::*;
use serde_json::json;
fn setup()->(Connection,Execution){let conn=Connection::open_in_memory().unwrap();ensure_schema(&conn).unwrap();let execution=begin_execution(&conn).unwrap();configure(&conn,&execution,"source","scope","cfg-1").unwrap();(conn,execution)}
fn material(ids:&[&str],at:&str)->(Manifest,Vec<Vec<u8>>){manifest_for(&ids.iter().map(|id|vec![Item{id:(*id).into(),data:json!({"enabled":false,"count":0})}]).collect::<Vec<_>>(),at).unwrap()}
fn complete(conn:&Connection,execution:&Execution,ids:&[&str],at:&str)->Ticket{let (manifest,parts)=material(ids,at);let ticket=allocate(conn,execution,"source","scope","cfg-1",&manifest).unwrap();for(part,body)in parts.iter().enumerate(){receive_part(conn,&ticket,part,body).unwrap();}assert_eq!(seal(conn,&ticket).unwrap(),Receipt::Received);ticket}
fn ids(conn:&Connection)->Option<Vec<String>>{received_items(conn,"source","scope").unwrap().map(|items|items.into_iter().map(|item|item.id).collect())}

#[test]
fn incomplete_parts_and_an_absent_end_marker_leave_the_previous_complete_head(){
    let(conn,execution)=setup();complete(&conn,&execution,&["old"],"2026-10-19T01:00:00Z");
    let (manifest,parts)=material(&["new-1","new-2"],"2026-10-19T01:01:00Z");let ticket=allocate(&conn,&execution,"source","scope","cfg-1",&manifest).unwrap();
    receive_part(&conn,&ticket,0,&parts[0]).unwrap();assert_eq!(ids(&conn).unwrap(),["old"]);
    receive_part(&conn,&ticket,1,&parts[1]).unwrap();assert_eq!(ids(&conn).unwrap(),["old"]);
    assert_eq!(seal(&conn,&ticket).unwrap(),Receipt::Received);assert_eq!(ids(&conn).unwrap(),["new-1","new-2"]);
    assert_eq!(seal(&conn,&ticket).unwrap(),Receipt::Duplicate);
}
#[test]
fn an_early_end_marker_can_wait_for_out_of_order_parts(){
    let(conn,execution)=setup();let(manifest,parts)=material(&["one","two"],"2026-10-19T01:00:00Z");let ticket=allocate(&conn,&execution,"source","scope","cfg-1",&manifest).unwrap();
    assert_eq!(seal(&conn,&ticket).unwrap(),Receipt::Stored);
    receive_part(&conn,&ticket,1,&parts[1]).unwrap();assert!(ids(&conn).is_none());
    assert_eq!(receive_part(&conn,&ticket,0,&parts[0]).unwrap(),Receipt::Received);assert_eq!(ids(&conn).unwrap(),["one","two"]);
    let bytes:Vec<u8>=conn.query_row("SELECT original FROM collection_parts WHERE run_id=? AND part=0",[ticket.id],|r|r.get(0)).unwrap();assert_eq!(bytes,parts[0]);
}
#[test]
fn duplicate_parts_are_idempotent_and_conflicts_cannot_revoke_a_receipt(){
    let(conn,execution)=setup();let ticket=complete(&conn,&execution,&["one"],"2026-10-19T01:00:00Z");
    let(_,parts)=material(&["one"],"2026-10-19T01:00:00Z");assert_eq!(receive_part(&conn,&ticket,0,&parts[0]).unwrap(),Receipt::Duplicate);
    assert!(receive_part(&conn,&ticket,0,b"[]").is_err());assert_eq!(ids(&conn).unwrap(),["one"]);
    assert_eq!(conn.query_row("SELECT count(*) FROM collection_parts",[],|r|r.get::<_,i64>(0)).unwrap(),1);
}
#[test]
fn conflicting_staged_parts_and_cross_part_identities_reject_the_run(){
    for duplicate in [false,true]{let(conn,execution)=setup();complete(&conn,&execution,&["old"],"2026-10-19T01:00:00Z");
        let(manifest,parts)=material(&["new","new"],"2026-10-19T01:01:00Z");let ticket=allocate(&conn,&execution,"source","scope","cfg-1",&manifest).unwrap();
        receive_part(&conn,&ticket,0,&parts[0]).unwrap();
        assert!(if duplicate{receive_part(&conn,&ticket,1,&parts[1])}else{receive_part(&conn,&ticket,0,b"[]")}.is_err());
        assert_eq!(seal(&conn,&ticket).unwrap(),Receipt::Cancelled);assert_eq!(ids(&conn).unwrap(),["old"]);
    }
}
#[test]
fn digest_or_total_mismatch_cannot_replace_current_material(){
    let(conn,execution)=setup();complete(&conn,&execution,&["old"],"2026-10-19T01:00:00Z");
    let(mut manifest,parts)=material(&["new"],"2026-10-19T01:01:00Z");manifest.digest="0".repeat(64);
    let ticket=allocate(&conn,&execution,"source","scope","cfg-1",&manifest).unwrap();receive_part(&conn,&ticket,0,&parts[0]).unwrap();assert_eq!(seal(&conn,&ticket).unwrap(),Receipt::Cancelled);
    assert_eq!(ids(&conn).unwrap(),["old"]);
}
#[test]
fn a_late_old_run_is_history_even_when_the_clock_moves_backwards(){
    let(conn,execution)=setup();let(manifest,parts)=material(&["late"],"2026-10-19T01:00:00Z");let old=allocate(&conn,&execution,"source","scope","cfg-1",&manifest).unwrap();
    let newer=complete(&conn,&execution,&["current"],"2020-01-01T00:00:00Z");assert!(newer.seq>old.seq);
    receive_part(&conn,&old,0,&parts[0]).unwrap();assert_eq!(seal(&conn,&old).unwrap(),Receipt::Historical);assert_eq!(ids(&conn).unwrap(),["current"]);
}
#[test]
fn replacing_execution_rejects_old_tickets_without_erasing_complete_material(){
    let(conn,old)=setup();complete(&conn,&old,&["retained"],"2026-10-19T01:00:00Z");let(manifest,parts)=material(&["late"],"2026-10-19T01:01:00Z");let stale=allocate(&conn,&old,"source","scope","cfg-1",&manifest).unwrap();
    let new=begin_execution(&conn).unwrap();assert_ne!(new.epoch,old.epoch);assert_eq!(receive_part(&conn,&stale,0,&parts[0]).unwrap(),Receipt::Cancelled);
    assert!(allocate(&conn,&old,"source","scope","cfg-1",&manifest).is_err());assert_eq!(ids(&conn).unwrap(),["retained"]);
    let current=complete(&conn,&new,&["new"],"2026-10-19T01:02:00Z");assert!(current.seq>stale.seq);
}
#[test]
fn configuration_changes_establish_an_independent_baseline_and_do_not_reuse_old_revisions(){
    let(conn,execution)=setup();complete(&conn,&execution,&["old"],"2026-10-19T01:00:00Z");let(manifest,parts)=material(&["stale"],"2026-10-19T01:01:00Z");let stale=allocate(&conn,&execution,"source","scope","cfg-1",&manifest).unwrap();
    configure(&conn,&execution,"source","scope","cfg-2").unwrap();assert!(ids(&conn).is_none());assert_eq!(receive_part(&conn,&stale,0,&parts[0]).unwrap(),Receipt::Cancelled);
    assert!(configure(&conn,&execution,"source","scope","cfg-1").is_err());
    assert_eq!(conn.query_row("SELECT count(*) FROM collection_parts WHERE run_id=1",[],|r|r.get::<_,i64>(0)).unwrap(),1);
}
#[test]
fn sequence_exhaustion_is_an_error_instead_of_wrapping(){
    let(conn,execution)=setup();complete(&conn,&execution,&["old"],"2026-10-19T01:00:00Z");conn.execute("UPDATE collection_scope_heads SET issued=?",[i64::MAX]).unwrap();
    let(manifest,_)=material(&["new"],"2026-10-19T01:01:00Z");assert!(allocate(&conn,&execution,"source","scope","cfg-1",&manifest).is_err());assert_eq!(ids(&conn).unwrap(),["old"]);
}
#[test]
fn shape_and_budget_errors_never_turn_into_an_empty_complete_result(){
    let(conn,execution)=setup();complete(&conn,&execution,&["old"],"2026-10-19T01:00:00Z");let(mut manifest,_)=material(&["new"],"2026-10-19T01:01:00Z");manifest.observed_at="not a timestamp".into();assert!(allocate(&conn,&execution,"source","scope","cfg-1",&manifest).is_err());
    manifest.observed_at="2026-10-19T01:01:00Z".into();let ticket=allocate(&conn,&execution,"source","scope","cfg-1",&manifest).unwrap();assert!(receive_part(&conn,&ticket,0,b"false").is_err());assert!(receive_part(&conn,&ticket,64,b"[]").is_err());assert_eq!(ids(&conn).unwrap(),["old"]);
}

#[test]
fn failed_part_writes_roll_back_and_can_retry_original_bytes(){
    let(conn,execution)=setup();complete(&conn,&execution,&["old"],"2026-10-19T01:00:00Z");
    let(manifest,parts)=material(&["one","two"],"2026-10-19T01:01:00Z");let ticket=allocate(&conn,&execution,"source","scope","cfg-1",&manifest).unwrap();
    receive_part(&conn,&ticket,0,&parts[0]).unwrap();seal(&conn,&ticket).unwrap();
    conn.execute_batch("CREATE TEMP TRIGGER injected_failure BEFORE INSERT ON collection_parts WHEN NEW.part=1 BEGIN SELECT RAISE(ABORT,'injected write failure'); END;").unwrap();
    assert!(receive_part(&conn,&ticket,1,&parts[1]).is_err());assert_eq!(ids(&conn).unwrap(),["old"]);
    assert_eq!(conn.query_row("SELECT count(*) FROM collection_parts WHERE run_id=?",[ticket.id],|r|r.get::<_,i64>(0)).unwrap(),1);
    conn.execute_batch("DROP TRIGGER injected_failure;").unwrap();
    assert_eq!(receive_part(&conn,&ticket,1,&parts[1]).unwrap(),Receipt::Received);assert_eq!(ids(&conn).unwrap(),["one","two"]);
}
#[test]
fn another_source_can_complete_while_one_source_is_incomplete(){
    let(conn,execution)=setup();let(manifest,parts)=material(&["one"],"2026-10-19T01:00:00Z");
    let incomplete=allocate(&conn,&execution,"source","scope","cfg-1",&manifest).unwrap();receive_part(&conn,&incomplete,0,&parts[0]).unwrap();
    configure(&conn,&execution,"another","scope","cfg-1").unwrap();let healthy=allocate(&conn,&execution,"another","scope","cfg-1",&manifest).unwrap();
    receive_part(&conn,&healthy,0,&parts[0]).unwrap();assert_eq!(seal(&conn,&healthy).unwrap(),Receipt::Received);
    assert!(ids(&conn).is_none());let items=received_items(&conn,"another","scope").unwrap().unwrap();assert_eq!(items.len(),1);assert_eq!(items[0].data["enabled"],false);assert_eq!(items[0].data["count"],0);
}
#[test]
fn receiver_reopening_with_the_same_trusted_host_can_resume_saved_parts(){
    let file=std::env::temp_dir().join(format!("pcscope-run-prototype-{}-{}.db",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let conn=Connection::open(&file).unwrap();ensure_schema(&conn).unwrap();let execution=begin_execution(&conn).unwrap();configure(&conn,&execution,"source","scope","cfg-1").unwrap();
    let(manifest,parts)=material(&["one","two"],"2026-10-19T01:00:00Z");let ticket=allocate(&conn,&execution,"source","scope","cfg-1",&manifest).unwrap();receive_part(&conn,&ticket,0,&parts[0]).unwrap();drop(conn);
    let reopened=Connection::open(&file).unwrap();ensure_schema(&reopened).unwrap();assert!(ids(&reopened).is_none());
    receive_part(&reopened,&ticket,1,&parts[1]).unwrap();assert_eq!(seal(&reopened,&ticket).unwrap(),Receipt::Received);assert_eq!(ids(&reopened).unwrap(),["one","two"]);
    drop(reopened);std::fs::remove_file(file).unwrap();
}
