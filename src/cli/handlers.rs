use crate::cli::args::{
    Cli, Command, InstallCommand, OutputFormat, ProjectCommand, ProjectRedeployArgs,
    ProjectRemoveArgs, ProjectSkillAgentArgs, ProjectStatusArgs,
};
use crate::cli::output::{format_table, to_json, TableColumn};
use crate::core::adopt::AdoptReport;
use crate::core::doctor::{DoctorIssue, DoctorReport};
use crate::core::error::SkillKitsError;
use crate::core::ids::AgentId;
use crate::core::paths::AppPaths;
use crate::core::project::{
    resolve_project_scope, ProjectDeployRequest,
    ProjectRedeployRequest as CoreProjectRedeployRequest,
    ProjectRemoveRequest as CoreProjectRemoveRequest, ProjectSkillRequest, RedeployOutcome,
};
use crate::core::scan::RiskFinding;
use crate::core::status::GlobalStatus;
use crate::core::{DeploymentStatus, ManagedSkill};
use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;
use std::fmt;

pub fn run_cli() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match execute(cli) {
        Ok(Some(output)) => {
            println!("{output}");
            Ok(())
        }
        Ok(None) => Ok(()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(error.exit_code());
        }
    }
}

pub fn exit_code_for_error(error: &SkillKitsError) -> i32 {
    match error {
        SkillKitsError::DeployConflict { .. }
        | SkillKitsError::AdoptionConflict { .. }
        | SkillKitsError::DeploymentDrift { .. }
        | SkillKitsError::MissingManagedSource { .. }
        | SkillKitsError::UnsafeRemoveRequiresForce { .. }
        | SkillKitsError::InvalidToggleState { .. }
        | SkillKitsError::AmbiguousSkill { .. } => 3,
        SkillKitsError::RegistryBusy => 4,
        _ => 1,
    }
}

#[derive(Debug)]
pub enum CliRunError {
    Core(SkillKitsError),
    DoctorFoundErrors,
}

impl CliRunError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Core(error) => exit_code_for_error(error),
            Self::DoctorFoundErrors => 5,
        }
    }
}

impl fmt::Display for CliRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core(error) => fmt::Display::fmt(error, f),
            Self::DoctorFoundErrors => write!(f, "doctor found errors"),
        }
    }
}

impl std::error::Error for CliRunError {}

impl From<SkillKitsError> for CliRunError {
    fn from(error: SkillKitsError) -> Self {
        Self::Core(error)
    }
}

type CliResult<T> = std::result::Result<T, CliRunError>;

fn execute(cli: Cli) -> CliResult<Option<String>> {
    let app_paths = AppPaths::default_user_paths().map_err(CliRunError::from)?;
    match cli.command.unwrap_or(Command::Status {
        format: OutputFormat::Table,
    }) {
        Command::List { format } => render_list(list_skills(&app_paths)?, format),
        Command::Status { format } => render_global_status(global_status(&app_paths)?, format),
        Command::Install { command } => {
            let InstallCommand::Local { path } = command;
            render_operation(install_local(&app_paths, path)?)
        }
        Command::Uninstall { skill } => render_operation(uninstall_skill(&app_paths, skill)?),
        Command::Scan { skill, format } => render_scan(scan_skill(&app_paths, skill)?, format),
        Command::Doctor { fix } => render_doctor(doctor(&app_paths, fix)?),
        Command::Adopt { global_agent } => {
            render_adopt(adopt_global_agent(&app_paths, global_agent)?)
        }
        Command::Project { command } => execute_project(&app_paths, command),
    }
}

fn execute_project(app_paths: &AppPaths, command: ProjectCommand) -> CliResult<Option<String>> {
    match command {
        ProjectCommand::Status(args) => {
            let format = args.format;
            render_project_status(project_status(app_paths, args)?, format)
        }
        ProjectCommand::Adopt(args) => {
            render_adopt(project_adopt(app_paths, args.agent, args.project)?)
        }
        ProjectCommand::Deploy(args) => render_operation(project_deploy(app_paths, args)?),
        ProjectCommand::Enable(args) => render_operation(project_enable(app_paths, args)?),
        ProjectCommand::Disable(args) => render_operation(project_disable(app_paths, args)?),
        ProjectCommand::Redeploy(args) => render_operation(project_redeploy(app_paths, args)?),
        ProjectCommand::Remove(args) => render_operation(project_remove(app_paths, args)?),
    }
}

fn render_list(skills: Vec<ManagedSkill>, format: OutputFormat) -> CliResult<Option<String>> {
    match format {
        OutputFormat::Json => Ok(Some(to_json(&skills).map_err(general_io_error)?)),
        OutputFormat::Table => {
            let rows = skills
                .into_iter()
                .map(|skill| {
                    vec![
                        TableColumn::from(skill.id.to_string()),
                        TableColumn::from(skill.name),
                        TableColumn::from(skill.content_hash),
                    ]
                })
                .collect::<Vec<_>>();
            Ok(Some(format_table(
                &["Skill ID", "Name", "Content Hash"],
                &rows,
            )))
        }
    }
}

fn render_global_status(status: GlobalStatus, format: OutputFormat) -> CliResult<Option<String>> {
    #[derive(Serialize)]
    struct GlobalStatusOutput {
        managed_skills: usize,
        agents: usize,
        enabled_agents: usize,
        agent_config_state: String,
        recent_projects: usize,
        registry_health: String,
        lock_health: String,
        cache_health: String,
        risk_count: usize,
    }

    let output = GlobalStatusOutput {
        managed_skills: status.managed_skill_count,
        agents: status.agent_count,
        enabled_agents: status.enabled_agent_count,
        agent_config_state: format!("{:?}", status.agent_config_state),
        recent_projects: status.recent_project_count,
        registry_health: format!("{:?}", status.registry_health),
        lock_health: format!("{:?}", status.lock_health),
        cache_health: format!("{:?}", status.cache_health),
        risk_count: status.risk_count,
    };

    match format {
        OutputFormat::Json => Ok(Some(to_json(&output).map_err(general_io_error)?)),
        OutputFormat::Table => Ok(Some(format_table(
            &["Metric", "Value"],
            &[
                vec![
                    TableColumn::from("Managed Skills"),
                    TableColumn::from(output.managed_skills),
                ],
                vec![
                    TableColumn::from("Agents"),
                    TableColumn::from(output.agents),
                ],
                vec![
                    TableColumn::from("Enabled Agents"),
                    TableColumn::from(output.enabled_agents),
                ],
                vec![
                    TableColumn::from("Agent Config"),
                    TableColumn::from(output.agent_config_state),
                ],
                vec![
                    TableColumn::from("Recent Projects"),
                    TableColumn::from(output.recent_projects),
                ],
                vec![
                    TableColumn::from("Registry Health"),
                    TableColumn::from(output.registry_health),
                ],
                vec![
                    TableColumn::from("Lock Health"),
                    TableColumn::from(output.lock_health),
                ],
                vec![
                    TableColumn::from("Cache Health"),
                    TableColumn::from(output.cache_health),
                ],
                vec![
                    TableColumn::from("Risk Count"),
                    TableColumn::from(output.risk_count),
                ],
            ],
        ))),
    }
}

fn render_scan(findings: Vec<RiskFinding>, format: OutputFormat) -> CliResult<Option<String>> {
    match format {
        OutputFormat::Json => Ok(Some(to_json(&findings).map_err(general_io_error)?)),
        OutputFormat::Table => {
            let rows = findings
                .into_iter()
                .map(|finding| {
                    vec![
                        TableColumn::from(format!("{:?}", finding.severity)),
                        TableColumn::from(finding.rule_id),
                        TableColumn::from(finding.path.to_string()),
                        TableColumn::from(
                            finding
                                .line
                                .map_or_else(|| "-".to_string(), |line| line.to_string()),
                        ),
                        TableColumn::from(finding.message),
                    ]
                })
                .collect::<Vec<_>>();
            Ok(Some(format_table(
                &["Severity", "Rule", "Path", "Line", "Message"],
                &rows,
            )))
        }
    }
}

fn render_doctor(report: DoctorReport) -> CliResult<Option<String>> {
    let rows = doctor_rows(&report.issues);
    if !report.ok {
        if rows.is_empty() {
            eprintln!("doctor found errors");
        } else {
            eprintln!(
                "{}",
                format_table(&["Severity", "Check", "Path", "Message"], &rows)
            );
        }
        return Err(CliRunError::DoctorFoundErrors);
    }

    if rows.is_empty() {
        Ok(Some(format_table(
            &["Check", "Status"],
            &[vec![
                TableColumn::from("Skill-kits state"),
                TableColumn::from("ok"),
            ]],
        )))
    } else {
        Ok(Some(format_table(
            &["Severity", "Check", "Path", "Message"],
            &rows,
        )))
    }
}

fn doctor_rows(issues: &[DoctorIssue]) -> Vec<Vec<TableColumn>> {
    issues
        .iter()
        .map(|issue| {
            vec![
                TableColumn::from(format!("{:?}", issue.severity)),
                TableColumn::from(format!("{:?}", issue.code)),
                TableColumn::from(
                    issue
                        .path
                        .as_ref()
                        .map_or_else(|| "-".to_string(), |path| path.to_string()),
                ),
                TableColumn::from(issue.message.clone()),
            ]
        })
        .collect()
}

fn render_adopt(report: AdoptReport) -> CliResult<Option<String>> {
    Ok(Some(format_table(
        &["Imported", "Conflicts"],
        &[vec![
            TableColumn::from(report.imported),
            TableColumn::from(report.conflicts),
        ]],
    )))
}

fn render_project_status(
    deployments: Vec<DeploymentStatus>,
    format: OutputFormat,
) -> CliResult<Option<String>> {
    match format {
        OutputFormat::Json => Ok(Some(to_json(&deployments).map_err(general_io_error)?)),
        OutputFormat::Table => {
            let rows = deployments
                .into_iter()
                .map(|deployment| {
                    vec![
                        TableColumn::from(deployment.record.skill_name),
                        TableColumn::from(deployment.record.agent_id.to_string()),
                        TableColumn::from(format!("{:?}", deployment.toggle)),
                        TableColumn::from(deployment.outdated),
                        TableColumn::from(deployment.drift),
                    ]
                })
                .collect::<Vec<_>>();
            Ok(Some(format_table(
                &["Skill", "Agent", "Toggle", "Outdated", "Drift"],
                &rows,
            )))
        }
    }
}

fn render_operation(message: OperationMessage) -> CliResult<Option<String>> {
    Ok(Some(format_table(
        &["Operation", "Status"],
        &[vec![
            TableColumn::from(message.operation),
            TableColumn::from(message.status),
        ]],
    )))
}

fn general_io_error(error: anyhow::Error) -> CliRunError {
    SkillKitsError::Io {
        source: std::io::Error::other(error.to_string()),
    }
    .into()
}

#[derive(Clone, Debug, Serialize)]
struct OperationMessage {
    operation: String,
    status: String,
}

fn list_skills(app_paths: &AppPaths) -> crate::core::Result<Vec<ManagedSkill>> {
    Ok(crate::core::registry::read_skills_registry(app_paths)?.skills)
}

fn global_status(app_paths: &AppPaths) -> crate::core::Result<GlobalStatus> {
    crate::core::status::global_status(app_paths)
}

fn install_local(app_paths: &AppPaths, path: Utf8PathBuf) -> crate::core::Result<OperationMessage> {
    let result = crate::core::install::install_local_skill(
        crate::core::install::InstallLocalRequest {
            source_path: path.as_path(),
        },
        app_paths,
    )?;
    Ok(OperationMessage {
        operation: "install local".to_string(),
        status: format!("installed {}", result.skill.id),
    })
}

fn uninstall_skill(app_paths: &AppPaths, query: String) -> crate::core::Result<OperationMessage> {
    let result =
        crate::core::install::uninstall_skill(crate::core::install::UninstallSkillRequest {
            app_paths,
            query: &query,
        })?;
    Ok(operation_status("uninstall", result.skill_id.as_str()))
}

fn scan_skill(
    app_paths: &AppPaths,
    skill: Option<String>,
) -> crate::core::Result<Vec<RiskFinding>> {
    if let Some(query) = skill {
        let skill = list_skills(app_paths)?
            .into_iter()
            .find(|skill| skill.id.as_str() == query || skill.name == query)
            .ok_or(SkillKitsError::SkillNotFound { query })?;
        crate::core::scan::scan_skill_dir(&skill.managed_path)
    } else {
        let mut findings = Vec::new();
        for skill in list_skills(app_paths)? {
            findings.extend(crate::core::scan::scan_skill_dir(&skill.managed_path)?);
        }
        Ok(findings)
    }
}

fn doctor(app_paths: &AppPaths, fix: bool) -> crate::core::Result<DoctorReport> {
    crate::core::doctor::run_doctor(app_paths, fix)
}

fn adopt_global_agent(
    app_paths: &AppPaths,
    global_agent: String,
) -> crate::core::Result<AdoptReport> {
    let home = dirs::home_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "home directory"))?;
    let home = Utf8PathBuf::from_path_buf(home).map_err(|path| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("home path is not UTF-8: {}", path.display()),
        )
    })?;
    crate::core::adopt::global_agent_adopt(crate::core::adopt::GlobalAgentAdoptRequest {
        app_paths,
        agent_id: &AgentId::new(global_agent),
        home_dir: &home,
    })
}

fn project_status(
    app_paths: &AppPaths,
    args: ProjectStatusArgs,
) -> crate::core::Result<Vec<DeploymentStatus>> {
    let project = project_path_or_cwd(args.project)?;
    let deployments = crate::core::registry::read_deployments_registry(app_paths)?;
    deployments
        .deployments
        .into_iter()
        .filter(|record| record.project_path == project)
        .map(|record| {
            crate::core::project::project_deployment_status(
                app_paths,
                &project,
                &record.agent_id,
                &record.skill_name,
            )
        })
        .collect()
}

fn project_adopt(
    app_paths: &AppPaths,
    agent: String,
    project: Option<Utf8PathBuf>,
) -> crate::core::Result<AdoptReport> {
    let project = project_path_or_cwd(project)?;
    crate::core::adopt::project_adopt_all(crate::core::adopt::ProjectAdoptRequest {
        app_paths,
        project_path: &project,
        agent_id: &AgentId::new(agent),
        skill_name: "",
    })
}

fn project_deploy(
    app_paths: &AppPaths,
    args: ProjectSkillAgentArgs,
) -> crate::core::Result<OperationMessage> {
    let project = project_path_or_cwd(args.project)?;
    let status = crate::core::project::deploy_project_skill(ProjectDeployRequest {
        app_paths,
        project_path: &project,
        agent_id: &AgentId::new(args.agent),
        skill_query: &args.skill,
    })?;
    Ok(operation_status(
        "project deploy",
        &status.record.skill_name,
    ))
}

fn project_enable(
    app_paths: &AppPaths,
    args: ProjectSkillAgentArgs,
) -> crate::core::Result<OperationMessage> {
    let project = project_path_or_cwd(args.project)?;
    let status = crate::core::project::enable_project_skill(ProjectSkillRequest {
        app_paths,
        project_path: &project,
        agent_id: &AgentId::new(args.agent),
        skill_query: &args.skill,
    })?;
    Ok(operation_status(
        "project enable",
        &status.record.skill_name,
    ))
}

fn project_disable(
    app_paths: &AppPaths,
    args: ProjectSkillAgentArgs,
) -> crate::core::Result<OperationMessage> {
    let project = project_path_or_cwd(args.project)?;
    let status = crate::core::project::disable_project_skill(ProjectSkillRequest {
        app_paths,
        project_path: &project,
        agent_id: &AgentId::new(args.agent),
        skill_query: &args.skill,
    })?;
    Ok(operation_status(
        "project disable",
        &status.record.skill_name,
    ))
}

fn project_redeploy(
    app_paths: &AppPaths,
    args: ProjectRedeployArgs,
) -> crate::core::Result<OperationMessage> {
    let project = project_path_or_cwd(args.project)?;
    let outcome = crate::core::project::redeploy_project_skill(CoreProjectRedeployRequest {
        app_paths,
        project_path: &project,
        agent_id: &AgentId::new(args.agent),
        skill_query: &args.skill,
        overwrite: args.overwrite,
        promote: args.promote,
    })?;
    let status = match outcome {
        RedeployOutcome::Overwritten(status) => format!("overwritten {}", status.record.skill_name),
        RedeployOutcome::Promoted(skill) => format!("promoted {}", skill.id),
    };
    Ok(OperationMessage {
        operation: "project redeploy".to_string(),
        status,
    })
}

fn project_remove(
    app_paths: &AppPaths,
    args: ProjectRemoveArgs,
) -> crate::core::Result<OperationMessage> {
    let project = project_path_or_cwd(args.project)?;
    crate::core::project::remove_project_skill(CoreProjectRemoveRequest {
        app_paths,
        project_path: &project,
        agent_id: &AgentId::new(args.agent),
        skill_query: &args.skill,
        force: args.force,
    })?;
    Ok(operation_status("project remove", &args.skill))
}

fn operation_status(operation: &str, subject: &str) -> OperationMessage {
    OperationMessage {
        operation: operation.to_string(),
        status: subject.to_string(),
    }
}

fn project_path_or_cwd(project: Option<Utf8PathBuf>) -> crate::core::Result<Utf8PathBuf> {
    let scope = resolve_project_scope(project.as_deref())?;
    Ok(scope.path)
}

#[cfg(test)]
mod tests {
    use super::CliRunError;

    #[test]
    fn doctor_error_code_is_reserved() {
        assert_eq!(CliRunError::DoctorFoundErrors.exit_code(), 5);
    }
}
