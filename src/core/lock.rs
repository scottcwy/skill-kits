use crate::core::error::{Result, SkillKitsError};
use crate::core::paths::AppPaths;
use std::fs::{self, OpenOptions};
use std::io::Write;

#[derive(Debug)]
pub struct StateLock {
    path: camino::Utf8PathBuf,
}

impl StateLock {
    pub fn acquire(paths: &AppPaths) -> Result<Self> {
        fs::create_dir_all(&paths.locks_dir)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&paths.state_lock)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    SkillKitsError::RegistryBusy
                } else {
                    error.into()
                }
            })?;
        writeln!(file, "pid={}", std::process::id())?;
        file.sync_all()?;
        Ok(Self {
            path: paths.state_lock.clone(),
        })
    }
}

impl Drop for StateLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
