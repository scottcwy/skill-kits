use crate::core::{
    config::{read_config, update_config},
    error::{Result, SkillKitsError},
    ids::AgentId,
    paths::AppPaths,
};
use camino::{Utf8Component, Utf8PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    BuiltIn,
    Custom,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: AgentId,
    pub label: String,
    pub kind: AgentKind,
    #[serde(default)]
    pub global_skill_dirs: Vec<Utf8PathBuf>,
    #[serde(default)]
    pub project_skill_dirs: Vec<Utf8PathBuf>,
    pub enabled: bool,
}

pub trait AgentAdapter {
    fn id(&self) -> AgentId;
    fn label(&self) -> &'static str;
    fn default_global_skill_dirs(&self) -> Vec<Utf8PathBuf>;
    fn default_project_skill_dirs(&self) -> Vec<Utf8PathBuf>;

    fn global_skill_dirs(&self, config: &AgentConfig) -> Vec<Utf8PathBuf> {
        if config.global_skill_dirs.is_empty() {
            self.default_global_skill_dirs()
        } else {
            config.global_skill_dirs.clone()
        }
    }

    fn project_skill_dirs(&self, config: &AgentConfig) -> Vec<Utf8PathBuf> {
        if config.project_skill_dirs.is_empty() {
            self.default_project_skill_dirs()
        } else {
            config.project_skill_dirs.clone()
        }
    }
}

#[derive(Clone, Debug)]
pub struct BuiltInAgent {
    pub id: &'static str,
    pub label: &'static str,
    pub global_dirs: &'static [&'static str],
    pub project_dir: &'static str,
}

impl AgentAdapter for BuiltInAgent {
    fn id(&self) -> AgentId {
        AgentId::new(self.id)
    }

    fn label(&self) -> &'static str {
        self.label
    }

    fn default_global_skill_dirs(&self) -> Vec<Utf8PathBuf> {
        self.global_dirs.iter().map(|dir| (*dir).into()).collect()
    }

    fn default_project_skill_dirs(&self) -> Vec<Utf8PathBuf> {
        vec![self.project_dir.into()]
    }
}

pub fn built_in_agents() -> Vec<BuiltInAgent> {
    vec![
        BuiltInAgent {
            id: "codex",
            label: "Codex",
            global_dirs: &[
                "~/.codex/skills",
                "~/.codex/plugins/cache",
                "~/.codex/vendor_imports",
            ],
            project_dir: ".agents/skills",
        },
        BuiltInAgent {
            id: "claude",
            label: "Claude Code",
            global_dirs: &["~/.claude/skills"],
            project_dir: ".claude/skills",
        },
        BuiltInAgent {
            id: "gemini",
            label: "Gemini CLI",
            global_dirs: &["~/.gemini/skills"],
            project_dir: ".gemini/skills",
        },
    ]
}

pub fn default_agent_configs() -> Vec<AgentConfig> {
    built_in_agents()
        .into_iter()
        .map(|agent| AgentConfig {
            id: AgentId::new(agent.id),
            label: agent.label.to_string(),
            kind: AgentKind::BuiltIn,
            global_skill_dirs: agent.default_global_skill_dirs(),
            project_skill_dirs: vec![agent.project_dir.into()],
            enabled: true,
        })
        .collect()
}

pub fn built_in_agent_config(agent_id: &AgentId) -> Option<AgentConfig> {
    default_agent_configs()
        .into_iter()
        .find(|agent| agent.id == *agent_id)
}

pub fn project_skill_dirs_for(agent_id: &AgentId) -> Option<Vec<Utf8PathBuf>> {
    built_in_agent_config(agent_id).map(|agent| agent.project_skill_dirs)
}

pub fn configured_project_skill_dirs_for(
    paths: &AppPaths,
    agent_id: &AgentId,
) -> Result<Vec<Utf8PathBuf>> {
    let config = read_config(paths)?;
    config
        .agents
        .into_iter()
        .find(|agent| agent.id == *agent_id)
        .map(|agent| {
            if agent.project_skill_dirs.is_empty() {
                project_skill_dirs_for(agent_id).unwrap_or_default()
            } else {
                agent.project_skill_dirs
            }
        })
        .filter(|dirs| !dirs.is_empty())
        .or_else(|| project_skill_dirs_for(agent_id))
        .ok_or_else(|| SkillKitsError::AgentNotFound {
            agent_id: agent_id.clone(),
        })
}

pub fn global_skill_dirs_for(agent_id: &AgentId) -> Option<Vec<Utf8PathBuf>> {
    built_in_agent_config(agent_id).map(|agent| agent.global_skill_dirs)
}

pub fn configured_global_skill_dirs_for(
    paths: &AppPaths,
    agent_id: &AgentId,
) -> Result<Vec<Utf8PathBuf>> {
    let config = read_config(paths)?;
    if let Some(agent) = config
        .agents
        .into_iter()
        .find(|agent| agent.id == *agent_id)
    {
        let defaults = global_skill_dirs_for(agent_id).unwrap_or_default();
        return if agent.global_skill_dirs.is_empty() {
            Ok(defaults)
        } else if matches!(agent.kind, AgentKind::BuiltIn) {
            Ok(merge_dirs(agent.global_skill_dirs, defaults))
        } else {
            Ok(agent.global_skill_dirs)
        };
    }

    global_skill_dirs_for(agent_id).ok_or_else(|| SkillKitsError::AgentNotFound {
        agent_id: agent_id.clone(),
    })
}

fn merge_dirs(mut primary: Vec<Utf8PathBuf>, secondary: Vec<Utf8PathBuf>) -> Vec<Utf8PathBuf> {
    for dir in secondary {
        if !primary.contains(&dir) {
            primary.push(dir);
        }
    }
    primary
}

pub fn add_custom_agent_config(
    paths: &AppPaths,
    id: AgentId,
    label: String,
    project_skill_dir: Utf8PathBuf,
) -> Result<AgentConfig> {
    let id = validated_agent_id(id)?;
    let label = validated_agent_label(label)?;
    validate_project_skill_dir(&project_skill_dir)?;

    update_config(paths, |config| {
        if config.agents.iter().any(|agent| agent.id == id) {
            return Err(SkillKitsError::AgentAlreadyConfigured {
                agent_id: id.clone(),
            });
        }

        let agent = AgentConfig {
            id,
            label,
            kind: AgentKind::Custom,
            global_skill_dirs: Vec::new(),
            project_skill_dirs: vec![project_skill_dir],
            enabled: true,
        };
        config.agents.push(agent.clone());
        Ok(agent)
    })
}

pub fn update_agent_project_skill_dirs(
    paths: &AppPaths,
    agent_id: &AgentId,
    project_skill_dirs: Vec<Utf8PathBuf>,
) -> Result<AgentConfig> {
    validate_project_skill_dirs(&project_skill_dirs)?;

    update_config(paths, |config| {
        let agent = config
            .agents
            .iter_mut()
            .find(|agent| agent.id == *agent_id)
            .ok_or_else(|| SkillKitsError::AgentNotFound {
                agent_id: agent_id.clone(),
            })?;

        agent.project_skill_dirs = project_skill_dirs;
        Ok(agent.clone())
    })
}

pub fn reset_agent_project_skill_dirs(paths: &AppPaths, agent_id: &AgentId) -> Result<AgentConfig> {
    let default_agent =
        built_in_agent_config(agent_id).ok_or_else(|| SkillKitsError::InvalidAgentConfig {
            reason: format!("{agent_id} is not a built-in Agent"),
        })?;

    update_config(paths, |config| {
        let agent = config
            .agents
            .iter_mut()
            .find(|agent| agent.id == *agent_id)
            .ok_or_else(|| SkillKitsError::AgentNotFound {
                agent_id: agent_id.clone(),
            })?;
        if agent.kind != AgentKind::BuiltIn {
            return Err(SkillKitsError::InvalidAgentConfig {
                reason: format!("{agent_id} is not a built-in Agent"),
            });
        }

        agent.project_skill_dirs = default_agent.project_skill_dirs;
        Ok(agent.clone())
    })
}

pub fn remove_custom_agent_config(paths: &AppPaths, agent_id: &AgentId) -> Result<AgentConfig> {
    update_config(paths, |config| {
        let index = config
            .agents
            .iter()
            .position(|agent| agent.id == *agent_id)
            .ok_or_else(|| SkillKitsError::AgentNotFound {
                agent_id: agent_id.clone(),
            })?;
        if config.agents[index].kind != AgentKind::Custom {
            return Err(SkillKitsError::InvalidAgentConfig {
                reason: format!("{agent_id} is not a custom Agent"),
            });
        }

        Ok(config.agents.remove(index))
    })
}

fn validated_agent_id(id: AgentId) -> Result<AgentId> {
    let value = id.as_str().trim();
    if value.is_empty() {
        return Err(SkillKitsError::InvalidAgentConfig {
            reason: "agent id cannot be empty".to_string(),
        });
    }
    Ok(AgentId::new(value))
}

fn validated_agent_label(label: String) -> Result<String> {
    let label = label.trim();
    if label.is_empty() {
        return Err(SkillKitsError::InvalidAgentConfig {
            reason: "agent label cannot be empty".to_string(),
        });
    }
    Ok(label.to_string())
}

fn validate_project_skill_dirs(project_skill_dirs: &[Utf8PathBuf]) -> Result<()> {
    if project_skill_dirs.is_empty() {
        return Err(SkillKitsError::InvalidSkillDir {
            path: Utf8PathBuf::new(),
            reason: "at least one project Skill directory is required".to_string(),
        });
    }

    for dir in project_skill_dirs {
        validate_project_skill_dir(dir)?;
    }

    Ok(())
}

fn validate_project_skill_dir(project_skill_dir: &Utf8PathBuf) -> Result<()> {
    if project_skill_dir.as_str().trim().is_empty() {
        return Err(SkillKitsError::InvalidSkillDir {
            path: project_skill_dir.clone(),
            reason: "project Skill directory cannot be empty".to_string(),
        });
    }

    if project_skill_dir.is_absolute() {
        return Err(SkillKitsError::InvalidSkillDir {
            path: project_skill_dir.clone(),
            reason: "project Skill directory must be relative".to_string(),
        });
    }

    if project_skill_dir
        .components()
        .any(|component| matches!(component, Utf8Component::ParentDir))
    {
        return Err(SkillKitsError::InvalidSkillDir {
            path: project_skill_dir.clone(),
            reason: "project Skill directory cannot contain parent traversal".to_string(),
        });
    }

    Ok(())
}
