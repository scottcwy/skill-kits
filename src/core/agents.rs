use crate::core::ids::AgentId;
use camino::Utf8PathBuf;
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
    pub global_dir: &'static str,
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
        vec![self.global_dir.into()]
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
            global_dir: "~/.codex/skills",
            project_dir: ".agents/skills",
        },
        BuiltInAgent {
            id: "claude",
            label: "Claude Code",
            global_dir: "~/.claude/skills",
            project_dir: ".claude/skills",
        },
        BuiltInAgent {
            id: "gemini",
            label: "Gemini CLI",
            global_dir: "~/.gemini/skills",
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
            global_skill_dirs: vec![agent.global_dir.into()],
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

pub fn global_skill_dirs_for(agent_id: &AgentId) -> Option<Vec<Utf8PathBuf>> {
    built_in_agent_config(agent_id).map(|agent| agent.global_skill_dirs)
}
