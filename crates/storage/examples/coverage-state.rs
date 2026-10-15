use anyhow::{Context,Result};
use rusqlite::{Connection,OpenFlags};
fn main()->Result<()> {
    let path=std::env::args().nth(1).context("coverage-state <database> <source>")?;
    let source=std::env::args().nth(2).context("source required")?;
    let conn=Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut q=conn.prepare("SELECT reason,first_seen,last_seen,observations,active FROM coverage_state WHERE source=?")?;
    for row in q.query_map([&source],|r|Ok(serde_json::json!({"reason":r.get::<_,String>(0)?,"first_seen":r.get::<_,String>(1)?,"last_seen":r.get::<_,String>(2)?,"observations":r.get::<_,i64>(3)?,"active":r.get::<_,bool>(4)?})))? {println!("{}",row?);}
    let seq:Option<i64>=conn.query_row("SELECT max(seq) FROM records WHERE source=? AND kind='coverage.gap'",[source],|r|r.get(0))?;
    println!("Latest gap record: {seq:?}");
    Ok(())
}
