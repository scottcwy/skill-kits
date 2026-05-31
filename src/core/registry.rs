use crate::core::error::{Result, SkillKitsError};
use crate::core::fs::{atomic_write_toml, safe_read_to_string};
use crate::core::ids::{AgentId, SkillId};
use crate::core::lock::StateLock;
use crate::core::paths::{ensure_app_dirs, AppPaths};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManagedSkill {
    pub id: SkillId,
    pub name: String,
    pub source: SkillSource,
    pub managed_path: Utf8PathBuf,
    pub content_hash: String,
    pub metadata: Option<SkillMetadata>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SkillSource {
    Local {
        source_path: Utf8PathBuf,
    },
    GlobalAgentAdopt {
        agent_id: AgentId,
        source_path: Utf8PathBuf,
    },
    ProjectAdopt {
        agent_id: AgentId,
        project_path: Utf8PathBuf,
        source_path: Utf8PathBuf,
    },
    PromotedFromProject {
        deployment_id: String,
        project_path: Utf8PathBuf,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub frontmatter: toml::value::Table,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeploymentRecord {
    pub id: String,
    pub skill_id: SkillId,
    pub agent_id: AgentId,
    pub project_name: String,
    pub project_path: Utf8PathBuf,
    pub deployment_path: Utf8PathBuf,
    pub skill_name: String,
    pub baseline_hash: String,
    pub deployed_from_hash: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ToggleState {
    Enabled,
    Disabled,
    InvalidBothPresent,
    InvalidBothMissing,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeploymentStatus {
    pub record: DeploymentRecord,
    pub toggle: ToggleState,
    pub current_hash: Option<String>,
    pub drift: bool,
    pub outdated: bool,
    pub missing_managed_source: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillsRegistry {
    pub version: u32,
    #[serde(default)]
    pub skills: Vec<ManagedSkill>,
}

impl Default for SkillsRegistry {
    fn default() -> Self {
        Self {
            version: 1,
            skills: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeploymentsRegistry {
    pub version: u32,
    #[serde(default)]
    pub deployments: Vec<DeploymentRecord>,
}

impl Default for DeploymentsRegistry {
    fn default() -> Self {
        Self {
            version: 1,
            deployments: Vec::new(),
        }
    }
}

pub fn read_skills_registry(paths: &AppPaths) -> Result<SkillsRegistry> {
    ensure_app_dirs(paths)?;
    if !paths.skills_registry_file.exists() {
        let _lock = StateLock::acquire(paths)?;
        ensure_app_dirs(paths)?;
        let registry = read_skills_registry_unlocked(paths)?;
        if !paths.skills_registry_file.exists() {
            atomic_write_toml(&paths.skills_registry_file, &registry)?;
        }
        return Ok(registry);
    }

    read_skills_registry_unlocked(paths)
}

fn read_skills_registry_unlocked(paths: &AppPaths) -> Result<SkillsRegistry> {
    if !paths.skills_registry_file.exists() {
        return Ok(SkillsRegistry::default());
    }

    let contents = safe_read_to_string(&paths.skills_registry_file)?;
    toml::from_str(&contents).map_err(|source| SkillKitsError::RegistryParse {
        path: paths.skills_registry_file.clone(),
        source,
    })
}

pub fn write_skills_registry(paths: &AppPaths, registry: &SkillsRegistry) -> Result<()> {
    let _lock = StateLock::acquire(paths)?;
    ensure_app_dirs(paths)?;
    atomic_write_toml(&paths.skills_registry_file, registry)
}

pub fn update_skills_registry<R>(
    paths: &AppPaths,
    update: impl FnOnce(&mut SkillsRegistry) -> Result<R>,
) -> Result<R> {
    let _lock = StateLock::acquire(paths)?;
    ensure_app_dirs(paths)?;
    let mut registry = read_skills_registry_unlocked(paths)?;
    let result = update(&mut registry)?;
    atomic_write_toml(&paths.skills_registry_file, &registry)?;
    Ok(result)
}

pub fn read_deployments_registry(paths: &AppPaths) -> Result<DeploymentsRegistry> {
    ensure_app_dirs(paths)?;
    if !paths.deployments_registry_file.exists() {
        let _lock = StateLock::acquire(paths)?;
        ensure_app_dirs(paths)?;
        let registry = read_deployments_registry_unlocked(paths)?;
        if !paths.deployments_registry_file.exists() {
            atomic_write_toml(&paths.deployments_registry_file, &registry)?;
        }
        return Ok(registry);
    }

    read_deployments_registry_unlocked(paths)
}

fn read_deployments_registry_unlocked(paths: &AppPaths) -> Result<DeploymentsRegistry> {
    if !paths.deployments_registry_file.exists() {
        return Ok(DeploymentsRegistry::default());
    }

    let contents = safe_read_to_string(&paths.deployments_registry_file)?;
    toml::from_str(&contents).map_err(|source| SkillKitsError::RegistryParse {
        path: paths.deployments_registry_file.clone(),
        source,
    })
}

pub fn write_deployments_registry(paths: &AppPaths, registry: &DeploymentsRegistry) -> Result<()> {
    let _lock = StateLock::acquire(paths)?;
    ensure_app_dirs(paths)?;
    atomic_write_toml(&paths.deployments_registry_file, registry)
}

pub fn update_deployments_registry<R>(
    paths: &AppPaths,
    update: impl FnOnce(&mut DeploymentsRegistry) -> Result<R>,
) -> Result<R> {
    let _lock = StateLock::acquire(paths)?;
    ensure_app_dirs(paths)?;
    let mut registry = read_deployments_registry_unlocked(paths)?;
    let result = update(&mut registry)?;
    atomic_write_toml(&paths.deployments_registry_file, &registry)?;
    Ok(result)
}
