use crate::core::Result;
use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn ensure_dir(path: &Utf8Path) -> Result<()> {
    fs::create_dir_all(path)?;
    Ok(())
}

pub fn safe_read_to_string(path: &Utf8Path) -> Result<String> {
    Ok(fs::read_to_string(path)?)
}

pub fn copy_dir_clean_source_to_empty_target(source: &Utf8Path, target: &Utf8Path) -> Result<()> {
    if target.exists() {
        return Err(crate::core::SkillKitsError::DeployConflict {
            target: target.to_path_buf(),
        });
    }

    fs::create_dir_all(target)?;
    let copy_result = copy_dir_contents(source, target);
    if copy_result.is_err() {
        let _ = fs::remove_dir_all(target);
    }
    copy_result
}

fn copy_dir_contents(source: &Utf8Path, target: &Utf8Path) -> Result<()> {
    for entry in walkdir::WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(|source| std::io::Error::other(source.to_string()))?;
        let path = Utf8PathBuf::from_path_buf(entry.path().to_path_buf()).map_err(|path| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("path is not UTF-8: {}", path.display()),
            )
        })?;
        let rel = path.strip_prefix(source).unwrap_or(&path);
        if rel.as_str().is_empty() {
            continue;
        }
        let destination = target.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&destination)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), destination.as_std_path())?;
        }
    }
    Ok(())
}

pub fn atomic_write_toml<T>(path: &Utf8Path, value: &T) -> Result<()>
where
    T: Serialize,
{
    let bytes = toml::to_string_pretty(value)?;
    atomic_write_string(path, &bytes)
}

pub fn atomic_write_string(path: &Utf8Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }

    let temp_path = temp_path_for(path);
    let write_result = (|| -> Result<()> {
        let mut temp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        temp_file.write_all(contents.as_bytes())?;
        temp_file.sync_all()?;
        drop(temp_file);
        fs::rename(&temp_path, path)?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result
}

fn temp_path_for(path: &Utf8Path) -> camino::Utf8PathBuf {
    let file_name = path.file_name().unwrap_or("state.toml");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    path.with_file_name(format!(".{file_name}.{}.{}.tmp", std::process::id(), nanos))
}
