pub mod adopt;
pub mod agent_space;
pub mod agents;
pub mod config;
pub mod doctor;
pub mod error;
pub mod fs;
pub mod hash;
pub mod ids;
pub mod install;
pub mod lock;
pub mod onboarding;
pub mod paths;
pub mod project;
pub mod registry;
pub mod scan;
pub mod skills;
pub mod status;

pub use error::{Result, SkillKitsError};
pub use registry::{
    DeploymentRecord, DeploymentStatus, ManagedSkill, SkillMetadata, SkillSource, ToggleState,
};
