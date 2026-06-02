use crate::core::{
    adopt::{
        global_agent_adopt_resilient, import_managed_copy, project_adopt, project_adopt_all,
        project_adopt_conflict_as_new, GlobalAgentAdoptRequest, ImportManagedCopyRequest,
        ProjectAdoptRequest,
    },
    agent_space::{
        disable_skill_instance, enable_skill_instance, scan_agent_spaces, SkillInstance,
        SkillInstanceRequest, SkillInstanceScope, SkillInstanceSourceKind,
    },
    agents::{
        add_custom_agent_config, remove_custom_agent_config, reset_agent_project_skill_dirs,
        update_agent_project_skill_dirs, AgentConfig, AgentKind,
    },
    config::{read_config, RecentProject},
    ids::{AgentId, SkillId},
    install::{install_local_skill, uninstall_skill, InstallLocalRequest, UninstallSkillRequest},
    onboarding::{
        project_onboarding_scan, record_recent_project, DiscoveredProjectSkill,
        ProjectOnboardingScanRequest,
    },
    paths::AppPaths,
    project::{
        deploy_project_skill, disable_project_skill, enable_project_skill,
        project_deployment_status, redeploy_project_skill, remove_project_skill,
        resolve_project_scope, ProjectDeployRequest, ProjectRedeployRequest, ProjectRemoveRequest,
        ProjectSkillRequest,
    },
    registry::{
        read_deployments_registry, read_skills_registry, DeploymentRecord, DeploymentStatus,
        ManagedSkill, SkillSource, ToggleState,
    },
    scan::{scan_skill_dir, RiskFinding, RiskSeverity},
    status::{global_status, HealthState},
    Result, SkillKitsError,
};
use camino::Utf8PathBuf;
use egui::Color32;

pub const DRIFT_REMOVE_CONFIRMATION_MESSAGE: &str =
    "This project copy has local changes. Removing it deletes only this deployed Skill, not the Agent skill root.";
pub const GLOBAL_UNINSTALL_CONFIRMATION_MESSAGE: &str =
    "Uninstall removes this managed copy from Managed Inventory. Agent Space copies are not deleted.";
pub const SKILL_INSTANCE_DISABLE_CONFIRMATION_MESSAGE: &str =
    "Disable changes SKILL.md to SKILL.md.disabled in the Agent Space. It does not delete the Skill directory.";

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
            Self::Skills => "Skill",
            Self::Agents => "Agent",
            Self::Projects => "Project",
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
    InstallLocalSkill {
        source_path: Utf8PathBuf,
    },
    ScanAgentSpaces,
    ImportAllManagedCopies,
    ImportManagedCopy {
        instance_id: String,
    },
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
    EnableSkillInstance {
        instance_id: String,
    },
    DisableSkillInstance {
        instance_id: String,
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
    ProjectAdoptSelected {
        project_path: Utf8PathBuf,
        agent_id: AgentId,
        skill_name: String,
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
    ResetAgentProjectSkillDirs {
        agent_id: AgentId,
    },
    RemoveCustomAgent {
        agent_id: AgentId,
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
    home_dir: Option<Utf8PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuiControllerOutcome {
    None,
    AgentSpacesScanned {
        instances: usize,
    },
    AgentSkillsAdopted {
        imported: usize,
        conflicts: usize,
        failures: usize,
    },
    SkillInstalled {
        skill_id: SkillId,
        scanned_hash: String,
        findings: Vec<RiskFinding>,
    },
    SkillScan {
        skill_id: SkillId,
        scanned_hash: String,
        findings: Vec<RiskFinding>,
    },
    ProjectScan {
        project_path: Utf8PathBuf,
        discovered_unmanaged_count: usize,
        discovered_skills: Vec<ProjectDiscoveredSkill>,
        adopt_result: Option<ProjectAdoptAllSummary>,
        pending_conflicts: Vec<ProjectConflict>,
        preserve_existing_conflicts: bool,
    },
    ProjectOpened {
        project_path: Utf8PathBuf,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectDiscoveredSkill {
    pub agent_id: AgentId,
    pub name: String,
    pub path: Utf8PathBuf,
    pub toggle: ToggleState,
}

impl From<DiscoveredProjectSkill> for ProjectDiscoveredSkill {
    fn from(skill: DiscoveredProjectSkill) -> Self {
        Self {
            agent_id: skill.agent_id,
            name: skill.name,
            path: skill.path,
            toggle: skill.toggle,
        }
    }
}

impl ProjectDiscoveredSkill {
    fn conflict_key(&self) -> ProjectConflict {
        ProjectConflict {
            agent_id: self.agent_id.clone(),
            skill_name: self.name.clone(),
        }
    }
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

fn default_home_dir() -> Result<Utf8PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "home directory"))?;
    Utf8PathBuf::from_path_buf(home).map_err(|path| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("home path is not UTF-8: {}", path.display()),
        )
        .into()
    })
}

impl GuiController {
    pub fn new(paths: AppPaths) -> Self {
        Self {
            paths,
            home_dir: None,
        }
    }

    pub fn with_home_dir(paths: AppPaths, home_dir: Utf8PathBuf) -> Self {
        Self {
            paths,
            home_dir: Some(home_dir),
        }
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    fn load_model(&self) -> Result<GuiModel> {
        match &self.home_dir {
            Some(home_dir) => GuiModel::load_with_home_dir(&self.paths, home_dir.clone()),
            None => GuiModel::load(&self.paths),
        }
    }

    pub fn execute(&self, intent: &GuiActionIntent) -> Result<GuiControllerOutcome> {
        let outcome = match intent {
            GuiActionIntent::InstallLocalSkill { source_path } => {
                let result = install_local_skill(
                    InstallLocalRequest {
                        source_path: source_path.as_path(),
                    },
                    &self.paths,
                )?;
                GuiControllerOutcome::SkillInstalled {
                    skill_id: result.skill.id,
                    scanned_hash: result.skill.content_hash,
                    findings: result.risk_findings,
                }
            }
            GuiActionIntent::ScanAgentSpaces => {
                let home = self.home_dir.clone().map_or_else(default_home_dir, Ok)?;
                let instances = scan_agent_spaces(&self.paths, &home)?.len();
                GuiControllerOutcome::AgentSpacesScanned { instances }
            }
            GuiActionIntent::ImportAllManagedCopies => {
                let home = self.home_dir.clone().map_or_else(default_home_dir, Ok)?;
                let config = read_config(&self.paths)?;
                let mut imported = 0;
                let mut conflicts = 0;
                let mut failures = 0;
                for agent in config.agents.iter().filter(|agent| agent.enabled) {
                    match global_agent_adopt_resilient(GlobalAgentAdoptRequest {
                        app_paths: &self.paths,
                        agent_id: &agent.id,
                        home_dir: &home,
                    }) {
                        Ok(report) => {
                            imported += report.imported;
                            conflicts += report.conflicts;
                            failures += report.failures;
                        }
                        Err(_) => {
                            failures += 1;
                        }
                    }
                }
                GuiControllerOutcome::AgentSkillsAdopted {
                    imported,
                    conflicts,
                    failures,
                }
            }
            GuiActionIntent::ImportManagedCopy { instance_id } => {
                let home = self.home_dir.clone().map_or_else(default_home_dir, Ok)?;
                let instance = scan_agent_spaces(&self.paths, &home)?
                    .into_iter()
                    .find(|instance| instance.id == *instance_id)
                    .ok_or_else(|| SkillKitsError::SkillNotFound {
                        query: instance_id.clone(),
                    })?;
                let skill = match &instance.scope {
                    SkillInstanceScope::Global => import_managed_copy(ImportManagedCopyRequest {
                        app_paths: &self.paths,
                        agent_id: &instance.agent_id,
                        source_path: &instance.skill_dir,
                    })?,
                    SkillInstanceScope::Project { path, .. } => {
                        project_adopt(ProjectAdoptRequest {
                            app_paths: &self.paths,
                            project_path: path,
                            agent_id: &instance.agent_id,
                            skill_name: instance
                                .skill_dir
                                .file_name()
                                .unwrap_or(instance.name.as_str()),
                        })?;
                        read_skills_registry(&self.paths)?
                            .skills
                            .into_iter()
                            .find(|skill| {
                                matches!(
                                    &skill.source,
                                    SkillSource::ProjectAdopt { source_path, .. }
                                        if source_path == &instance.skill_dir
                                )
                            })
                            .ok_or_else(|| SkillKitsError::SkillNotFound {
                                query: instance.name.clone(),
                            })?
                    }
                };
                let findings = scan_skill_dir(&skill.managed_path)?;
                GuiControllerOutcome::SkillInstalled {
                    skill_id: skill.id,
                    scanned_hash: skill.content_hash,
                    findings,
                }
            }
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
            GuiActionIntent::EnableSkillInstance { instance_id } => {
                let home = self.home_dir.clone().map_or_else(default_home_dir, Ok)?;
                enable_skill_instance(SkillInstanceRequest {
                    app_paths: &self.paths,
                    home_dir: &home,
                    instance_id,
                })?;
                GuiControllerOutcome::None
            }
            GuiActionIntent::DisableSkillInstance { instance_id } => {
                let home = self.home_dir.clone().map_or_else(default_home_dir, Ok)?;
                disable_skill_instance(SkillInstanceRequest {
                    app_paths: &self.paths,
                    home_dir: &home,
                    instance_id,
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
                    discovered_skills: report
                        .discovered
                        .into_iter()
                        .map(ProjectDiscoveredSkill::from)
                        .collect(),
                    adopt_result: None,
                    pending_conflicts,
                    preserve_existing_conflicts: true,
                }
            }
            GuiActionIntent::ProjectAdoptSelected {
                project_path,
                agent_id,
                skill_name,
            } => {
                project_adopt(ProjectAdoptRequest {
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
                    discovered_skills: report
                        .discovered
                        .into_iter()
                        .map(ProjectDiscoveredSkill::from)
                        .collect(),
                    adopt_result: None,
                    pending_conflicts,
                    preserve_existing_conflicts: false,
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
                    discovered_skills: report
                        .discovered
                        .into_iter()
                        .map(ProjectDiscoveredSkill::from)
                        .collect(),
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
                    discovered_skills: report
                        .discovered
                        .into_iter()
                        .map(ProjectDiscoveredSkill::from)
                        .collect(),
                    adopt_result: None,
                    pending_conflicts,
                    preserve_existing_conflicts: false,
                }
            }
            GuiActionIntent::OpenProject { project_path } => {
                let project = resolve_project_scope(Some(project_path))?;
                record_recent_project(&self.paths, &project.path)?;
                GuiControllerOutcome::ProjectOpened {
                    project_path: project.path,
                }
            }
            GuiActionIntent::UpdateAgentProjectSkillDirs {
                agent_id,
                project_skill_dirs,
            } => {
                update_agent_project_skill_dirs(&self.paths, agent_id, project_skill_dirs.clone())?;
                GuiControllerOutcome::None
            }
            GuiActionIntent::ResetAgentProjectSkillDirs { agent_id } => {
                reset_agent_project_skill_dirs(&self.paths, agent_id)?;
                GuiControllerOutcome::None
            }
            GuiActionIntent::RemoveCustomAgent { agent_id } => {
                remove_custom_agent_config(&self.paths, agent_id)?;
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
    pub agent_space_instance_count: usize,
    pub managed_skill_count: usize,
    pub agent_count: usize,
    pub enabled_agent_count: usize,
    pub recent_project_count: usize,
    pub deployment_count: usize,
    pub outdated_deployment_count: usize,
    pub drifted_deployment_count: usize,
    pub invalid_toggle_count: usize,
    pub missing_managed_source_count: usize,
    pub registry_health: HealthState,
    pub lock_health: HealthState,
    pub cache_health: HealthState,
    pub risk_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSummary {
    pub name: String,
    pub path: Utf8PathBuf,
    pub deployment_count: usize,
    pub onboarding_scanned: bool,
    pub discovered_unmanaged_count: usize,
    pub discovered_skills: Vec<ProjectDiscoveredSkill>,
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InstallLocalSkillDraft {
    pub path_text: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OpenProjectDraft {
    pub path_text: String,
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
    pub skill_instances: Vec<SkillInstance>,
    pub skills: Vec<ManagedSkill>,
    pub agents: Vec<AgentConfig>,
    pub recent_projects: Vec<RecentProject>,
    pub project_summaries: Vec<ProjectSummary>,
    pub deployments: Vec<DeploymentRecord>,
    pub deployment_statuses: Vec<DeploymentStatus>,
    selected_skill: Option<SkillId>,
    selected_skill_instance: Option<String>,
    selected_agent: Option<AgentId>,
    selected_project: Option<Utf8PathBuf>,
    selected_deployment: Option<String>,
    selected_discovered_project_skill: Option<ProjectConflict>,
    pending_uninstall_confirmation: Option<SkillId>,
    pending_disable_skill_instance_confirmation: Option<String>,
    pending_remove_confirmation: Option<String>,
    skill_agent_filter: Option<AgentId>,
    skill_scope_filter: Option<String>,
    skill_status_filter: Option<String>,
    pending_intents: Vec<GuiActionIntent>,
    last_status: Option<GuiStatus>,
    skill_risk_reports: Vec<(SkillId, GuiRiskReport)>,
    agent_editor_draft: Option<AgentEditorDraft>,
    install_local_skill_draft: Option<InstallLocalSkillDraft>,
    open_project_draft: Option<OpenProjectDraft>,
}

impl GuiModel {
    pub fn load(paths: &AppPaths) -> Result<Self> {
        let home = paths
            .data_root
            .file_name()
            .filter(|name| *name == ".skill-kits")
            .and_then(|_| paths.data_root.parent().map(ToOwned::to_owned))
            .map_or_else(default_home_dir, Ok)?;
        Self::load_with_home_dir(paths, home)
    }

    pub fn load_with_home_dir(paths: &AppPaths, home_dir: Utf8PathBuf) -> Result<Self> {
        let config = read_config(paths)?;
        let skill_instances = scan_agent_spaces(paths, &home_dir)?;
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
                    onboarding_scanned: false,
                    discovered_unmanaged_count: 0,
                    discovered_skills: Vec::new(),
                    last_adopt_all_result: None,
                    pending_conflicts: Vec::new(),
                    skipped_conflicts: Vec::new(),
                }
            })
            .collect::<Vec<_>>();
        let status = global_status(paths)?;
        let dashboard = DashboardSummary {
            agent_space_instance_count: skill_instances.len(),
            managed_skill_count: status.managed_skill_count,
            agent_count: status.agent_count,
            enabled_agent_count: status.enabled_agent_count,
            recent_project_count: status.recent_project_count,
            deployment_count: deployments.len(),
            outdated_deployment_count: deployment_statuses
                .iter()
                .filter(|status| status.outdated)
                .count(),
            drifted_deployment_count: deployment_statuses
                .iter()
                .filter(|status| status.drift)
                .count(),
            invalid_toggle_count: deployment_statuses
                .iter()
                .filter(|status| {
                    matches!(
                        status.toggle,
                        crate::core::registry::ToggleState::InvalidBothPresent
                            | crate::core::registry::ToggleState::InvalidBothMissing
                    )
                })
                .count(),
            missing_managed_source_count: deployment_statuses
                .iter()
                .filter(|status| status.missing_managed_source)
                .count(),
            registry_health: status.registry_health,
            lock_health: status.lock_health,
            cache_health: status.cache_health,
            risk_count: status.risk_count,
        };
        let selected_project = config
            .recent_projects
            .first()
            .map(|project| project.path.clone());

        Ok(Self {
            active_view: NavigationView::Dashboard,
            active_scope: GuiScope::GlobalInventory,
            dashboard,
            skill_instances,
            skills,
            agents: config.agents,
            recent_projects: config.recent_projects,
            project_summaries,
            deployments,
            deployment_statuses,
            selected_skill: None,
            selected_skill_instance: None,
            selected_agent: None,
            selected_project,
            selected_deployment: None,
            selected_discovered_project_skill: None,
            pending_uninstall_confirmation: None,
            pending_disable_skill_instance_confirmation: None,
            pending_remove_confirmation: None,
            skill_agent_filter: None,
            skill_scope_filter: None,
            skill_status_filter: None,
            pending_intents: Vec::new(),
            last_status: None,
            skill_risk_reports: Vec::new(),
            agent_editor_draft: None,
            install_local_skill_draft: None,
            open_project_draft: None,
        })
    }

    pub fn navigate(&mut self, view: NavigationView) {
        self.active_view = view;
    }

    pub fn select_scope(&mut self, scope: GuiScope) {
        if let GuiScope::Project(path) = &scope {
            self.selected_project = Some(path.clone());
            self.selected_deployment = None;
            self.selected_discovered_project_skill = None;
            self.pending_remove_confirmation = None;
        }
        self.active_scope = scope;
    }

    pub fn select_skill(&mut self, skill_id: SkillId) {
        if self
            .pending_uninstall_confirmation
            .as_ref()
            .is_some_and(|pending| pending != &skill_id)
        {
            self.pending_uninstall_confirmation = None;
        }
        self.selected_skill = Some(skill_id);
    }

    pub fn select_managed_skill(&mut self, skill_id: SkillId) {
        self.select_skill(skill_id);
    }

    pub fn select_agent(&mut self, agent_id: AgentId) {
        self.selected_agent = Some(agent_id);
    }

    pub fn select_project(&mut self, project_path: Utf8PathBuf) {
        self.selected_project = Some(project_path);
        self.selected_deployment = None;
        self.selected_discovered_project_skill = None;
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
        self.selected_discovered_project_skill = None;
    }

    pub fn select_render_row(&mut self, row_id: &str) -> bool {
        match self.active_view {
            NavigationView::Dashboard => false,
            NavigationView::Skills => {
                if self
                    .skill_instances
                    .iter()
                    .any(|instance| instance.id == row_id)
                {
                    self.select_skill_instance(row_id.to_string());
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
                if let Some(discovered) = parse_discovered_project_row_id(row_id) {
                    if self.selected_project_summary().is_some_and(|summary| {
                        summary.discovered_skills.iter().any(|skill| {
                            skill.agent_id == discovered.agent_id
                                && skill.name == discovered.skill_name
                        })
                    }) {
                        self.selected_discovered_project_skill = Some(discovered);
                        self.selected_deployment = None;
                        self.pending_remove_confirmation = None;
                        return true;
                    }
                    return false;
                }
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

    pub fn append_pending_intents(&mut self, intents: Vec<GuiActionIntent>) {
        self.pending_intents.extend(intents);
    }

    pub fn take_pending_intents(&mut self) -> Vec<GuiActionIntent> {
        std::mem::take(&mut self.pending_intents)
    }

    pub fn pending_action_status_label(&self) -> String {
        let Some(intent) = self.pending_intents.first() else {
            return "Idle".to_string();
        };
        format!(
            "Next: {} ({} queued)",
            action_label(intent),
            self.pending_intents.len()
        )
    }

    pub fn next_action_label(&self) -> Option<&'static str> {
        self.pending_intents.first().map(action_label)
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

    pub fn pending_uninstall_confirmation(&self) -> Option<&str> {
        self.pending_uninstall_confirmation
            .as_ref()
            .map(SkillId::as_str)
    }

    pub fn pending_uninstall_confirmation_message(&self) -> Option<&'static str> {
        self.pending_uninstall_confirmation
            .as_ref()
            .map(|_| GLOBAL_UNINSTALL_CONFIRMATION_MESSAGE)
    }

    pub fn pending_disable_skill_instance_confirmation(&self) -> Option<&str> {
        self.pending_disable_skill_instance_confirmation.as_deref()
    }

    pub fn pending_disable_skill_instance_confirmation_message(&self) -> Option<&'static str> {
        self.pending_disable_skill_instance_confirmation
            .as_ref()
            .map(|_| SKILL_INSTANCE_DISABLE_CONFIRMATION_MESSAGE)
    }

    pub fn pending_remove_confirmation_message(&self) -> Option<&'static str> {
        self.pending_remove_confirmation
            .as_ref()
            .map(|_| DRIFT_REMOVE_CONFIRMATION_MESSAGE)
    }

    pub fn set_skill_agent_filter(&mut self, agent_id: Option<AgentId>) {
        self.skill_agent_filter = agent_id;
    }

    pub fn skill_agent_filter(&self) -> Option<&AgentId> {
        self.skill_agent_filter.as_ref()
    }

    pub fn set_skill_scope_filter(&mut self, scope: Option<String>) {
        self.skill_scope_filter = scope;
    }

    pub fn skill_scope_filter(&self) -> Option<&str> {
        self.skill_scope_filter.as_deref()
    }

    pub fn set_skill_status_filter(&mut self, status: Option<String>) {
        self.skill_status_filter = status;
    }

    pub fn skill_status_filter(&self) -> Option<&str> {
        self.skill_status_filter.as_deref()
    }

    pub fn skill_agent_filter_options(&self) -> Vec<(AgentId, String)> {
        let mut options = self
            .skill_instances
            .iter()
            .map(|instance| {
                let label = self
                    .agents
                    .iter()
                    .find(|agent| agent.id == instance.agent_id)
                    .map(|agent| agent.label.clone())
                    .unwrap_or_else(|| instance.agent_id.to_string());
                (instance.agent_id.clone(), label)
            })
            .collect::<Vec<_>>();
        options.sort_by(|left, right| left.1.cmp(&right.1).then(left.0.cmp(&right.0)));
        options.dedup_by(|left, right| left.0 == right.0);
        options
    }

    pub fn skill_scope_filter_options(&self) -> Vec<String> {
        let mut options = self
            .skill_instances
            .iter()
            .map(|instance| skill_instance_scope_filter_label(&instance.scope))
            .collect::<Vec<_>>();
        options.sort();
        options.dedup();
        options
    }

    pub fn skill_status_filter_options(&self) -> Vec<&'static str> {
        const ORDER: [&str; 5] = ["Enabled", "Disabled", "Invalid", "Missing", "Read-only"];
        ORDER
            .into_iter()
            .filter(|status| {
                self.skill_instances
                    .iter()
                    .any(|instance| skill_instance_status_label(instance) == *status)
            })
            .collect()
    }

    pub fn request_scan_selected_skill(&mut self) -> Option<GuiActionIntent> {
        let skill_id = self.selected_skill.clone()?;
        self.push_intent(GuiActionIntent::ScanSkill { skill_id })
    }

    pub fn begin_install_local_skill(&mut self) {
        self.install_local_skill_draft = Some(InstallLocalSkillDraft::default());
    }

    pub fn update_install_local_skill_path(&mut self, value: String) {
        if let Some(draft) = &mut self.install_local_skill_draft {
            draft.path_text = value;
        }
    }

    pub fn request_save_install_local_skill(&mut self) -> Option<GuiActionIntent> {
        let source_path = self.install_local_skill_draft.as_ref()?.path_text.trim();
        if source_path.is_empty() {
            return None;
        }
        self.push_intent(GuiActionIntent::InstallLocalSkill {
            source_path: source_path.into(),
        })
    }

    pub fn request_import_all_agent_skills_as_managed_copies(&mut self) -> Option<GuiActionIntent> {
        self.push_intent(GuiActionIntent::ImportAllManagedCopies)
    }

    pub fn request_scan_agent_spaces(&mut self) -> Option<GuiActionIntent> {
        self.push_intent(GuiActionIntent::ScanAgentSpaces)
    }

    pub fn request_import_selected_skill_instance_as_managed_copy(
        &mut self,
    ) -> Option<GuiActionIntent> {
        let instance = self.selected_skill_instance()?.clone();
        if !matches!(
            instance.source_kind,
            SkillInstanceSourceKind::AgentSpace | SkillInstanceSourceKind::ProjectDeployment
        ) {
            return None;
        }
        if instance.stable_id.is_some() {
            return None;
        }
        if matches!(
            instance.toggle_state,
            ToggleState::InvalidBothPresent | ToggleState::InvalidBothMissing
        ) {
            return None;
        }
        self.push_intent(GuiActionIntent::ImportManagedCopy {
            instance_id: instance.id,
        })
    }

    pub fn cancel_install_local_skill(&mut self) {
        self.install_local_skill_draft = None;
    }

    pub fn begin_open_project(&mut self) {
        self.open_project_draft = Some(OpenProjectDraft::default());
    }

    pub fn update_open_project_path(&mut self, value: String) {
        if let Some(draft) = &mut self.open_project_draft {
            draft.path_text = value;
        }
    }

    pub fn request_save_open_project(&mut self) -> Option<GuiActionIntent> {
        let project_path = self.open_project_draft.as_ref()?.path_text.trim();
        if project_path.is_empty() {
            return None;
        }
        let project = resolve_project_scope(Some(camino::Utf8Path::new(project_path))).ok()?;
        self.push_intent(GuiActionIntent::OpenProject {
            project_path: project.path,
        })
    }

    pub fn cancel_open_project(&mut self) {
        self.open_project_draft = None;
    }

    pub fn request_uninstall_selected_skill(&mut self, confirmed: bool) -> Option<GuiActionIntent> {
        let skill_id = self.selected_skill.clone()?;
        if !confirmed {
            self.pending_uninstall_confirmation = Some(skill_id);
            return None;
        }
        self.pending_uninstall_confirmation = None;
        self.push_intent(GuiActionIntent::UninstallSkill { skill_id })
    }

    pub fn confirm_pending_uninstall(&mut self) -> Option<GuiActionIntent> {
        let skill_id = self.pending_uninstall_confirmation.clone()?;
        self.selected_skill = Some(skill_id);
        self.request_uninstall_selected_skill(true)
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

    pub fn request_adopt_selected_discovered_project_skill(&mut self) -> Option<GuiActionIntent> {
        let project = self.selected_project_summary()?;
        let selected = self.selected_discovered_project_skill.as_ref()?;
        let skill = project.discovered_skills.iter().find(|skill| {
            skill.agent_id == selected.agent_id && skill.name == selected.skill_name
        })?;
        if project.pending_conflicts.contains(&skill.conflict_key()) {
            return None;
        }
        self.push_intent(GuiActionIntent::ProjectAdoptSelected {
            project_path: project.path.clone(),
            agent_id: skill.agent_id.clone(),
            skill_name: skill.name.clone(),
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

    pub fn request_reset_selected_agent_project_dirs(&mut self) -> Option<GuiActionIntent> {
        let agent = self.selected_agent()?;
        if agent.kind != AgentKind::BuiltIn {
            return None;
        }
        self.push_intent(GuiActionIntent::ResetAgentProjectSkillDirs {
            agent_id: agent.id.clone(),
        })
    }

    pub fn request_remove_selected_custom_agent(&mut self) -> Option<GuiActionIntent> {
        let agent = self.selected_agent()?;
        if agent.kind != AgentKind::Custom {
            return None;
        }
        self.push_intent(GuiActionIntent::RemoveCustomAgent {
            agent_id: agent.id.clone(),
        })
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

    pub fn request_enable_selected_skill_instance(&mut self) -> Option<GuiActionIntent> {
        let instance = self.selected_skill_instance()?.clone();
        if !instance.writable || instance.toggle_state != ToggleState::Disabled {
            return None;
        }
        self.push_intent(GuiActionIntent::EnableSkillInstance {
            instance_id: instance.id,
        })
    }

    pub fn request_disable_selected_skill_instance(&mut self) -> Option<GuiActionIntent> {
        self.request_disable_selected_skill_instance_with_confirmation(true)
    }

    pub fn request_disable_selected_skill_instance_with_confirmation(
        &mut self,
        confirmed: bool,
    ) -> Option<GuiActionIntent> {
        let instance = self.selected_skill_instance()?.clone();
        if !instance.writable || instance.toggle_state != ToggleState::Enabled {
            return None;
        }
        if !confirmed {
            self.pending_disable_skill_instance_confirmation = Some(instance.id);
            return None;
        }
        self.pending_disable_skill_instance_confirmation = None;
        self.push_intent(GuiActionIntent::DisableSkillInstance {
            instance_id: instance.id,
        })
    }

    pub fn confirm_pending_disable_skill_instance(&mut self) -> Option<GuiActionIntent> {
        let instance_id = self.pending_disable_skill_instance_confirmation.clone()?;
        self.selected_skill_instance = Some(instance_id);
        self.request_disable_selected_skill_instance_with_confirmation(true)
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
        let selected_skill_instance = self.selected_skill_instance.clone();
        let selected_agent = self.selected_agent.clone();
        let selected_project = self.selected_project.clone();
        let selected_deployment = self.selected_deployment.clone();
        let selected_discovered_project_skill = self.selected_discovered_project_skill.clone();
        let pending_uninstall_confirmation = self.pending_uninstall_confirmation.clone();
        let pending_disable_skill_instance_confirmation =
            self.pending_disable_skill_instance_confirmation.clone();
        let pending_remove_confirmation = self.pending_remove_confirmation.clone();
        let skill_agent_filter = self.skill_agent_filter.clone();
        let skill_scope_filter = self.skill_scope_filter.clone();
        let skill_status_filter = self.skill_status_filter.clone();
        let pending_intents = self.pending_intents.clone();
        let skill_risk_reports = self.skill_risk_reports.clone();
        let selected_agent_after_save = match &intent {
            GuiActionIntent::AddCustomAgent { agent_id, .. }
            | GuiActionIntent::UpdateAgentProjectSkillDirs { agent_id, .. }
            | GuiActionIntent::ResetAgentProjectSkillDirs { agent_id } => Some(agent_id.clone()),
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
                    summary.discovered_skills.clone(),
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
        let selected_skill_after_install = match &outcome {
            GuiControllerOutcome::SkillInstalled { skill_id, .. } => Some(skill_id.clone()),
            _ => None,
        };
        let success_message = self.intent_success_message(&intent, &outcome);
        *self = controller.load_model()?;
        for (path, pending_conflicts, skipped_conflicts, discovered_skills) in
            project_conflict_state
        {
            if let Some(summary) = self
                .project_summaries
                .iter_mut()
                .find(|summary| summary.path == path)
            {
                summary.pending_conflicts = pending_conflicts;
                summary.skipped_conflicts = skipped_conflicts;
                if !discovered_skills.is_empty() && summary.discovered_skills.is_empty() {
                    summary.discovered_skills = discovered_skills;
                }
            }
        }
        self.active_view = active_view;
        self.active_scope = active_scope;
        self.selected_skill = selected_skill_after_install
            .or(selected_skill)
            .filter(|selected| {
                self.skills
                    .iter()
                    .any(|skill| skill.id.as_str() == selected.as_str())
            });
        self.selected_skill_instance = selected_skill_instance.filter(|selected| {
            self.skill_instances
                .iter()
                .any(|instance| instance.id == *selected)
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
        self.selected_discovered_project_skill =
            selected_discovered_project_skill.filter(|selected| {
                self.selected_project_summary().is_some_and(|summary| {
                    summary.discovered_skills.iter().any(|skill| {
                        skill.agent_id == selected.agent_id && skill.name == selected.skill_name
                    })
                })
            });
        self.pending_uninstall_confirmation = pending_uninstall_confirmation
            .filter(|pending| self.skills.iter().any(|skill| skill.id == *pending));
        self.pending_disable_skill_instance_confirmation =
            pending_disable_skill_instance_confirmation.filter(|pending| {
                self.skill_instances
                    .iter()
                    .any(|instance| instance.id == *pending)
            });
        self.pending_remove_confirmation = pending_remove_confirmation.filter(|pending| {
            self.deployments
                .iter()
                .any(|deployment| deployment.id == *pending)
        });
        self.skill_agent_filter = skill_agent_filter;
        self.skill_scope_filter = skill_scope_filter;
        self.skill_status_filter = skill_status_filter;
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
        self.install_local_skill_draft = None;
        self.open_project_draft = None;
        let status_kind = match &outcome {
            GuiControllerOutcome::AgentSkillsAdopted { failures, .. } if *failures > 0 => {
                GuiStatusKind::Error
            }
            _ => GuiStatusKind::Success,
        };
        self.last_status = Some(GuiStatus {
            kind: status_kind,
            message: success_message,
        });
        self.apply_controller_outcome(outcome);
        if let Some(selected) = &self.selected_discovered_project_skill {
            if !self.selected_project_summary().is_some_and(|summary| {
                summary.discovered_skills.iter().any(|skill| {
                    skill.agent_id == selected.agent_id && skill.name == selected.skill_name
                })
            }) {
                self.selected_discovered_project_skill = None;
            }
        }
        Ok(Some(intent))
    }

    fn intent_success_message(
        &self,
        intent: &GuiActionIntent,
        outcome: &GuiControllerOutcome,
    ) -> String {
        match intent {
            GuiActionIntent::InstallLocalSkill { source_path } => {
                let skill_name = source_path.file_name().unwrap_or("skill");
                let summary = match outcome {
                    GuiControllerOutcome::SkillInstalled { findings, .. } => GuiRiskReport {
                        scanned_hash: String::new(),
                        findings: findings.clone(),
                    }
                    .summary_label(),
                    _ => "No findings".to_string(),
                };
                format!("Installed {skill_name}: {summary}.")
            }
            GuiActionIntent::ScanAgentSpaces => match outcome {
                GuiControllerOutcome::AgentSpacesScanned { instances } => {
                    format!(
                        "Scanned Agent Spaces: {instances} instance{}.",
                        if *instances == 1 { "" } else { "s" }
                    )
                }
                _ => "Scanned Agent Spaces.".to_string(),
            },
            GuiActionIntent::ImportAllManagedCopies => match outcome {
                GuiControllerOutcome::AgentSkillsAdopted {
                    imported,
                    conflicts,
                    failures,
                } => {
                    let failure_summary = if *failures == 0 {
                        String::new()
                    } else {
                        format!(
                            ", {failures} failure{}",
                            if *failures == 1 { "" } else { "s" }
                        )
                    };
                    format!(
                        "Imported Agent Skills into Managed Inventory: {imported} imported, {conflicts} conflict{}{failure_summary}.",
                        if *conflicts == 1 { "" } else { "s" },
                    )
                }
                _ => "Imported Agent Skills into Managed Inventory.".to_string(),
            },
            GuiActionIntent::ImportManagedCopy { instance_id } => {
                let skill_name = self
                    .skill_instances
                    .iter()
                    .find(|instance| instance.id == *instance_id)
                    .map(|instance| instance.name.as_str())
                    .unwrap_or(instance_id);
                format!("Imported {skill_name} into Managed Inventory.")
            }
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
                format!("Uninstalled managed copy {skill_name} from Managed Inventory.")
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
            GuiActionIntent::EnableSkillInstance { instance_id } => {
                let skill_name = self
                    .skill_instances
                    .iter()
                    .find(|instance| instance.id == *instance_id)
                    .map(|instance| instance.name.as_str())
                    .unwrap_or(instance_id);
                format!("Enabled {skill_name} in Agent Space.")
            }
            GuiActionIntent::DisableSkillInstance { instance_id } => {
                let skill_name = self
                    .skill_instances
                    .iter()
                    .find(|instance| instance.id == *instance_id)
                    .map(|instance| instance.name.as_str())
                    .unwrap_or(instance_id);
                format!("Disabled {skill_name} in Agent Space.")
            }
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
            GuiActionIntent::ProjectAdoptSelected {
                project_path,
                skill_name,
                ..
            } => format!("Adopted {skill_name} for {}.", project_label(project_path)),
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
            GuiActionIntent::ResetAgentProjectSkillDirs { agent_id } => {
                format!(
                    "Reset {} project Skill directories.",
                    self.agent_label(agent_id)
                )
            }
            GuiActionIntent::RemoveCustomAgent { agent_id } => {
                format!("Removed custom Agent {}.", self.agent_label(agent_id))
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
            GuiControllerOutcome::None
            | GuiControllerOutcome::AgentSpacesScanned { .. }
            | GuiControllerOutcome::AgentSkillsAdopted { .. } => {}
            GuiControllerOutcome::SkillInstalled {
                skill_id,
                scanned_hash,
                findings,
            }
            | GuiControllerOutcome::SkillScan {
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
                discovered_skills,
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
                    let discovered_skills = filter_discovered_skills(
                        discovered_skills,
                        &pending_conflicts,
                        &skipped_conflicts,
                    );
                    summary.deployment_count = deployment_count;
                    summary.onboarding_scanned = true;
                    summary.discovered_unmanaged_count = discovered_skills.len();
                    summary.discovered_skills = discovered_skills;
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
                        onboarding_scanned: true,
                        discovered_unmanaged_count: discovered_skills.len(),
                        discovered_skills,
                        last_adopt_all_result: adopt_result,
                        pending_conflicts,
                        skipped_conflicts: Vec::new(),
                    });
                }
            }
            GuiControllerOutcome::ProjectOpened { project_path } => {
                let deployment_count = self
                    .deployments
                    .iter()
                    .filter(|deployment| deployment.project_path == project_path)
                    .count();
                if !self
                    .project_summaries
                    .iter()
                    .any(|summary| summary.path == project_path)
                {
                    self.project_summaries.push(ProjectSummary {
                        name: project_path
                            .file_name()
                            .map(ToOwned::to_owned)
                            .unwrap_or_else(|| project_path.to_string()),
                        path: project_path.clone(),
                        deployment_count,
                        onboarding_scanned: false,
                        discovered_unmanaged_count: 0,
                        discovered_skills: Vec::new(),
                        last_adopt_all_result: None,
                        pending_conflicts: Vec::new(),
                        skipped_conflicts: Vec::new(),
                    });
                }
                self.active_scope = GuiScope::Project(project_path.clone());
                self.selected_project = Some(project_path);
                self.selected_deployment = None;
                self.pending_remove_confirmation = None;
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

    pub fn select_skill_instance(&mut self, instance_id: String) {
        self.selected_skill_instance = Some(instance_id);
    }

    pub fn selected_skill_instance(&self) -> Option<&SkillInstance> {
        self.selected_skill_instance.as_ref().and_then(|selected| {
            self.skill_instances
                .iter()
                .find(|instance| instance.id == *selected)
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

    pub fn install_local_skill_draft(&self) -> Option<&InstallLocalSkillDraft> {
        self.install_local_skill_draft.as_ref()
    }

    pub fn open_project_draft(&self) -> Option<&OpenProjectDraft> {
        self.open_project_draft.as_ref()
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

    pub fn selected_discovered_project_skill(&self) -> Option<&ProjectDiscoveredSkill> {
        let selected = self.selected_discovered_project_skill.as_ref()?;
        self.selected_project_summary()?
            .discovered_skills
            .iter()
            .find(|skill| skill.agent_id == selected.agent_id && skill.name == selected.skill_name)
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
                agent_space_instance_count: 0,
                managed_skill_count: 0,
                agent_count: 0,
                enabled_agent_count: 0,
                recent_project_count: 0,
                deployment_count: 0,
                outdated_deployment_count: 0,
                drifted_deployment_count: 0,
                invalid_toggle_count: 0,
                missing_managed_source_count: 0,
                registry_health: HealthState::Ok,
                lock_health: HealthState::Ok,
                cache_health: HealthState::Ok,
                risk_count: 0,
            },
            skills: Vec::new(),
            skill_instances: Vec::new(),
            agents: Vec::new(),
            recent_projects: Vec::new(),
            project_summaries: Vec::new(),
            deployments: Vec::new(),
            deployment_statuses: Vec::new(),
            selected_skill: None,
            selected_skill_instance: None,
            selected_agent: None,
            selected_project: None,
            selected_deployment: None,
            pending_uninstall_confirmation: None,
            pending_disable_skill_instance_confirmation: None,
            pending_remove_confirmation: None,
            skill_agent_filter: None,
            skill_scope_filter: None,
            skill_status_filter: None,
            selected_discovered_project_skill: None,
            pending_intents: Vec::new(),
            last_status: None,
            skill_risk_reports: Vec::new(),
            agent_editor_draft: None,
            install_local_skill_draft: None,
            open_project_draft: None,
        }
    }
}

fn action_label(intent: &GuiActionIntent) -> &'static str {
    match intent {
        GuiActionIntent::InstallLocalSkill { .. } => "Install local Skill",
        GuiActionIntent::ScanAgentSpaces => "Scan Agent Spaces",
        GuiActionIntent::ImportAllManagedCopies => "Import all managed copies",
        GuiActionIntent::ImportManagedCopy { .. } => "Import managed copy",
        GuiActionIntent::ScanSkill { .. } => "Scan",
        GuiActionIntent::UninstallSkill { .. } => "Uninstall",
        GuiActionIntent::DeploySkill { .. } => "Deploy",
        GuiActionIntent::EnableDeployment { .. } => "Enable",
        GuiActionIntent::DisableDeployment { .. } => "Disable",
        GuiActionIntent::EnableSkillInstance { .. } => "Enable",
        GuiActionIntent::DisableSkillInstance { .. } => "Disable",
        GuiActionIntent::RemoveDeployment { .. } => "Remove",
        GuiActionIntent::RedeployDeployment { .. } => "Redeploy",
        GuiActionIntent::RefreshProject { .. } => "Refresh",
        GuiActionIntent::ProjectAdoptAll { .. } => "Adopt all",
        GuiActionIntent::ProjectAdoptSelected { .. } => "Adopt selected",
        GuiActionIntent::ProjectImportConflictAsNew { .. } => "Import as new",
        GuiActionIntent::OpenProject { .. } => "Open project",
        GuiActionIntent::UpdateAgentProjectSkillDirs { .. } => "Update Agent",
        GuiActionIntent::ResetAgentProjectSkillDirs { .. } => "Reset Agent",
        GuiActionIntent::RemoveCustomAgent { .. } => "Remove custom Agent",
        GuiActionIntent::AddCustomAgent { .. } => "Add custom Agent",
    }
}

fn filter_discovered_skills(
    discovered_skills: Vec<ProjectDiscoveredSkill>,
    pending_conflicts: &[ProjectConflict],
    skipped_conflicts: &[ProjectConflict],
) -> Vec<ProjectDiscoveredSkill> {
    discovered_skills
        .into_iter()
        .filter(|skill| {
            let key = skill.conflict_key();
            pending_conflicts.contains(&key) || !skipped_conflicts.contains(&key)
        })
        .collect()
}

fn parse_discovered_project_row_id(row_id: &str) -> Option<ProjectConflict> {
    let rest = row_id.strip_prefix("discovered:")?;
    let (agent_id, skill_name) = rest.split_once(':')?;
    if agent_id.is_empty() || skill_name.is_empty() {
        return None;
    }
    Some(ProjectConflict {
        agent_id: AgentId::new(agent_id),
        skill_name: skill_name.to_string(),
    })
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

pub fn skill_instance_scope_label(scope: &SkillInstanceScope) -> String {
    match scope {
        SkillInstanceScope::Global => "Global".to_string(),
        SkillInstanceScope::Project { name, .. } => format!("Project / {name}"),
    }
}

pub fn skill_instance_scope_filter_label(scope: &SkillInstanceScope) -> String {
    skill_instance_scope_label(scope)
}

pub fn skill_instance_status_label(instance: &SkillInstance) -> &'static str {
    if !instance.writable
        && matches!(
            instance.toggle_state,
            ToggleState::Enabled | ToggleState::Disabled
        )
    {
        return "Read-only";
    }
    match instance.toggle_state {
        ToggleState::Enabled => "Enabled",
        ToggleState::Disabled => "Disabled",
        ToggleState::InvalidBothPresent => "Invalid",
        ToggleState::InvalidBothMissing => "Missing",
    }
}

pub fn skill_instance_source_label(model: &GuiModel, instance: &SkillInstance) -> String {
    match &instance.source_kind {
        SkillInstanceSourceKind::AgentSpace => {
            format!("{} global", model.agent_label(&instance.agent_id))
        }
        SkillInstanceSourceKind::ProjectDeployment => "Project".to_string(),
        SkillInstanceSourceKind::PluginCache => "Plugin cache".to_string(),
        SkillInstanceSourceKind::Vendor => "Vendor".to_string(),
        SkillInstanceSourceKind::ManagedInventory => "Managed inventory".to_string(),
    }
}
