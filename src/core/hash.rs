use crate::core::Result;
use camino::{Utf8Path, Utf8PathBuf};
use sha2::{Digest, Sha256};
use std::fs;
use walkdir::WalkDir;

pub const DEFAULT_HASH_SUFFIX_LEN: usize = 8;

pub fn hash_skill_dir(skill_dir: &Utf8Path) -> Result<String> {
    let mut entries = Vec::new();

    for entry in WalkDir::new(skill_dir).follow_links(false) {
        let entry = entry.map_err(|source| std::io::Error::other(source.to_string()))?;
        if entry.file_type().is_dir() {
            continue;
        }

        let path = Utf8PathBuf::from_path_buf(entry.path().to_path_buf()).map_err(|path| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("path is not UTF-8: {}", path.display()),
            )
        })?;
        let rel_path = path.strip_prefix(skill_dir).unwrap_or(&path).to_path_buf();
        if should_ignore_path(&rel_path) {
            continue;
        }

        entries.push(rel_path);
    }

    entries.sort();

    let mut hasher = Sha256::new();
    for rel_path in entries {
        let full_path = skill_dir.join(&rel_path);
        hasher.update(rel_path.as_str().as_bytes());
        hasher.update([0]);
        let bytes = fs::read(&full_path)?;
        hasher.update(bytes.len().to_le_bytes());
        hasher.update([0]);
        hasher.update(&bytes);
    }

    Ok(hex::encode(hasher.finalize()))
}

fn should_ignore_path(path: &Utf8Path) -> bool {
    let file_name = path.file_name().unwrap_or_default();
    if file_name == ".DS_Store" {
        return true;
    }
    if file_name.starts_with('.') && file_name.ends_with(".tmp") {
        return true;
    }
    if file_name.starts_with(".#") || file_name.ends_with('~') {
        return true;
    }
    if file_name.ends_with(".swp") || file_name.ends_with(".swo") {
        return true;
    }
    file_name.starts_with(".skill-kits")
}
