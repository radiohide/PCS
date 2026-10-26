//! Explicit history replacement after the daemon owns the endpoint and all old workers exited.
//! Only configuration is copied. No scan, DELETE of large tables, or VACUUM of the old history.
use crate::*;
const CONFIG: &[&str]=&["plugin_schedule","item_settings","file_watch","directory_watch","runtime_watch","monitoring","show_sensitive","retention_mode","quota_bytes","max_pending_projections"];

impl Store {
    pub fn history_id(&self)->Result<String>{self.setting("history_id")}
}

/// `operation` is persisted before shutdown. Replaying it after a crash cannot erase new data.
pub fn replace_history(directory:&Path,operation:&str,expected_history:&str)->Result<()> {
    anyhow::ensure!(operation.len()==40&&operation.bytes().all(|c|c.is_ascii_hexdigit()),"无效的删除操作标识");
    let live=directory.join("history.db");
    let old=Store::open_program(&live)?;
    let applied:Option<String>=old.conn.query_row("SELECT value FROM settings WHERE key='history_reset_operation'",[],|r|r.get(0)).optional()?;
    if applied.as_deref()==Some(operation){return Ok(())}
    anyhow::ensure!(old.history_id()?==expected_history,"历史数据已变化，请重新确认删除");
    let values:Vec<(String,String)>=CONFIG.iter().filter_map(|key|match old.conn.query_row("SELECT value FROM settings WHERE key=?",[key],|r|r.get::<_,String>(0)).optional(){Ok(Some(v))=>Some(Ok((key.to_string(),v))),Ok(None)=>None,Err(e)=>Some(Err(e))}).collect::<rusqlite::Result<_>>()?;
    let staging=directory.join(format!("history-reset-{operation}"));
    // A prior partial preparation is deliberately not trusted as a completed empty database.
    // Only the fixed files in this operation's own directory may be removed; never recurse.
    if staging.exists(){
        let metadata=std::fs::symlink_metadata(&staging)?;
        anyhow::ensure!(metadata.is_dir()&&!metadata.file_type().is_symlink(),"临时数据路径异常，未删除历史");
        #[cfg(windows)] {use std::os::windows::fs::MetadataExt;anyhow::ensure!(metadata.file_attributes()&0x400==0,"临时数据路径不能是重解析目标");}
        for name in ["history.db","history.db-wal","history.db-shm"] {let path=staging.join(name);if path.exists(){let meta=std::fs::symlink_metadata(&path)?;anyhow::ensure!(meta.is_file()&&!meta.file_type().is_symlink(),"临时文件路径异常");std::fs::remove_file(path)?;}}
        std::fs::remove_dir(&staging)?;
    }
    std::fs::create_dir(&staging)?;
    let fresh=staging.join("history.db");
    let mut new=Store::open_program(&fresh)?;
    let tx=new.conn.transaction()?;
    for (key,value) in values {tx.execute("INSERT INTO settings VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value])?;}
    let started:String=tx.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')",[],|r|r.get(0))?;
    tx.execute("INSERT INTO settings VALUES('collection_baseline',?)",[json!({"version":1,"started_at":started,"event_history":"from_baseline"}).to_string()])?;
    tx.execute("INSERT INTO settings VALUES('history_reset_operation',?)",[operation])?;tx.commit()?;
    // This also takes an exclusive SQLite lock. An outstanding reader/writer makes replacement
    // fail safely before the live directory entry is touched.
    for store in [&new,&old] {
        let mode:String=store.conn.query_row("PRAGMA journal_mode=DELETE",[],|r|r.get(0))?;
        anyhow::ensure!(mode.eq_ignore_ascii_case("delete"),"历史文件仍在使用，请稍后重试");
    }
    drop(new);drop(old);
    anyhow::ensure!(!directory.join("history.db-wal").exists()&&!directory.join("history.db-shm").exists(),"历史文件尚未关闭，未删除历史");
    std::fs::OpenOptions::new().write(true).open(&fresh)?.sync_all()?;
    #[cfg(windows)] {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{MoveFileExW,MOVEFILE_REPLACE_EXISTING,MOVEFILE_WRITE_THROUGH};
        let wide=|p:&Path|p.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
        if unsafe{MoveFileExW(wide(&fresh).as_ptr(),wide(&live).as_ptr(),MOVEFILE_REPLACE_EXISTING|MOVEFILE_WRITE_THROUGH)}==0{return Err(std::io::Error::last_os_error().into());}
    }
    #[cfg(not(windows))] std::fs::rename(&fresh,&live)?;
    // Replacement is the commit point: a leftover empty staging folder is not a deletion failure.
    let _=std::fs::remove_dir(staging);Ok(())
}

#[cfg(test)] mod tests {
    use super::*;
    fn fixture()->PathBuf{let p=std::env::temp_dir().join(format!("pcscope-reset-{}-{}",std::process::id(),chrono::Utc::now().timestamp_nanos_opt().unwrap()));std::fs::create_dir(&p).unwrap();p}
    #[test] fn reset_releases_history_preserves_settings_and_replay_is_safe(){
        let dir=fixture();let path=dir.join("history.db");let old=Store::open_program(&path).unwrap();let id=old.history_id().unwrap();
        for key in CONFIG {old.conn.execute("INSERT OR REPLACE INTO settings VALUES(?,?)",params![key,match *key{"retention_mode"=>"unlimited","quota_bytes"=>"5000000000","max_pending_projections"=>"200000",_=>"{}"}]).unwrap();}
        old.conn.execute_batch("CREATE TABLE deletion_fixture(data BLOB); INSERT INTO deletion_fixture VALUES(zeroblob(4000000)); INSERT INTO settings VALUES('runtime_watch_state','old-cache'); INSERT INTO checkpoints VALUES('source','epoch','old-cursor');").unwrap();drop(old);
        let size=std::fs::metadata(&path).unwrap().len();let op="0123456789012345678901234567890123456789";
        replace_history(&dir,op,&id).unwrap();let new=Store::open_program(&path).unwrap();assert_ne!(new.history_id().unwrap(),id);
        assert_eq!(new.conn.query_row("SELECT count(*) FROM checkpoints",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        assert!(!new.conn.query_row("SELECT EXISTS(SELECT 1 FROM settings WHERE key='runtime_watch_state')",[],|r|r.get::<_,bool>(0)).unwrap());
        for key in CONFIG {assert!(new.setting(key).is_ok());}
        assert!(new.collection_started_at().unwrap().is_some());
        new.conn.execute("INSERT INTO settings VALUES('after_reset','keep')",[]).unwrap();drop(new);
        assert!(std::fs::metadata(&path).unwrap().len()<size/2);replace_history(&dir,op,&id).unwrap();
        let new=Store::open_program(&path).unwrap();assert_eq!(new.setting("after_reset").unwrap(),"keep");drop(new);
        assert!(replace_history(&dir,"abcdef0123456789012345678901234567890123",&id).is_err());
        assert!(replace_history(&dir,"../bad",&id).is_err());std::fs::remove_dir_all(dir).unwrap();
    }
    #[test] fn active_reader_prevents_replacement_and_retry_recovers_preparation(){
        let dir=fixture();let path=dir.join("history.db");let old=Store::open_program(&path).unwrap();let id=old.history_id().unwrap();
        old.conn.execute("INSERT INTO settings VALUES('before_reset','keep')",[]).unwrap();
        let reader=Connection::open(&path).unwrap();reader.execute_batch("BEGIN; SELECT * FROM settings;").unwrap();
        let op="abcdef0123456789012345678901234567890123";assert!(replace_history(&dir,op,&id).is_err());
        assert_eq!(old.setting("before_reset").unwrap(),"keep");drop(reader);drop(old);
        replace_history(&dir,op,&id).unwrap();assert!(!dir.join(format!("history-reset-{op}")).exists());std::fs::remove_dir_all(dir).unwrap();
    }
}
