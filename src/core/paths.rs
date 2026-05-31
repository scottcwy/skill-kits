use camino::Utf8PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPaths {
    pub data_root: Utf8PathBuf,
    pub config_file: Utf8PathBuf,
    pub registry_dir: Utf8PathBuf,
    pub skills_registry_file: Utf8PathBuf,
    pub deployments_registry_file: Utf8PathBuf,
    pub skills_dir: Utf8PathBuf,
    pub cache_dir: Utf8PathBuf,
    pub locks_dir: Utf8PathBuf,
    pub state_lock: Utf8PathBuf,
}

impl AppPaths {
    pub fn from_data_root(data_root: impl Into<Utf8PathBuf>) -> Self {
        let data_root = data_root.into();
        let registry_dir = data_root.join("registry");
        let locks_dir = data_root.join("locks");
        Self {
            config_file: data_root.join("config.toml"),
            skills_registry_file: registry_dir.join("skills.toml"),
            deployments_registry_file: registry_dir.join("deployments.toml"),
            skills_dir: data_root.join("skills"),
            cache_dir: data_root.join("cache"),
            state_lock: locks_dir.join("state.lock"),
            registry_dir,
            locks_dir,
            data_root,
        }
    }

    pub fn default_user_paths() -> crate::core::Result<Self> {
        let home = dirs::home_dir()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "home directory"))?;
        let home = Utf8PathBuf::from_path_buf(home).map_err(|path| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("home path is not UTF-8: {}", path.display()),
            )
        })?;
        Ok(Self::from_data_root(home.join(".skill-kits")))
    }
}

pub fn ensure_app_dirs(paths: &AppPaths) -> crate::core::Result<()> {
    crate::core::fs::ensure_dir(&paths.data_root)?;
    crate::core::fs::ensure_dir(&paths.registry_dir)?;
    crate::core::fs::ensure_dir(&paths.skills_dir)?;
    crate::core::fs::ensure_dir(&paths.cache_dir)?;
    crate::core::fs::ensure_dir(&paths.locks_dir)?;
    Ok(())
}
