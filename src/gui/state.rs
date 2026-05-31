use crate::core::{
    agents::AgentConfig,
    config::{read_config, RecentProject},
    ids::{AgentId, SkillId},
    paths::AppPaths,
    project::project_deployment_status,
    registry::{
        read_deployments_registry, read_skills_registry, DeploymentRecord, DeploymentStatus,
        ManagedSkill, SkillSource,
    },
    Result,
};
use camino::Utf8PathBuf;
use egui::Color32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationView {
    Dashboard,
    Skills,
    Agents,
    Projects,
}

impl NavigationView {
    pub const ORDER: [Self; 4] = [Self::Dashboard, Self::Skills, Self::Agents, Self::Projects];

    pub fn title(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Skills => "Skills",
            Self::Agents => "Agents",
            Self::Projects => "Projects",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuiScope {
    GlobalInventory,
    Project(Utf8PathBuf),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuiActionIntent {
    ScanSkill {
        skill_id: SkillId,
    },
    UninstallSkill {
        skill_id: SkillId,
    },
    DeploySkill {
        project_path: Utf8PathBuf,
        agent_id: AgentId,
        skill_id: SkillId,
    },
    EnableDeployment {
        project_path: Utf8PathBuf,
        agent_id: AgentId,
        skill_name: String,
    },
    DisableDeployment {
        project_path: Utf8PathBuf,
        agent_id: AgentId,
        skill_name: String,
    },
    RemoveDeployment {
        project_path: Utf8PathBuf,
        agent_id: AgentId,
        skill_name: String,
        force: bool,
    },
    RefreshProject {
        project_path: Utf8PathBuf,
    },
    OpenProject {
        project_path: Utf8PathBuf,
    },
    EditAgent {
        agent_id: AgentId,
    },
    AddCustomAgent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DashboardSummary {
    pub managed_skill_count: usize,
    pub agent_count: usize,
    pub enabled_agent_count: usize,
    pub recent_project_count: usize,
    pub deployment_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSummary {
    pub name: String,
    pub path: Utf8PathBuf,
    pub deployment_count: usize,
    pub discovered_unmanaged_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderRow {
    pub id: String,
    pub cells: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectorSection {
    pub title: String,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderableView {
    pub view: NavigationView,
    pub title: String,
    pub columns: Vec<String>,
    pub main_rows: Vec<RenderRow>,
    pub inspector_sections: Vec<InspectorSection>,
}

#[derive(Clone, Debug)]
pub struct GuiModel {
    pub active_view: NavigationView,
    pub active_scope: GuiScope,
    pub dashboard: DashboardSummary,
    pub skills: Vec<ManagedSkill>,
    pub agents: Vec<AgentConfig>,
    pub recent_projects: Vec<RecentProject>,
    pub project_summaries: Vec<ProjectSummary>,
    pub deployments: Vec<DeploymentRecord>,
    pub deployment_statuses: Vec<DeploymentStatus>,
    selected_skill: Option<SkillId>,
    selected_agent: Option<AgentId>,
    selected_project: Option<Utf8PathBuf>,
    selected_deployment: Option<String>,
    pending_intents: Vec<GuiActionIntent>,
}

impl GuiModel {
    pub fn load(paths: &AppPaths) -> Result<Self> {
        let config = read_config(paths)?;
        let skills = read_skills_registry(paths)?.skills;
        let deployments = read_deployments_registry(paths)?.deployments;
        let deployment_statuses = deployments
            .iter()
            .map(|deployment| {
                project_deployment_status(
                    paths,
                    &deployment.project_path,
                    &deployment.agent_id,
                    &deployment.skill_name,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let project_summaries = config
            .recent_projects
            .iter()
            .map(|project| {
                let deployment_count = deployments
                    .iter()
                    .filter(|deployment| deployment.project_path == project.path)
                    .count();
                ProjectSummary {
                    name: project.name.clone(),
                    path: project.path.clone(),
                    deployment_count,
                    discovered_unmanaged_count: 0,
                }
            })
            .collect::<Vec<_>>();
        let dashboard = DashboardSummary {
            managed_skill_count: skills.len(),
            agent_count: config.agents.len(),
            enabled_agent_count: config.agents.iter().filter(|agent| agent.enabled).count(),
            recent_project_count: config.recent_projects.len(),
            deployment_count: deployments.len(),
        };
        let selected_project = config
            .recent_projects
            .first()
            .map(|project| project.path.clone());

        Ok(Self {
            active_view: NavigationView::Dashboard,
            active_scope: GuiScope::GlobalInventory,
            dashboard,
            skills,
            agents: config.agents,
            recent_projects: config.recent_projects,
            project_summaries,
            deployments,
            deployment_statuses,
            selected_skill: None,
            selected_agent: None,
            selected_project,
            selected_deployment: None,
            pending_intents: Vec::new(),
        })
    }

    pub fn navigate(&mut self, view: NavigationView) {
        self.active_view = view;
    }

    pub fn select_scope(&mut self, scope: GuiScope) {
        if let GuiScope::Project(path) = &scope {
            self.selected_project = Some(path.clone());
        }
        self.active_scope = scope;
    }

    pub fn select_skill(&mut self, skill_id: SkillId) {
        self.selected_skill = Some(skill_id);
    }

    pub fn select_agent(&mut self, agent_id: AgentId) {
        self.selected_agent = Some(agent_id);
    }

    pub fn select_project(&mut self, project_path: Utf8PathBuf) {
        self.selected_project = Some(project_path);
    }

    pub fn select_deployment(&mut self, deployment_id: String) {
        self.selected_deployment = Some(deployment_id);
    }

    pub fn pending_intents(&self) -> &[GuiActionIntent] {
        &self.pending_intents
    }

    pub fn request_scan_selected_skill(&mut self) -> Option<GuiActionIntent> {
        let skill_id = self.selected_skill.clone()?;
        self.push_intent(GuiActionIntent::ScanSkill { skill_id })
    }

    pub fn request_uninstall_selected_skill(&mut self) -> Option<GuiActionIntent> {
        let skill_id = self.selected_skill.clone()?;
        self.push_intent(GuiActionIntent::UninstallSkill { skill_id })
    }

    pub fn request_deploy_selected_skill(&mut self, agent_id: AgentId) -> Option<GuiActionIntent> {
        let project_path = self.scope_project_path()?;
        let skill_id = self.selected_skill.clone()?;
        self.push_intent(GuiActionIntent::DeploySkill {
            project_path,
            agent_id,
            skill_id,
        })
    }

    pub fn request_refresh_selected_project(&mut self) -> Option<GuiActionIntent> {
        let project_path = self.selected_project.clone()?;
        self.push_intent(GuiActionIntent::RefreshProject { project_path })
    }

    pub fn renderable_view(&self) -> RenderableView {
        match self.active_view {
            NavigationView::Dashboard => crate::gui::dashboard::renderable(self),
            NavigationView::Skills => crate::gui::skills::renderable(self),
            NavigationView::Agents => crate::gui::agents::renderable(self),
            NavigationView::Projects => crate::gui::projects::renderable(self),
        }
    }

    pub fn selected_skill(&self) -> Option<&ManagedSkill> {
        self.selected_skill.as_ref().and_then(|selected| {
            self.skills
                .iter()
                .find(|skill| skill.id.as_str() == selected.as_str())
        })
    }

    pub fn selected_agent(&self) -> Option<&AgentConfig> {
        self.selected_agent.as_ref().and_then(|selected| {
            self.agents
                .iter()
                .find(|agent| agent.id.as_str() == selected.as_str())
        })
    }

    pub fn selected_project_summary(&self) -> Option<&ProjectSummary> {
        self.selected_project.as_ref().and_then(|selected| {
            self.project_summaries
                .iter()
                .find(|project| project.path == *selected)
        })
    }

    pub fn selected_deployment(&self) -> Option<&DeploymentRecord> {
        self.selected_deployment.as_ref().and_then(|selected| {
            self.deployments
                .iter()
                .find(|deployment| deployment.id == *selected)
        })
    }

    pub fn selected_deployment_status(&self) -> Option<&DeploymentStatus> {
        self.selected_deployment.as_ref().and_then(|selected| {
            self.deployment_statuses
                .iter()
                .find(|status| status.record.id == *selected)
        })
    }

    pub fn scope_project_path(&self) -> Option<Utf8PathBuf> {
        match &self.active_scope {
            GuiScope::GlobalInventory => self.selected_project.clone(),
            GuiScope::Project(path) => Some(path.clone()),
        }
    }

    fn push_intent(&mut self, intent: GuiActionIntent) -> Option<GuiActionIntent> {
        self.pending_intents.push(intent.clone());
        Some(intent)
    }
}

impl Default for GuiModel {
    fn default() -> Self {
        Self {
            active_view: NavigationView::Dashboard,
            active_scope: GuiScope::GlobalInventory,
            dashboard: DashboardSummary {
                managed_skill_count: 0,
                agent_count: 0,
                enabled_agent_count: 0,
                recent_project_count: 0,
                deployment_count: 0,
            },
            skills: Vec::new(),
            agents: Vec::new(),
            recent_projects: Vec::new(),
            project_summaries: Vec::new(),
            deployments: Vec::new(),
            deployment_statuses: Vec::new(),
            selected_skill: None,
            selected_agent: None,
            selected_project: None,
            selected_deployment: None,
            pending_intents: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct UiColors {
    pub canvas: Color32,
    pub surface_1: Color32,
    pub surface_2: Color32,
    pub surface_3: Color32,
    pub surface_4: Color32,
    pub hairline: Color32,
    pub hairline_strong: Color32,
    pub ink: Color32,
    pub ink_muted: Color32,
    pub ink_subtle: Color32,
    pub ink_tertiary: Color32,
    pub inverse_ink: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub info: Color32,
    pub focus: Color32,
}

impl UiColors {
    pub fn dark() -> Self {
        Self {
            canvas: Color32::from_rgb(0x08, 0x09, 0x0b),
            surface_1: Color32::from_rgb(0x10, 0x11, 0x14),
            surface_2: Color32::from_rgb(0x17, 0x19, 0x1d),
            surface_3: Color32::from_rgb(0x20, 0x22, 0x27),
            surface_4: Color32::from_rgb(0x2a, 0x2d, 0x33),
            hairline: Color32::from_rgb(0x25, 0x27, 0x2d),
            hairline_strong: Color32::from_rgb(0x36, 0x39, 0x42),
            ink: Color32::from_rgb(0xf2, 0xf3, 0xf3),
            ink_muted: Color32::from_rgb(0xb9, 0xbe, 0xc7),
            ink_subtle: Color32::from_rgb(0x85, 0x8b, 0x96),
            ink_tertiary: Color32::from_rgb(0x5f, 0x65, 0x70),
            inverse_ink: Color32::from_rgb(0x11, 0x12, 0x16),
            success: Color32::from_rgb(0x67, 0xa8, 0x78),
            warning: Color32::from_rgb(0xc5, 0xa3, 0x65),
            danger: Color32::from_rgb(0xd0, 0x6b, 0x6b),
            info: Color32::from_rgb(0x9e, 0xa4, 0xad),
            focus: Color32::from_rgb(0xe4, 0xe6, 0xeb),
        }
    }
}

pub fn skill_source_label(source: &SkillSource) -> String {
    match source {
        SkillSource::Local { .. } => "Local".to_string(),
        SkillSource::GlobalAgentAdopt { agent_id, .. } => {
            format!("Global adopt / {agent_id}")
        }
        SkillSource::ProjectAdopt { agent_id, .. } => {
            format!("Project adopt / {agent_id}")
        }
        SkillSource::PromotedFromProject { .. } => "Promoted".to_string(),
    }
}
