use crate::core::ids::{AgentId, SkillId};
use camino::Utf8PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, SkillKitsError>;

#[derive(Debug, Error)]
pub enum SkillKitsError {
    #[error("registry busy: another Skill-kits process is writing; retry or run doctor if stale")]
    RegistryBusy,
    #[error("registry parse failed for {path}")]
    RegistryParse {
        path: Utf8PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("invalid Skill directory {path}: {reason}")]
    InvalidSkillDir { path: Utf8PathBuf, reason: String },
    #[error("Skill not found: {query}")]
    SkillNotFound { query: String },
    #[error("ambiguous Skill query {query}: {matches:?}")]
    AmbiguousSkill {
        query: String,
        matches: Vec<SkillId>,
    },
    #[error("Agent not found: {agent_id}")]
    AgentNotFound { agent_id: AgentId },
    #[error("project not found: {path}")]
    ProjectNotFound { path: Utf8PathBuf },
    #[error("deploy conflict at {target}: target exists; adopt it, remove it, or choose another Skill name")]
    DeployConflict { target: Utf8PathBuf },
    #[error("adoption conflict for Skill {name}")]
    AdoptionConflict { name: String },
    #[error("invalid toggle state at {path}")]
    InvalidToggleState { path: Utf8PathBuf },
    #[error(
        "project copy has local changes for {deployment_id}; use --overwrite, --promote, or keep it"
    )]
    DeploymentDrift { deployment_id: String },
    #[error("missing managed source {skill_id} for deployment {deployment_id}: promote it or remove it from the project")]
    MissingManagedSource {
        skill_id: SkillId,
        deployment_id: String,
    },
    #[error("deployment {deployment_id} has drift; pass --force to remove it")]
    UnsafeRemoveRequiresForce { deployment_id: String },
    #[error(transparent)]
    Io {
        #[from]
        source: std::io::Error,
    },
    #[error(transparent)]
    TomlSer {
        #[from]
        source: toml::ser::Error,
    },
    #[error(transparent)]
    TomlDe {
        #[from]
        source: toml::de::Error,
    },
}

impl SkillKitsError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::DeployConflict { .. }
            | Self::AdoptionConflict { .. }
            | Self::DeploymentDrift { .. }
            | Self::UnsafeRemoveRequiresForce { .. } => 3,
            Self::RegistryBusy => 4,
            _ => 1,
        }
    }
}
