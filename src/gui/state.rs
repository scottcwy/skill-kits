use crate::core::{
    adopt::{project_adopt_all, project_adopt_conflict_as_new, ProjectAdoptRequest},
    agents::{add_custom_agent_config, update_agent_project_skill_dirs, AgentConfig},
    config::{read_config, RecentProject},
    ids::{AgentId, SkillId},
    install::{uninstall_skill, UninstallSkillRequest},
    onboarding::{project_onboarding_scan, DiscoveredProjectSkill, ProjectOnboardingScanRequest},
    paths::AppPaths,
    project::{
        deploy_project_skill, disable_project_skill, enable_project_skill,
        project_deployment_status, redeploy_project_skill, remove_project_skill,
        ProjectDeployRequest, ProjectRedeployRequest, ProjectRemoveRequest, ProjectSkillRequest,
    },
    registry::{
        read_deployments_registry, read_skills_registry, DeploymentRecord, DeploymentStatus,
        ManagedSkill, SkillSource,
    },
    scan::{scan_skill_dir, RiskFinding, RiskSeverity},
    Result, SkillKitsError,
};
use camino::Utf8PathBuf;
use egui::Color32;

pub const DRIFT_REMOVE_CONFIRMATION_MESSAGE: &str =
    "This project copy has local changes. Removing it deletes only this deployed Skill, not the Agent skill root.";

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
    RedeployDeployment {
        project_path: Utf8PathBuf,
        agent_id: AgentId,
        skill_name: String,
        overwrite: bool,
        promote: bool,
    },
    RefreshProject {
        project_path: Utf8PathBuf,
    },
    ProjectAdoptAll {
        project_path: Utf8PathBuf,
    },
    ProjectImportConflictAsNew {
        project_path: Utf8PathBuf,
        agent_id: AgentId,
        skill_name: String,
    },
    OpenProject {
        project_path: Utf8PathBuf,
    },
    UpdateAgentProjectSkillDirs {
        agent_id: AgentId,
        project_skill_dirs: Vec<Utf8PathBuf>,
    },
    AddCustomAgent {
        agent_id: AgentId,
        label: String,
        project_skill_dir: Utf8PathBuf,
    },
}

#[derive(Clone, Debug)]
pub struct GuiController {
    paths: AppPaths,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuiControllerOutcome {
    None,
    SkillScan {
        skill_id: SkillId,
        scanned_hash: String,
        findings: Vec<RiskFinding>,
    },
    ProjectScan {
        project_path: Utf8PathBuf,
        discovered_unmanaged_count: usize,
        adopt_result: Option<ProjectAdoptAllSummary>,
        pending_conflicts: Vec<ProjectConflict>,
        preserve_existing_conflicts: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectAdoptAllSummary {
    pub imported: usize,
    pub conflicts: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectConflict {
    pub agent_id: AgentId,
    pub skill_name: String,
}

fn unresolved_project_conflicts(discovered: &[DiscoveredProjectSkill]) -> Vec<ProjectConflict> {
    discovered
        .iter()
        .map(|skill| ProjectConflict {
            agent_id: skill.agent_id.clone(),
            skill_name: skill.name.clone(),
        })
        .collect()
}

fn pending_conflicts_minus_skipped(
    pending_conflicts: Vec<ProjectConflict>,
    skipped_conflicts: &[ProjectConflict],
) -> Vec<ProjectConflict> {
    pending_conflicts
        .into_iter()
        .filter(|conflict| !skipped_conflicts.contains(conflict))
        .collect()
}

fn pending_conflicts_intersection(
    pending_conflicts: Vec<ProjectConflict>,
    discovered_conflicts: &[ProjectConflict],
) -> Vec<ProjectConflict> {
    pending_conflicts
        .into_iter()
        .filter(|conflict| discovered_conflicts.contains(conflict))
        .collect()
}

impl GuiController {
    pub fn new(paths: AppPaths) -> Self {
        Self { paths }
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn execute(&self, intent: &GuiActionIntent) -> Result<GuiControllerOutcome> {
        let outcome = match intent {
            GuiActionIntent::ScanSkill { skill_id } => {
                let skill = read_skills_registry(&self.paths)?
                    .skills
                    .into_iter()
                    .find(|skill| skill.id == *skill_id)
                    .ok_or_else(|| SkillKitsError::SkillNotFound {
                        query: skill_id.to_string(),
                    })?;
                let findings = scan_skill_dir(&skill.managed_path)?;
                GuiControllerOutcome::SkillScan {
                    skill_id: skill_id.clone(),
                    scanned_hash: skill.content_hash,
                    findings,
                }
            }
            GuiActionIntent::UninstallSkill { skill_id } => {
                uninstall_skill(UninstallSkillRequest {
                    app_paths: &self.paths,
                    query: skill_id.as_str(),
                })?;
                GuiControllerOutcome::None
            }
            GuiActionIntent::DeploySkill {
                project_path,
                agent_id,
                skill_id,
            } => {
                deploy_project_skill(ProjectDeployRequest {
                    app_paths: &self.paths,
                    project_path,
                    agent_id,
                    skill_query: skill_id.as_str(),
                })?;
                GuiControllerOutcome::None
            }
            GuiActionIntent::EnableDeployment {
                project_path,
                agent_id,
                skill_name,
            } => {
                enable_project_skill(ProjectSkillRequest {
                    app_paths: &self.paths,
                    project_path,
                    agent_id,
                    skill_query: skill_name,
                })?;
                GuiControllerOutcome::None
            }
            GuiActionIntent::DisableDeployment {
                project_path,
                agent_id,
                skill_name,
            } => {
                disable_project_skill(ProjectSkillRequest {
                    app_paths: &self.paths,
                    project_path,
                    agent_id,
                    skill_query: skill_name,
                })?;
                GuiControllerOutcome::None
            }
            GuiActionIntent::RemoveDeployment {
                project_path,
                agent_id,
                skill_name,
                force,
            } => {
                remove_project_skill(ProjectRemoveRequest {
                    app_paths: &self.paths,
                    project_path,
                    agent_id,
                    skill_query: skill_name,
                    force: *force,
                })?;
                GuiControllerOutcome::None
            }
            GuiActionIntent::RedeployDeployment {
                project_path,
                agent_id,
                skill_name,
                overwrite,
                promote,
            } => {
                let _outcome = redeploy_project_skill(ProjectRedeployRequest {
                    app_paths: &self.paths,
                    project_path,
                    agent_id,
                    skill_query: skill_name,
                    overwrite: *overwrite,
                    promote: *promote,
                })?;
                GuiControllerOutcome::None
            }
            GuiActionIntent::RefreshProject { project_path } => {
                let report = project_onboarding_scan(ProjectOnboardingScanRequest {
                    app_paths: &self.paths,
                    project_path,
                })?;
                let pending_conflicts = unresolved_project_conflicts(&report.discovered);
                GuiControllerOutcome::ProjectScan {
                    project_path: report.project_path,
                    discovered_unmanaged_count: report.discovered.len(),
                    adopt_result: None,
                    pending_conflicts,
                    preserve_existing_conflicts: true,
                }
            }
            GuiActionIntent::ProjectAdoptAll { project_path } => {
                let config = read_config(&self.paths)?;
                let mut imported = 0;
                let mut conflicts = 0;
                for agent in config.agents.iter().filter(|agent| agent.enabled) {
                    let report = project_adopt_all(ProjectAdoptRequest {
                        app_paths: &self.paths,
                        project_path,
                        agent_id: &agent.id,
                        skill_name: "",
                    })?;
                    imported += report.imported;
                    conflicts += report.conflicts;
                }
                let report = project_onboarding_scan(ProjectOnboardingScanRequest {
                    app_paths: &self.paths,
                    project_path,
                })?;
                let pending_conflicts = unresolved_project_conflicts(&report.discovered);
                GuiControllerOutcome::ProjectScan {
                    project_path: report.project_path,
                    discovered_unmanaged_count: report.discovered.len(),
                    adopt_result: Some(ProjectAdoptAllSummary {
                        imported,
                        conflicts,
                    }),
                    pending_conflicts,
                    preserve_existing_conflicts: false,
                }
            }
            GuiActionIntent::ProjectImportConflictAsNew {
                project_path,
                agent_id,
                skill_name,
            } => {
                let _report = project_adopt_conflict_as_new(ProjectAdoptRequest {
                    app_paths: &self.paths,
                    project_path,
                    agent_id,
                    skill_name,
                })?;
                let report = project_onboarding_scan(ProjectOnboardingScanRequest {
                    app_paths: &self.paths,
                    project_path,
                })?;
                let pending_conflicts = unresolved_project_conflicts(&report.discovered);
                GuiControllerOutcome::ProjectScan {
                    project_path: report.project_path,
                    discovered_unmanaged_count: report.discovered.len(),
                    adopt_result: None,
                    pending_conflicts,
                    preserve_existing_conflicts: false,
                }
            }
            GuiActionIntent::OpenProject { .. } => GuiControllerOutcome::None,
            GuiActionIntent::UpdateAgentProjectSkillDirs {
                agent_id,
                project_skill_dirs,
            } => {
                update_agent_project_skill_dirs(&self.paths, agent_id, project_skill_dirs.clone())?;
                GuiControllerOutcome::None
            }
            GuiActionIntent::AddCustomAgent {
                agent_id,
                label,
                project_skill_dir,
            } => {
                add_custom_agent_config(
                    &self.paths,
                    agent_id.clone(),
                    label.clone(),
                    project_skill_dir.clone(),
                )?;
                GuiControllerOutcome::None
            }
        };
        Ok(outcome)
    }
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
    pub last_adopt_all_result: Option<ProjectAdoptAllSummary>,
    pub pending_conflicts: Vec<ProjectConflict>,
    pub skipped_conflicts: Vec<ProjectConflict>,
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
    pub empty_message: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuiStatusKind {
    Success,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuiStatus {
    pub kind: GuiStatusKind,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuiRiskReport {
    pub scanned_hash: String,
    pub findings: Vec<RiskFinding>,
}

impl GuiRiskReport {
    pub fn summary_label(&self) -> String {
        let high = self
            .findings
            .iter()
            .filter(|finding| finding.severity == RiskSeverity::High)
            .count();
        let warn = self
            .findings
            .iter()
            .filter(|finding| finding.severity == RiskSeverity::Warn)
            .count();
        let info = self
            .findings
            .iter()
            .filter(|finding| finding.severity == RiskSeverity::Info)
            .count();

        let mut parts = Vec::new();
        if high > 0 {
            parts.push(format!("{high} high"));
        }
        if warn > 0 {
            parts.push(format!("{warn} warn"));
        }
        if info > 0 {
            parts.push(format!("{info} info"));
        }

        if parts.is_empty() {
            "No findings".to_string()
        } else {
            parts.join(", ")
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentEditorMode {
    AddCustom,
    EditPath { agent_id: AgentId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentEditorDraft {
    pub mode: AgentEditorMode,
    pub id_text: String,
    pub label_text: String,
    pub project_dir_text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectDeployTarget {
    pub project_path: Utf8PathBuf,
    pub skill_id: SkillId,
    pub skill_name: String,
    pub agent_id: AgentId,
    pub agent_label: String,
    pub target_path: Utf8PathBuf,
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
    pending_remove_confirmation: Option<String>,
    pending_intents: Vec<GuiActionIntent>,
    last_status: Option<GuiStatus>,
    skill_risk_reports: Vec<(SkillId, GuiRiskReport)>,
    agent_editor_draft: Option<AgentEditorDraft>,
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
                    last_adopt_all_result: None,
                    pending_conflicts: Vec::new(),
                    skipped_conflicts: Vec::new(),
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
            pending_remove_confirmation: None,
            pending_intents: Vec::new(),
            last_status: None,
            skill_risk_reports: Vec::new(),
            agent_editor_draft: None,
        })
    }

    pub fn navigate(&mut self, view: NavigationView) {
        self.active_view = view;
    }

    pub fn select_scope(&mut self, scope: GuiScope) {
        if let GuiScope::Project(path) = &scope {
            self.selected_project = Some(path.clone());
            self.selected_deployment = None;
            self.pending_remove_confirmation = None;
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
        self.selected_deployment = None;
        self.pending_remove_confirmation = None;
    }

    pub fn select_deployment(&mut self, deployment_id: String) {
        if self
            .pending_remove_confirmation
            .as_ref()
            .is_some_and(|pending| pending != &deployment_id)
        {
            self.pending_remove_confirmation = None;
        }
        self.selected_deployment = Some(deployment_id);
    }

    pub fn select_render_row(&mut self, row_id: &str) -> bool {
        match self.active_view {
            NavigationView::Dashboard => false,
            NavigationView::Skills => {
                if self.skills.iter().any(|skill| skill.id.as_str() == row_id) {
                    self.select_skill(SkillId::new(row_id));
                    true
                } else {
                    false
                }
            }
            NavigationView::Agents => {
                if self.agents.iter().any(|agent| agent.id.as_str() == row_id) {
                    self.select_agent(AgentId::new(row_id));
                    true
                } else {
                    false
                }
            }
            NavigationView::Projects => {
                if let Some(project_path) = self
                    .deployments
                    .iter()
                    .find(|deployment| deployment.id == row_id)
                    .map(|deployment| deployment.project_path.clone())
                {
                    self.selected_project = Some(project_path);
                    self.select_deployment(row_id.to_string());
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn pending_intents(&self) -> &[GuiActionIntent] {
        &self.pending_intents
    }

    pub fn last_status(&self) -> Option<&GuiStatus> {
        self.last_status.as_ref()
    }

    pub fn skill_risk_report(&self, skill_id: &SkillId) -> Option<&GuiRiskReport> {
        self.skill_risk_reports
            .iter()
            .find(|(id, _)| id == skill_id)
            .map(|(_, report)| report)
    }

    pub fn pending_remove_confirmation(&self) -> Option<&str> {
        self.pending_remove_confirmation.as_deref()
    }

    pub fn pending_remove_confirmation_message(&self) -> Option<&'static str> {
        self.pending_remove_confirmation
            .as_ref()
            .map(|_| DRIFT_REMOVE_CONFIRMATION_MESSAGE)
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

    pub fn request_deploy_selected_skill_to_default_agent(&mut self) -> Option<GuiActionIntent> {
        if !matches!(self.active_scope, GuiScope::Project(_)) {
            return None;
        }
        let agent_id = self
            .selected_agent
            .as_ref()
            .and_then(|selected| {
                self.agents
                    .iter()
                    .find(|agent| agent.id == *selected && agent.enabled)
            })
            .or_else(|| {
                self.agents
                    .iter()
                    .find(|agent| agent.enabled && !agent.project_skill_dirs.is_empty())
            })?
            .id
            .clone();
        self.request_deploy_selected_skill(agent_id)
    }

    pub fn request_deploy_selected_skill_to_target_agent(&mut self) -> Option<GuiActionIntent> {
        let target = self.project_deploy_target()?;
        self.push_intent(GuiActionIntent::DeploySkill {
            project_path: target.project_path,
            agent_id: target.agent_id,
            skill_id: target.skill_id,
        })
    }

    pub fn has_explicit_project_deploy_target(&self) -> bool {
        matches!(self.active_scope, GuiScope::Project(_))
            && self
                .agents
                .iter()
                .any(|agent| agent.enabled && !agent.project_skill_dirs.is_empty())
    }

    pub fn has_project_deploy_target(&self) -> bool {
        self.project_deploy_target().is_some()
    }

    pub fn project_deploy_target(&self) -> Option<ProjectDeployTarget> {
        let GuiScope::Project(project_path) = &self.active_scope else {
            return None;
        };
        let skill = self.selected_skill()?;
        let selected_agent = self.selected_agent.as_ref()?;
        let agent = self.agents.iter().find(|agent| {
            agent.id == *selected_agent && agent.enabled && !agent.project_skill_dirs.is_empty()
        })?;
        let project_dir = agent.project_skill_dirs.first()?.clone();

        Some(ProjectDeployTarget {
            project_path: project_path.clone(),
            skill_id: skill.id.clone(),
            skill_name: skill.name.clone(),
            agent_id: agent.id.clone(),
            agent_label: agent.label.clone(),
            target_path: project_path.join(project_dir).join(&skill.name),
        })
    }

    pub fn request_refresh_selected_project(&mut self) -> Option<GuiActionIntent> {
        let project_path = self.selected_project.clone()?;
        self.push_intent(GuiActionIntent::RefreshProject { project_path })
    }

    pub fn request_adopt_all_discovered_for_selected_project(&mut self) -> Option<GuiActionIntent> {
        let project = self.selected_project_summary()?;
        if project.discovered_unmanaged_count == 0 {
            return None;
        }
        if project.discovered_unmanaged_count <= project.pending_conflicts.len() {
            return None;
        }
        self.push_intent(GuiActionIntent::ProjectAdoptAll {
            project_path: project.path.clone(),
        })
    }

    pub fn request_import_selected_project_conflict_as_new(&mut self) -> Option<GuiActionIntent> {
        let project = self.selected_project_summary()?;
        let conflict = project.pending_conflicts.first()?;
        self.push_intent(GuiActionIntent::ProjectImportConflictAsNew {
            project_path: project.path.clone(),
            agent_id: conflict.agent_id.clone(),
            skill_name: conflict.skill_name.clone(),
        })
    }

    pub fn skip_selected_project_conflict(&mut self) -> Option<()> {
        let project_path = self.selected_project.clone()?;
        let project = self
            .project_summaries
            .iter_mut()
            .find(|summary| summary.path == project_path)?;
        if project.pending_conflicts.is_empty() {
            return None;
        }
        let conflict = project.pending_conflicts.remove(0);
        project.skipped_conflicts.push(conflict);
        project.discovered_unmanaged_count = project.discovered_unmanaged_count.saturating_sub(1);
        Some(())
    }

    pub fn begin_edit_selected_agent_path(&mut self) -> Option<()> {
        let agent = self.selected_agent()?.clone();
        self.agent_editor_draft = Some(AgentEditorDraft {
            mode: AgentEditorMode::EditPath {
                agent_id: agent.id.clone(),
            },
            id_text: agent.id.to_string(),
            label_text: agent.label,
            project_dir_text: agent
                .project_skill_dirs
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        });
        Some(())
    }

    pub fn begin_add_custom_agent(&mut self) {
        self.agent_editor_draft = Some(AgentEditorDraft {
            mode: AgentEditorMode::AddCustom,
            id_text: "custom".to_string(),
            label_text: "Custom Agent".to_string(),
            project_dir_text: ".custom/skills".to_string(),
        });
    }

    pub fn update_agent_editor_identity(&mut self, id: String, label: String) {
        if let Some(draft) = &mut self.agent_editor_draft {
            draft.id_text = id;
            draft.label_text = label;
        }
    }

    pub fn update_agent_editor_project_dir(&mut self, value: String) {
        if let Some(draft) = &mut self.agent_editor_draft {
            draft.project_dir_text = value;
        }
    }

    pub fn request_save_agent_editor(&mut self) -> Option<GuiActionIntent> {
        let draft = self.agent_editor_draft.clone()?;
        let project_skill_dirs = parse_project_dir_text(&draft.project_dir_text);
        let intent = match draft.mode {
            AgentEditorMode::AddCustom => GuiActionIntent::AddCustomAgent {
                agent_id: AgentId::new(draft.id_text.trim()),
                label: draft.label_text.trim().to_string(),
                project_skill_dir: project_skill_dirs.into_iter().next()?,
            },
            AgentEditorMode::EditPath { agent_id } => {
                GuiActionIntent::UpdateAgentProjectSkillDirs {
                    agent_id,
                    project_skill_dirs,
                }
            }
        };
        self.push_intent(intent)
    }

    pub fn cancel_agent_editor(&mut self) {
        self.agent_editor_draft = None;
    }

    pub fn request_enable_selected_deployment(&mut self) -> Option<GuiActionIntent> {
        let deployment = self.selected_deployment()?.clone();
        self.push_intent(GuiActionIntent::EnableDeployment {
            project_path: deployment.project_path,
            agent_id: deployment.agent_id,
            skill_name: deployment.skill_name,
        })
    }

    pub fn request_disable_selected_deployment(&mut self) -> Option<GuiActionIntent> {
        let deployment = self.selected_deployment()?.clone();
        self.push_intent(GuiActionIntent::DisableDeployment {
            project_path: deployment.project_path,
            agent_id: deployment.agent_id,
            skill_name: deployment.skill_name,
        })
    }

    pub fn request_remove_selected_deployment(&mut self, force: bool) -> Option<GuiActionIntent> {
        if !force {
            let status = self.selected_deployment_status()?;
            if status.drift {
                self.pending_remove_confirmation = Some(status.record.id.clone());
                return None;
            }
        }
        let deployment = self.selected_deployment()?.clone();
        self.pending_remove_confirmation = None;
        self.push_intent(GuiActionIntent::RemoveDeployment {
            project_path: deployment.project_path,
            agent_id: deployment.agent_id,
            skill_name: deployment.skill_name,
            force,
        })
    }

    pub fn confirm_pending_remove(&mut self) -> Option<GuiActionIntent> {
        let deployment_id = self.pending_remove_confirmation.clone()?;
        self.selected_deployment = Some(deployment_id);
        self.request_remove_selected_deployment(true)
    }

    pub fn request_redeploy_selected_deployment(&mut self) -> Option<GuiActionIntent> {
        self.request_redeploy_selected_deployment_with_options(false, false)
    }

    pub fn request_overwrite_selected_deployment(&mut self) -> Option<GuiActionIntent> {
        self.request_redeploy_selected_deployment_with_options(true, false)
    }

    pub fn request_promote_selected_deployment(&mut self) -> Option<GuiActionIntent> {
        self.request_redeploy_selected_deployment_with_options(false, true)
    }

    fn request_redeploy_selected_deployment_with_options(
        &mut self,
        overwrite: bool,
        promote: bool,
    ) -> Option<GuiActionIntent> {
        let deployment = self.selected_deployment()?.clone();
        self.push_intent(GuiActionIntent::RedeployDeployment {
            project_path: deployment.project_path,
            agent_id: deployment.agent_id,
            skill_name: deployment.skill_name,
            overwrite,
            promote,
        })
    }

    pub fn execute_next_intent(
        &mut self,
        controller: &GuiController,
    ) -> Result<Option<GuiActionIntent>> {
        if self.pending_intents.is_empty() {
            return Ok(None);
        }
        let intent = self.pending_intents.remove(0);
        let active_view = self.active_view;
        let active_scope = self.active_scope.clone();
        let selected_skill = self.selected_skill.clone();
        let selected_agent = self.selected_agent.clone();
        let selected_project = self.selected_project.clone();
        let selected_deployment = self.selected_deployment.clone();
        let pending_remove_confirmation = self.pending_remove_confirmation.clone();
        let pending_intents = self.pending_intents.clone();
        let skill_risk_reports = self.skill_risk_reports.clone();
        let selected_agent_after_save = match &intent {
            GuiActionIntent::AddCustomAgent { agent_id, .. }
            | GuiActionIntent::UpdateAgentProjectSkillDirs { agent_id, .. } => {
                Some(agent_id.clone())
            }
            _ => None,
        };
        let project_conflict_state = self
            .project_summaries
            .iter()
            .map(|summary| {
                (
                    summary.path.clone(),
                    summary.pending_conflicts.clone(),
                    summary.skipped_conflicts.clone(),
                )
            })
            .collect::<Vec<_>>();
        let outcome = match controller.execute(&intent) {
            Ok(outcome) => outcome,
            Err(error) => {
                self.last_status = Some(GuiStatus {
                    kind: GuiStatusKind::Error,
                    message: self.intent_error_message(&intent, &error),
                });
                return Err(error);
            }
        };
        let success_message = self.intent_success_message(&intent, &outcome);
        *self = Self::load(controller.paths())?;
        for (path, pending_conflicts, skipped_conflicts) in project_conflict_state {
            if let Some(summary) = self
                .project_summaries
                .iter_mut()
                .find(|summary| summary.path == path)
            {
                summary.pending_conflicts = pending_conflicts;
                summary.skipped_conflicts = skipped_conflicts;
            }
        }
        self.active_view = active_view;
        self.active_scope = active_scope;
        self.selected_skill = selected_skill.filter(|selected| {
            self.skills
                .iter()
                .any(|skill| skill.id.as_str() == selected.as_str())
        });
        self.selected_agent = selected_agent_after_save
            .or(selected_agent)
            .filter(|selected| {
                self.agents
                    .iter()
                    .any(|agent| agent.id.as_str() == selected.as_str())
            });
        self.selected_project = selected_project.or_else(|| match &self.active_scope {
            GuiScope::GlobalInventory => None,
            GuiScope::Project(path) => Some(path.clone()),
        });
        self.selected_deployment = selected_deployment.filter(|selected| {
            self.deployments
                .iter()
                .any(|deployment| deployment.id == *selected)
        });
        self.pending_remove_confirmation = pending_remove_confirmation.filter(|pending| {
            self.deployments
                .iter()
                .any(|deployment| deployment.id == *pending)
        });
        self.pending_intents = pending_intents;
        self.skill_risk_reports = skill_risk_reports
            .into_iter()
            .filter(|(skill_id, report)| {
                self.skills
                    .iter()
                    .any(|skill| skill.id == *skill_id && skill.content_hash == report.scanned_hash)
            })
            .collect();
        self.agent_editor_draft = None;
        self.last_status = Some(GuiStatus {
            kind: GuiStatusKind::Success,
            message: success_message,
        });
        self.apply_controller_outcome(outcome);
        Ok(Some(intent))
    }

    fn intent_success_message(
        &self,
        intent: &GuiActionIntent,
        outcome: &GuiControllerOutcome,
    ) -> String {
        match intent {
            GuiActionIntent::ScanSkill { skill_id } => {
                let skill_name = self
                    .skills
                    .iter()
                    .find(|skill| skill.id == *skill_id)
                    .map(|skill| skill.name.as_str())
                    .unwrap_or_else(|| skill_id.as_str());
                let summary = match outcome {
                    GuiControllerOutcome::SkillScan { findings, .. } => GuiRiskReport {
                        scanned_hash: String::new(),
                        findings: findings.clone(),
                    }
                    .summary_label(),
                    _ => "No findings".to_string(),
                };
                format!("Scanned {skill_name}: {summary}.")
            }
            GuiActionIntent::UninstallSkill { skill_id } => {
                let skill_name = self
                    .skills
                    .iter()
                    .find(|skill| skill.id == *skill_id)
                    .map(|skill| skill.name.as_str())
                    .unwrap_or_else(|| skill_id.as_str());
                format!("Uninstalled {skill_name} from Global Inventory.")
            }
            GuiActionIntent::DeploySkill {
                project_path,
                agent_id,
                skill_id,
            } => {
                let skill_name = self
                    .skills
                    .iter()
                    .find(|skill| skill.id == *skill_id)
                    .map(|skill| skill.name.as_str())
                    .unwrap_or_else(|| skill_id.as_str());
                format!(
                    "Deployed {skill_name} to {} for {}.",
                    self.agent_label(agent_id),
                    project_label(project_path)
                )
            }
            GuiActionIntent::EnableDeployment {
                project_path,
                agent_id,
                skill_name,
            } => format!(
                "Enabled {skill_name} for {} in {}.",
                self.agent_label(agent_id),
                project_label(project_path)
            ),
            GuiActionIntent::DisableDeployment {
                project_path,
                agent_id,
                skill_name,
            } => format!(
                "Disabled {skill_name} for {} in {}.",
                self.agent_label(agent_id),
                project_label(project_path)
            ),
            GuiActionIntent::RemoveDeployment {
                project_path,
                agent_id,
                skill_name,
                ..
            } => format!(
                "Removed {skill_name} from {} for {}.",
                self.agent_label(agent_id),
                project_label(project_path)
            ),
            GuiActionIntent::RedeployDeployment {
                project_path,
                agent_id,
                skill_name,
                overwrite,
                promote,
            } => {
                let verb = if *promote {
                    "Promoted"
                } else if *overwrite {
                    "Overwrote"
                } else {
                    "Redeployed"
                };
                format!(
                    "{verb} {skill_name} for {} in {}.",
                    self.agent_label(agent_id),
                    project_label(project_path)
                )
            }
            GuiActionIntent::RefreshProject { project_path } => {
                format!("Refreshed {}.", project_label(project_path))
            }
            GuiActionIntent::ProjectAdoptAll { project_path } => {
                format!(
                    "Adopted discovered Skills for {}.",
                    project_label(project_path)
                )
            }
            GuiActionIntent::ProjectImportConflictAsNew {
                project_path,
                skill_name,
                ..
            } => format!(
                "Imported {skill_name} as a new managed Skill for {}.",
                project_label(project_path)
            ),
            GuiActionIntent::OpenProject { project_path } => {
                format!("Selected {}.", project_label(project_path))
            }
            GuiActionIntent::UpdateAgentProjectSkillDirs { agent_id, .. } => {
                format!(
                    "Updated {} project Skill directories.",
                    self.agent_label(agent_id)
                )
            }
            GuiActionIntent::AddCustomAgent { label, .. } => {
                format!("Added custom Agent {label}.")
            }
        }
    }

    fn intent_error_message(&self, intent: &GuiActionIntent, error: &SkillKitsError) -> String {
        match error {
            SkillKitsError::DeployConflict { .. } => {
                "Deploy conflict. The target already exists; adopt it, remove it, or choose another Skill name.".to_string()
            }
            SkillKitsError::AdoptionConflict { name } => {
                format!("Adoption conflict for {name}. Import it as new or skip it.")
            }
            SkillKitsError::DeploymentDrift { .. } => {
                "Redeploy blocked because the project copy has local changes. Keep it, overwrite from managed, or promote it to managed.".to_string()
            }
            SkillKitsError::UnsafeRemoveRequiresForce { .. } => {
                "Remove blocked because the project copy has local changes. Confirm Remove to delete this deployed Skill only.".to_string()
            }
            SkillKitsError::MissingManagedSource { .. } => {
                "Missing managed source. Promote the project copy to managed or remove it from the project.".to_string()
            }
            SkillKitsError::InvalidToggleState { .. } => {
                "Invalid toggle state. Fix the SKILL.md / SKILL.md.disabled pair before continuing."
                    .to_string()
            }
            _ => format!("{} failed: {error}", action_label(intent)),
        }
    }

    fn agent_label(&self, agent_id: &AgentId) -> String {
        self.agents
            .iter()
            .find(|agent| agent.id == *agent_id)
            .map(|agent| agent.label.clone())
            .unwrap_or_else(|| agent_id.to_string())
    }

    fn apply_controller_outcome(&mut self, outcome: GuiControllerOutcome) {
        match outcome {
            GuiControllerOutcome::None => {}
            GuiControllerOutcome::SkillScan {
                skill_id,
                scanned_hash,
                findings,
            } => {
                self.skill_risk_reports
                    .retain(|(existing_id, _)| existing_id != &skill_id);
                self.skill_risk_reports.push((
                    skill_id,
                    GuiRiskReport {
                        scanned_hash,
                        findings,
                    },
                ));
            }
            GuiControllerOutcome::ProjectScan {
                project_path,
                discovered_unmanaged_count,
                adopt_result,
                pending_conflicts,
                preserve_existing_conflicts,
            } => {
                let deployment_count = self
                    .deployments
                    .iter()
                    .filter(|deployment| deployment.project_path == project_path)
                    .count();
                if let Some(summary) = self
                    .project_summaries
                    .iter_mut()
                    .find(|summary| summary.path == project_path)
                {
                    let skipped_conflicts = pending_conflicts_intersection(
                        summary.skipped_conflicts.clone(),
                        &pending_conflicts,
                    );
                    let pending_conflicts = if preserve_existing_conflicts {
                        pending_conflicts_intersection(
                            summary.pending_conflicts.clone(),
                            &pending_conflicts,
                        )
                    } else {
                        pending_conflicts
                    };
                    let pending_conflicts = if discovered_unmanaged_count == 0 {
                        Vec::new()
                    } else {
                        pending_conflicts_minus_skipped(pending_conflicts, &skipped_conflicts)
                    };
                    summary.deployment_count = deployment_count;
                    summary.discovered_unmanaged_count =
                        discovered_unmanaged_count.saturating_sub(skipped_conflicts.len());
                    summary.last_adopt_all_result = adopt_result;
                    summary.pending_conflicts = pending_conflicts;
                    summary.skipped_conflicts = skipped_conflicts;
                } else {
                    self.project_summaries.push(ProjectSummary {
                        name: project_path
                            .file_name()
                            .map(ToOwned::to_owned)
                            .unwrap_or_else(|| project_path.to_string()),
                        path: project_path,
                        deployment_count,
                        discovered_unmanaged_count,
                        last_adopt_all_result: adopt_result,
                        pending_conflicts,
                        skipped_conflicts: Vec::new(),
                    });
                }
            }
        }
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

    pub fn agent_editor_draft(&self) -> Option<&AgentEditorDraft> {
        self.agent_editor_draft.as_ref()
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
            pending_remove_confirmation: None,
            pending_intents: Vec::new(),
            last_status: None,
            skill_risk_reports: Vec::new(),
            agent_editor_draft: None,
        }
    }
}

fn action_label(intent: &GuiActionIntent) -> &'static str {
    match intent {
        GuiActionIntent::ScanSkill { .. } => "Scan",
        GuiActionIntent::UninstallSkill { .. } => "Uninstall",
        GuiActionIntent::DeploySkill { .. } => "Deploy",
        GuiActionIntent::EnableDeployment { .. } => "Enable",
        GuiActionIntent::DisableDeployment { .. } => "Disable",
        GuiActionIntent::RemoveDeployment { .. } => "Remove",
        GuiActionIntent::RedeployDeployment { .. } => "Redeploy",
        GuiActionIntent::RefreshProject { .. } => "Refresh",
        GuiActionIntent::ProjectAdoptAll { .. } => "Adopt all",
        GuiActionIntent::ProjectImportConflictAsNew { .. } => "Import as new",
        GuiActionIntent::OpenProject { .. } => "Open project",
        GuiActionIntent::UpdateAgentProjectSkillDirs { .. } => "Update Agent",
        GuiActionIntent::AddCustomAgent { .. } => "Add custom Agent",
    }
}

fn parse_project_dir_text(value: &str) -> Vec<Utf8PathBuf> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(Utf8PathBuf::from)
        .collect()
}

fn project_label(project_path: &camino::Utf8Path) -> String {
    project_path
        .file_name()
        .map(ToString::to_string)
        .unwrap_or_else(|| project_path.to_string())
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
