//! A bounded receipt for hidden launches: startup failures must survive process exit.
use std::{io::Write, path::{Path,PathBuf}, time::Instant};
use serde_json::{json,Value};

pub struct Report {
    directory:PathBuf,
    started:Instant,
    state:Value,
}
impl Report {
    pub fn new(directory:&Path)->Self {
        Self{directory:directory.into(),started:Instant::now(),state:json!({
            "version":env!("CARGO_PKG_VERSION"),"pid":std::process::id(),
            "started_at":chrono::Utc::now().to_rfc3339(),"status":"starting","phase":"initializing"})}
    }
    pub fn phase(&mut self,phase:&str) {
        self.state["phase"]=json!(phase);
        if phase=="ready"{self.state["status"]=json!("ready");}
        self.persist();
    }
    pub fn finish(&mut self,result:&anyhow::Result<()>) {
        match result {
            Ok(())=>self.state["status"]=json!("stopped"),
            Err(error)=>{
                self.state["status"]=json!("failed");
                self.state["error"]=json!(format!("{error:#}").chars().take(4096).collect::<String>());
            }
        }
        self.persist();
    }
    fn persist(&mut self) {
        self.state["updated_at"]=json!(chrono::Utc::now().to_rfc3339());
        self.state["elapsed_ms"]=json!(self.started.elapsed().as_millis() as u64);
        if let Err(error)=write_receipt(&self.directory,&self.state) {
            // Diagnostics must never replace the actual startup result or stop collection.
            eprintln!("cannot save service startup receipt: {error:#}");
        }
    }
}
fn write_receipt(directory:&Path,state:&Value)->anyhow::Result<()> {
    let nonce=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
    let temporary=directory.join(format!(".service-startup-{}-{nonce}.tmp",std::process::id()));
    let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
    let result=(||->anyhow::Result<()> {
        file.write_all(&serde_json::to_vec(state)?)?;
        file.sync_all()?;drop(file);
        // Replace the directory entry, rather than opening/following a pre-existing receipt.
        std::fs::rename(&temporary,directory.join("service-startup.json"))?;Ok(())
    })();
    if result.is_err(){let _=std::fs::remove_file(temporary);}
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn phases_and_error_survive_exit_without_growing_logs() {
        let root=std::env::temp_dir().join(format!("pcscope-startup-report-{}-{}",std::process::id(),chrono::Utc::now().timestamp_nanos_opt().unwrap()));
        std::fs::create_dir(&root).unwrap();
        let mut report=Report::new(&root);
        for phase in ["opening_history","recording_session","ready"]{report.phase(phase);}
        report.finish(&Err(anyhow::anyhow!("{}","失败".repeat(5000)).context("startup failed")));
        let bytes=std::fs::read(root.join("service-startup.json")).unwrap();
        let value:Value=serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["status"],"failed");assert_eq!(value["phase"],"ready");
        assert!(value["error"].as_str().unwrap().starts_with("startup failed: "));
        assert_eq!(value["error"].as_str().unwrap().chars().count(),4096);
        assert!(bytes.len()<32768);assert_eq!(std::fs::read_dir(&root).unwrap().count(),1);
        let mut next=Report::new(&root);next.phase("ready");next.finish(&Ok(()));
        let value:Value=serde_json::from_slice(&std::fs::read(root.join("service-startup.json")).unwrap()).unwrap();
        assert_eq!(value["status"],"stopped");assert!(value.get("error").is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unavailable_diagnostics_do_not_change_the_startup_error() {
        let path=std::env::temp_dir().join(format!("pcscope-report-not-a-directory-{}-{}",std::process::id(),chrono::Utc::now().timestamp_nanos_opt().unwrap()));
        std::fs::write(&path,b"ordinary file").unwrap();
        let mut report=Report::new(&path);
        let result=Err(anyhow::anyhow!("original database error"));report.finish(&result);
        assert_eq!(result.unwrap_err().to_string(),"original database error");
        assert_eq!(std::fs::read(&path).unwrap(),b"ordinary file");std::fs::remove_file(path).unwrap();
    }
}
