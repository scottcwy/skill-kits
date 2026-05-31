use crate::core::agents::{default_agent_configs, AgentConfig};
use crate::core::error::{Result, SkillKitsError};
use crate::core::fs::{atomic_write_toml, safe_read_to_string};
use crate::core::lock::StateLock;
use crate::core::paths::{ensure_app_dirs, AppPaths};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecentProject {
    pub name: String,
    pub path: Utf8PathBuf,
    pub last_opened_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    pub theme: String,
    #[serde(default = "default_agent_configs")]
    pub agents: Vec<AgentConfig>,
    #[serde(default)]
    pub recent_projects: Vec<RecentProject>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            theme: "system".to_string(),
            agents: default_agent_configs(),
            recent_projects: Vec::new(),
        }
    }
}

pub fn read_config(paths: &AppPaths) -> Result<Config> {
    read_or_init_config(paths)
}

pub fn write_config(paths: &AppPaths, config: &Config) -> Result<()> {
    let _lock = StateLock::acquire(paths)?;
    ensure_app_dirs(paths)?;
    atomic_write_toml(&paths.config_file, config)
}

pub fn update_config<R>(
    paths: &AppPaths,
    update: impl FnOnce(&mut Config) -> Result<R>,
) -> Result<R> {
    let _lock = StateLock::acquire(paths)?;
    ensure_app_dirs(paths)?;
    let mut config = read_config_unlocked(paths)?;
    let result = update(&mut config)?;
    atomic_write_toml(&paths.config_file, &config)?;
    Ok(result)
}

fn read_or_init_config(paths: &AppPaths) -> Result<Config> {
    ensure_app_dirs(paths)?;
    if !paths.config_file.exists() {
        let _lock = StateLock::acquire(paths)?;
        ensure_app_dirs(paths)?;
        let config = read_config_unlocked(paths)?;
        if !paths.config_file.exists() {
            atomic_write_toml(&paths.config_file, &config)?;
        }
        return Ok(config);
    }

    read_config_unlocked(paths)
}

fn read_config_unlocked(paths: &AppPaths) -> Result<Config> {
    if !paths.config_file.exists() {
        return Ok(Config::default());
    }

    let contents = safe_read_to_string(&paths.config_file)?;
    toml::from_str(&contents).map_err(|source| SkillKitsError::RegistryParse {
        path: paths.config_file.clone(),
        source,
    })
}
