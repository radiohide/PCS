use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Prepare disk-backed SQLite work space inside this instance's selected data directory.
/// Do not change ACLs or redirect the database to another instance when access fails.
pub fn prepare_temp_directory(data: &Path) -> Result<PathBuf> {
    let root=std::fs::canonicalize(data)?;
    let temp=data.join("sqlite-temp");
    std::fs::create_dir_all(&temp).context("无法创建数据库临时目录；本次未启动采集")?;
    let metadata=std::fs::symlink_metadata(&temp)?;
    anyhow::ensure!(metadata.is_dir()&&!metadata.file_type().is_symlink(),"数据库临时目录必须是普通目录");
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        anyhow::ensure!(metadata.file_attributes()&0x400==0,"数据库临时目录不能是重解析目标");
    }
    let resolved=std::fs::canonicalize(&temp)?;
    anyhow::ensure!(resolved.parent()==Some(root.as_path()),"数据库临时目录不在所选数据目录内");
    let nonce=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
    let probe=temp.join(format!("write-check-{}-{nonce}.tmp",std::process::id()));
    let file=std::fs::OpenOptions::new().write(true).create_new(true).open(&probe)
        .context("数据库临时目录无法写入；本次未启动采集")?;
    drop(file);
    std::fs::remove_file(&probe)?;
    Ok(temp)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn temp_workspace_is_separate_per_instance_and_leaves_no_probe() {
        let nonce=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("pcscope-runtime-temp-{}-{nonce}",std::process::id()));
        let result=(||->Result<()> {
            std::fs::create_dir_all(root.join("甲"))?;std::fs::create_dir_all(root.join("乙"))?;
            let a=prepare_temp_directory(&root.join("甲"))?;
            let b=prepare_temp_directory(&root.join("乙"))?;
            assert_ne!(a,b);assert_eq!(std::fs::read_dir(&a)?.count(),0);
            assert_eq!(prepare_temp_directory(&root.join("甲"))?,a);
            std::fs::remove_dir(&a)?;std::fs::write(&a,b"not a directory")?;
            assert!(prepare_temp_directory(&root.join("甲")).is_err());
            Ok(())
        })();
        std::fs::remove_dir_all(root).unwrap();result.unwrap();
    }
}
