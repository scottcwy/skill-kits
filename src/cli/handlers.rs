use crate::cli::args::{
    Cli, Command, InstallCommand, ListKind, OutputFormat, PluginCommand, ProjectAgentArgs,
    ProjectCommand, ProjectRedeployArgs, ProjectRemoveArgs, ProjectSkillAgentArgs,
    ProjectStatusArgs,
};
use crate::cli::output::{format_table, to_json, TableColumn};
use crate::core::adopt::AdoptReport;
use crate::core::agent_space::{
    disable_project_skill_instance, disable_skill_instance_by_query, enable_project_skill_instance,
    enable_skill_instance_by_query, project_skill_instances, read_skill_instance_index,
    refresh_skill_instance_index, ProjectSkillInstanceRequest, SkillInstance, SkillInstanceIndex,
    SkillInstanceQueryRequest, SkillInstanceScope, SkillInstanceSourceKind,
};
use crate::core::config::read_config;
use crate::core::doctor::{DoctorIssue, DoctorReport};
use crate::core::error::SkillKitsError;
use crate::core::ids::AgentId;
use crate::core::paths::AppPaths;
use crate::core::plugins::{
    disable_plugin, enable_plugin, find_plugin_by_query, flattened_capabilities,
    query_matches_plugin_capability, scan_plugin_packages, PluginPackage, PluginRuntimeCapability,
    PluginStatus, RuntimeCapabilityKind,
};
use crate::core::project::{
    resolve_project_scope, ProjectDeployRequest,
    ProjectRedeployRequest as CoreProjectRedeployRequest,
    ProjectRemoveRequest as CoreProjectRemoveRequest, RedeployOutcome,
};
use crate::core::scan::RiskFinding;
use crate::core::ManagedSkill;
use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;
use std::fmt;

pub fn run_cli() -> anyhow::Result<()> {
    let cli = Cli::parse();
    run_parsed_cli(cli)
}

pub fn run_parsed_cli(cli: Cli) -> anyhow::Result<()> {
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
        | SkillKitsError::AmbiguousSkill { .. }
        | SkillKitsError::AmbiguousPlugin { .. }
        | SkillKitsError::InvalidPluginConfig { .. }
        | SkillKitsError::PluginToggleBlocked { .. }
        | SkillKitsError::InvalidNativeToggleTarget { .. } => 3,
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
        Command::List { kind, format } => execute_list(&app_paths, kind, format),
        Command::Status { format } => {
            render_agent_space_status(agent_space_status(&app_paths)?, format)
        }
        Command::Install { command } => {
            let InstallCommand::Local { path } = command;
            render_operation(install_local(&app_paths, path)?)
        }
        Command::Uninstall { skill } => render_operation(uninstall_skill(&app_paths, skill)?),
        Command::Enable { query } => render_operation(enable_instance(&app_paths, query)?),
        Command::Disable { query } => render_operation(disable_instance(&app_paths, query)?),
        Command::Scan { skill, format } => match skill {
            Some(skill) => render_scan(scan_skill(&app_paths, skill)?, format),
            None => render_scan_index(scan_agent_spaces(&app_paths)?, format),
        },
        Command::Doctor { fix } => render_doctor(doctor(&app_paths, fix)?),
        Command::Adopt { global_agent } => {
            render_adopt(adopt_global_agent(&app_paths, global_agent)?)
        }
        Command::Project { command } => execute_project(&app_paths, command),
        Command::Plugin { command } => execute_plugin(&app_paths, command),
    }
}

fn execute_list(
    app_paths: &AppPaths,
    kind: ListKind,
    format: OutputFormat,
) -> CliResult<Option<String>> {
    match kind {
        ListKind::Skill => render_list(list_skill_instances(app_paths)?, format),
        ListKind::Plugin => render_plugins(list_plugins(app_paths)?, format),
        ListKind::RuntimeCapability => {
            render_runtime_capabilities(list_runtime_capabilities(app_paths)?, format)
        }
    }
}

fn execute_project(app_paths: &AppPaths, command: ProjectCommand) -> CliResult<Option<String>> {
    match command {
        ProjectCommand::Status(args) => {
            let format = args.format;
            render_project_status(project_status(app_paths, args)?, format)
        }
        ProjectCommand::Adopt(args) => render_adopt(project_adopt(app_paths, args)?),
        ProjectCommand::Deploy(args) => render_operation(project_deploy(app_paths, args)?),
        ProjectCommand::Enable(args) => render_operation(project_enable(app_paths, args)?),
        ProjectCommand::Disable(args) => render_operation(project_disable(app_paths, args)?),
        ProjectCommand::Redeploy(args) => render_operation(project_redeploy(app_paths, args)?),
        ProjectCommand::Remove(args) => render_operation(project_remove(app_paths, args)?),
    }
}

fn execute_plugin(app_paths: &AppPaths, command: PluginCommand) -> CliResult<Option<String>> {
    match command {
        PluginCommand::List { format } => render_plugins(list_plugins(app_paths)?, format),
        PluginCommand::Status { query, format } => {
            let plugins = list_plugins(app_paths)?;
            render_plugins(vec![find_plugin_by_query(&plugins, &query)?], format)
        }
        PluginCommand::Enable { query } => {
            render_operation(enable_plugin_command(app_paths, query)?)
        }
        PluginCommand::Disable { query } => {
            render_operation(disable_plugin_command(app_paths, query)?)
        }
        PluginCommand::Scan { format } => render_plugins(list_plugins(app_paths)?, format),
    }
}

fn render_list(instances: Vec<SkillInstance>, format: OutputFormat) -> CliResult<Option<String>> {
    match format {
        OutputFormat::Json => Ok(Some(to_json(&instances).map_err(general_io_error)?)),
        OutputFormat::Table => {
            let rows = instances
                .into_iter()
                .map(|instance| {
                    let status = toggle_label(&instance);
                    let source = source_label(&instance.source_kind);
                    vec![
                        TableColumn::from(instance.id),
                        TableColumn::from(instance.name),
                        TableColumn::from(instance.agent_id.to_string()),
                        TableColumn::from(scope_label(&instance.scope)),
                        TableColumn::from(status),
                        TableColumn::from(source),
                        TableColumn::from(instance.skill_dir.to_string()),
                        TableColumn::from(instance.updated_at.unwrap_or_else(|| "-".to_string())),
                    ]
                })
                .collect::<Vec<_>>();
            Ok(Some(format_table(
                &[
                    "Instance ID",
                    "Skill",
                    "Agent",
                    "Scope",
                    "Status",
                    "Source",
                    "Skill Dir",
                    "Updated",
                ],
                &rows,
            )))
        }
    }
}

fn render_plugins(plugins: Vec<PluginPackage>, format: OutputFormat) -> CliResult<Option<String>> {
    match format {
        OutputFormat::Json => Ok(Some(to_json(&plugins).map_err(general_io_error)?)),
        OutputFormat::Table => {
            let rows = plugins
                .into_iter()
                .map(|plugin| {
                    vec![
                        TableColumn::from(plugin.id),
                        TableColumn::from(plugin.display_name),
                        TableColumn::from(plugin.plugin_key),
                        TableColumn::from(plugin.provider),
                        TableColumn::from(plugin.agent_id.to_string()),
                        TableColumn::from(plugin_status_label(&plugin.status)),
                        TableColumn::from(capabilities_summary(&plugin.capabilities)),
                        TableColumn::from(plugin.package_path.to_string()),
                        TableColumn::from(plugin.updated_at.unwrap_or_else(|| "-".to_string())),
                    ]
                })
                .collect::<Vec<_>>();
            Ok(Some(format_table(
                &[
                    "Plugin ID",
                    "Plugin",
                    "Plugin Key",
                    "Provider",
                    "Agent",
                    "Status",
                    "Capabilities",
                    "Path",
                    "Updated",
                ],
                &rows,
            )))
        }
    }
}

fn render_runtime_capabilities(
    capabilities: Vec<PluginRuntimeCapability>,
    format: OutputFormat,
) -> CliResult<Option<String>> {
    match format {
        OutputFormat::Json => Ok(Some(to_json(&capabilities).map_err(general_io_error)?)),
        OutputFormat::Table => {
            let rows = capabilities
                .into_iter()
                .map(|capability| {
                    vec![
                        TableColumn::from(capability.id),
                        TableColumn::from(capability.parent_plugin_id),
                        TableColumn::from(capability.name),
                        TableColumn::from(runtime_kind_label(&capability.kind)),
                        TableColumn::from(capability.path.to_string()),
                        TableColumn::from(capability.read_only),
                    ]
                })
                .collect::<Vec<_>>();
            Ok(Some(format_table(
                &[
                    "Capability ID",
                    "Plugin ID",
                    "Capability",
                    "Kind",
                    "Path",
                    "Read-only",
                ],
                &rows,
            )))
        }
    }
}

fn render_agent_space_status(
    status: AgentSpaceStatus,
    format: OutputFormat,
) -> CliResult<Option<String>> {
    match format {
        OutputFormat::Json => Ok(Some(to_json(&status).map_err(general_io_error)?)),
        OutputFormat::Table => Ok(Some(format_table(
            &["Metric", "Value"],
            &[
                vec![
                    TableColumn::from("Agents"),
                    TableColumn::from(status.agents),
                ],
                vec![
                    TableColumn::from("Enabled Agents"),
                    TableColumn::from(status.enabled_agents),
                ],
                vec![
                    TableColumn::from("Global Instances"),
                    TableColumn::from(status.global_instances),
                ],
                vec![
                    TableColumn::from("Project Instances"),
                    TableColumn::from(status.project_instances),
                ],
                vec![
                    TableColumn::from("Invalid Instances"),
                    TableColumn::from(status.invalid_instances),
                ],
                vec![
                    TableColumn::from("Read-only Instances"),
                    TableColumn::from(status.read_only_instances),
                ],
                vec![
                    TableColumn::from("Recent Projects"),
                    TableColumn::from(status.recent_projects),
                ],
                vec![
                    TableColumn::from("Last Scan"),
                    TableColumn::from(status.last_scanned_at),
                ],
                vec![
                    TableColumn::from("Stale Entries"),
                    TableColumn::from(status.stale_entries),
                ],
            ],
        ))),
    }
}

fn render_scan_index(index: SkillInstanceIndex, format: OutputFormat) -> CliResult<Option<String>> {
    match format {
        OutputFormat::Json => Ok(Some(to_json(&index).map_err(general_io_error)?)),
        OutputFormat::Table => Ok(Some(format_table(
            &["Operation", "Status"],
            &[vec![
                TableColumn::from("Scan Agent Spaces"),
                TableColumn::from(format!("indexed {} instances", index.instances.len())),
            ]],
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
    let next_step = if report.conflicts == 0 {
        "none"
    } else {
        "resolve conflicts by importing as a new Skill or skipping"
    };
    Ok(Some(format_table(
        &["Imported", "Conflicts", "Next Step"],
        &[vec![
            TableColumn::from(report.imported),
            TableColumn::from(report.conflicts),
            TableColumn::from(next_step),
        ]],
    )))
}

fn render_project_status(
    instances: Vec<SkillInstance>,
    format: OutputFormat,
) -> CliResult<Option<String>> {
    match format {
        OutputFormat::Json => Ok(Some(to_json(&instances).map_err(general_io_error)?)),
        OutputFormat::Table => {
            let rows = instances
                .into_iter()
                .map(|instance| {
                    let status = toggle_label(&instance);
                    let source = source_label(&instance.source_kind);
                    vec![
                        TableColumn::from(instance.name),
                        TableColumn::from(instance.agent_id.to_string()),
                        TableColumn::from(instance.skill_dir.to_string()),
                        TableColumn::from(status),
                        TableColumn::from(source),
                        TableColumn::from(instance.writable),
                    ]
                })
                .collect::<Vec<_>>();
            Ok(Some(format_table(
                &[
                    "Skill",
                    "Agent",
                    "Skill Dir",
                    "Status",
                    "Source",
                    "Writable",
                ],
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

#[derive(Clone, Debug, Serialize)]
struct AgentSpaceStatus {
    agents: usize,
    enabled_agents: usize,
    global_instances: usize,
    project_instances: usize,
    invalid_instances: usize,
    read_only_instances: usize,
    recent_projects: usize,
    last_scanned_at: String,
    stale_entries: usize,
}

fn list_skill_instances(app_paths: &AppPaths) -> crate::core::Result<Vec<SkillInstance>> {
    Ok(read_skill_instance_index(app_paths)?
        .instances
        .into_iter()
        .filter(|instance| {
            matches!(
                instance.source_kind,
                SkillInstanceSourceKind::AgentSpace | SkillInstanceSourceKind::ProjectAgentSpace
            )
        })
        .collect())
}

fn list_plugins(app_paths: &AppPaths) -> crate::core::Result<Vec<PluginPackage>> {
    let home = default_home_dir()?;
    scan_plugin_packages(app_paths, &home)
}

fn list_runtime_capabilities(
    app_paths: &AppPaths,
) -> crate::core::Result<Vec<PluginRuntimeCapability>> {
    Ok(flattened_capabilities(&list_plugins(app_paths)?))
}

fn agent_space_status(app_paths: &AppPaths) -> crate::core::Result<AgentSpaceStatus> {
    let config = read_config(app_paths)?;
    let index = read_skill_instance_index(app_paths)?;
    Ok(AgentSpaceStatus {
        agents: config.agents.len(),
        enabled_agents: config.agents.iter().filter(|agent| agent.enabled).count(),
        global_instances: index
            .instances
            .iter()
            .filter(|instance| matches!(instance.scope, SkillInstanceScope::Global))
            .count(),
        project_instances: index
            .instances
            .iter()
            .filter(|instance| matches!(instance.scope, SkillInstanceScope::Project { .. }))
            .count(),
        invalid_instances: index
            .instances
            .iter()
            .filter(|instance| {
                matches!(
                    instance.toggle_state,
                    crate::core::registry::ToggleState::InvalidBothPresent
                        | crate::core::registry::ToggleState::InvalidBothMissing
                )
            })
            .count(),
        read_only_instances: index
            .instances
            .iter()
            .filter(|instance| {
                matches!(
                    instance.source_kind,
                    SkillInstanceSourceKind::PluginCache | SkillInstanceSourceKind::Vendor
                ) || !instance.writable
            })
            .count(),
        recent_projects: config.recent_projects.len(),
        last_scanned_at: if index.last_scanned_at.is_empty() {
            "-".to_string()
        } else {
            index.last_scanned_at
        },
        stale_entries: index
            .instances
            .iter()
            .filter(|instance| !instance.enabled_path.exists() && !instance.disabled_path.exists())
            .count(),
    })
}

fn install_local(app_paths: &AppPaths, path: Utf8PathBuf) -> crate::core::Result<OperationMessage> {
    let result = crate::core::install::install_local_skill(
        crate::core::install::InstallLocalRequest {
            source_path: path.as_path(),
        },
        app_paths,
    )?;
    Ok(OperationMessage {
        operation: "legacy install local".to_string(),
        status: format!(
            "legacy managed-copy command; installed {} in Managed Inventory",
            result.skill.id
        ),
    })
}

fn uninstall_skill(app_paths: &AppPaths, query: String) -> crate::core::Result<OperationMessage> {
    let result =
        crate::core::install::uninstall_skill(crate::core::install::UninstallSkillRequest {
            app_paths,
            query: &query,
        })?;
    Ok(operation_status(
        "legacy uninstall",
        &format!(
            "legacy managed-copy command; removed {} from Managed Inventory",
            result.skill_id
        ),
    ))
}

fn enable_instance(app_paths: &AppPaths, query: String) -> crate::core::Result<OperationMessage> {
    let home = default_home_dir()?;
    let instance = match enable_skill_instance_by_query(SkillInstanceQueryRequest {
        app_paths,
        home_dir: &home,
        query: &query,
    }) {
        Ok(instance) => instance,
        Err(SkillKitsError::SkillNotFound { .. }) => {
            return reject_native_plugin_target(app_paths, &home, &query);
        }
        Err(error) => return Err(error),
    };
    Ok(operation_status("enable", &instance.name))
}

fn disable_instance(app_paths: &AppPaths, query: String) -> crate::core::Result<OperationMessage> {
    let home = default_home_dir()?;
    let instance = match disable_skill_instance_by_query(SkillInstanceQueryRequest {
        app_paths,
        home_dir: &home,
        query: &query,
    }) {
        Ok(instance) => instance,
        Err(SkillKitsError::SkillNotFound { .. }) => {
            return reject_native_plugin_target(app_paths, &home, &query);
        }
        Err(error) => return Err(error),
    };
    Ok(operation_status("disable", &instance.name))
}

fn enable_plugin_command(
    app_paths: &AppPaths,
    query: String,
) -> crate::core::Result<OperationMessage> {
    let home = default_home_dir()?;
    let plugin = enable_plugin(app_paths, &home, &query)?;
    Ok(operation_status("plugin enable", &plugin.plugin_key))
}

fn disable_plugin_command(
    app_paths: &AppPaths,
    query: String,
) -> crate::core::Result<OperationMessage> {
    let home = default_home_dir()?;
    let plugin = disable_plugin(app_paths, &home, &query)?;
    Ok(operation_status("plugin disable", &plugin.plugin_key))
}

fn reject_native_plugin_target(
    app_paths: &AppPaths,
    home: &Utf8PathBuf,
    query: &str,
) -> crate::core::Result<OperationMessage> {
    let plugins = scan_plugin_packages(app_paths, home)?;
    if find_plugin_by_query(&plugins, query).is_ok() {
        return Err(SkillKitsError::InvalidNativeToggleTarget {
            message:
                "This is a plugin package. Use skill-kits plugin enable or skill-kits plugin disable."
                    .to_string(),
        });
    }
    if query_matches_plugin_capability(&plugins, query) {
        return Err(SkillKitsError::InvalidNativeToggleTarget {
            message:
                "This is a plugin-provided Skill. Disable or uninstall the parent plugin instead."
                    .to_string(),
        });
    }
    Err(SkillKitsError::SkillNotFound {
        query: query.to_string(),
    })
}

fn scan_skill(app_paths: &AppPaths, query: String) -> crate::core::Result<Vec<RiskFinding>> {
    let skill = legacy_managed_skills(app_paths)?
        .into_iter()
        .find(|skill| skill.id.as_str() == query || skill.name == query)
        .ok_or(SkillKitsError::SkillNotFound { query })?;
    crate::core::scan::scan_skill_dir(&skill.managed_path)
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
) -> crate::core::Result<Vec<SkillInstance>> {
    let project = project_path_or_cwd(args.project)?;
    let home = default_home_dir()?;
    project_skill_instances(app_paths, &home, &project)
}

fn project_adopt(app_paths: &AppPaths, args: ProjectAgentArgs) -> crate::core::Result<AdoptReport> {
    let project = project_path_or_cwd(args.project)?;
    let agent_id = AgentId::new(args.agent);
    let request = crate::core::adopt::ProjectAdoptRequest {
        app_paths,
        project_path: &project,
        agent_id: &agent_id,
        skill_name: args.skill.as_deref().unwrap_or(""),
    };
    match args.skill {
        Some(_) => crate::core::adopt::project_adopt(request),
        None => crate::core::adopt::project_adopt_all(request),
    }
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
        "legacy project deploy",
        &format!(
            "legacy managed-copy command; deployed {}",
            status.record.skill_name
        ),
    ))
}

fn project_enable(
    app_paths: &AppPaths,
    args: ProjectSkillAgentArgs,
) -> crate::core::Result<OperationMessage> {
    let project = project_path_or_cwd(args.project)?;
    let home = default_home_dir()?;
    let status = enable_project_skill_instance(ProjectSkillInstanceRequest {
        app_paths,
        home_dir: &home,
        project_path: &project,
        agent_id: &AgentId::new(args.agent),
        skill_query: &args.skill,
    })?;
    Ok(operation_status("project enable", &status.name))
}

fn project_disable(
    app_paths: &AppPaths,
    args: ProjectSkillAgentArgs,
) -> crate::core::Result<OperationMessage> {
    let project = project_path_or_cwd(args.project)?;
    let home = default_home_dir()?;
    let status = disable_project_skill_instance(ProjectSkillInstanceRequest {
        app_paths,
        home_dir: &home,
        project_path: &project,
        agent_id: &AgentId::new(args.agent),
        skill_query: &args.skill,
    })?;
    Ok(operation_status("project disable", &status.name))
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
        operation: "legacy project redeploy".to_string(),
        status: format!("legacy managed-copy command; {status}"),
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
    Ok(operation_status(
        "legacy project remove",
        &format!("legacy managed-copy command; removed {}", args.skill),
    ))
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

fn scan_agent_spaces(app_paths: &AppPaths) -> crate::core::Result<SkillInstanceIndex> {
    let home = default_home_dir()?;
    refresh_skill_instance_index(app_paths, &home)
}

fn legacy_managed_skills(app_paths: &AppPaths) -> crate::core::Result<Vec<ManagedSkill>> {
    Ok(crate::core::registry::read_skills_registry(app_paths)?.skills)
}

fn default_home_dir() -> crate::core::Result<Utf8PathBuf> {
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

fn toggle_label(instance: &SkillInstance) -> String {
    if !instance.writable
        && matches!(
            instance.toggle_state,
            crate::core::registry::ToggleState::Enabled
                | crate::core::registry::ToggleState::Disabled
        )
    {
        return "Read-only".to_string();
    }
    match instance.toggle_state {
        crate::core::registry::ToggleState::Enabled => "Enabled".to_string(),
        crate::core::registry::ToggleState::Disabled => "Disabled".to_string(),
        crate::core::registry::ToggleState::InvalidBothPresent => "Invalid".to_string(),
        crate::core::registry::ToggleState::InvalidBothMissing => "Missing".to_string(),
    }
}

fn scope_label(scope: &SkillInstanceScope) -> String {
    match scope {
        SkillInstanceScope::Global => "Global".to_string(),
        SkillInstanceScope::Project { name, .. } => format!("Project / {name}"),
    }
}

fn source_label(source: &SkillInstanceSourceKind) -> String {
    match source {
        SkillInstanceSourceKind::AgentSpace => "Agent Space".to_string(),
        SkillInstanceSourceKind::ProjectAgentSpace => "Project Agent Space".to_string(),
        SkillInstanceSourceKind::PluginCache => "Plugin cache".to_string(),
        SkillInstanceSourceKind::Vendor => "Vendor".to_string(),
    }
}

fn plugin_status_label(status: &PluginStatus) -> &'static str {
    match status {
        PluginStatus::Enabled => "Enabled",
        PluginStatus::Disabled => "Disabled",
        PluginStatus::Unknown => "Unknown",
        PluginStatus::Invalid => "Invalid",
    }
}

fn runtime_kind_label(kind: &RuntimeCapabilityKind) -> &'static str {
    match kind {
        RuntimeCapabilityKind::PluginProvidedSkill => "Skill",
        RuntimeCapabilityKind::Command => "Command",
        RuntimeCapabilityKind::Agent => "Agent",
        RuntimeCapabilityKind::Asset => "Asset",
        RuntimeCapabilityKind::App => "App",
        RuntimeCapabilityKind::Unknown => "Unknown",
    }
}

fn capabilities_summary(capabilities: &[PluginRuntimeCapability]) -> String {
    if capabilities.is_empty() {
        return "No capabilities".to_string();
    }

    let mut counts: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    for capability in capabilities {
        *counts
            .entry(runtime_kind_label(&capability.kind))
            .or_default() += 1;
    }
    ["Skill", "Command", "Agent", "Asset", "App", "Unknown"]
        .into_iter()
        .filter_map(|label| {
            let count = counts.get(label)?;
            let plural = if *count == 1 { "" } else { "s" };
            Some(format!("{count} {label}{plural}"))
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

#[cfg(test)]
mod tests {
    use super::{
        project_adopt, reject_native_plugin_target, render_adopt, render_project_status,
        CliRunError,
    };
    use crate::cli::args::{OutputFormat, ProjectAgentArgs};
    use crate::core::adopt::AdoptReport;
    use crate::core::agent_space::{SkillInstance, SkillInstanceScope, SkillInstanceSourceKind};
    use crate::core::ids::AgentId;
    use crate::core::paths::AppPaths;
    use crate::core::registry::{read_skills_registry, ToggleState};
    use camino::{Utf8Path, Utf8PathBuf};
    use tempfile::TempDir;

    fn test_paths(temp_dir: &TempDir) -> AppPaths {
        AppPaths::from_data_root(
            Utf8PathBuf::from_path_buf(temp_dir.path().join(".skill-kits")).unwrap(),
        )
    }

    fn write_skill(path: &Utf8Path, body: &str) {
        std::fs::create_dir_all(path).unwrap();
        std::fs::write(path.join("SKILL.md"), body).unwrap();
    }

    fn write_file(path: &Utf8Path, body: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    #[test]
    fn doctor_error_code_is_reserved() {
        assert_eq!(CliRunError::DoctorFoundErrors.exit_code(), 5);
    }

    #[test]
    fn adopt_conflict_output_includes_next_step() {
        let output = render_adopt(AdoptReport {
            imported: 1,
            conflicts: 2,
        })
        .unwrap()
        .unwrap();

        assert!(output.contains("Next Step"));
        assert!(output.contains("resolve conflicts"));
    }

    #[test]
    fn project_adopt_with_skill_only_adopts_that_skill() {
        let temp_dir = TempDir::new().unwrap();
        let paths = test_paths(&temp_dir);
        let project = Utf8PathBuf::from_path_buf(temp_dir.path().join("project")).unwrap();
        write_skill(
            &project.join(".agents/skills/frontend-design"),
            "# Frontend\n",
        );
        write_skill(&project.join(".agents/skills/other-skill"), "# Other\n");

        let report = project_adopt(
            &paths,
            ProjectAgentArgs {
                skill: Some("frontend-design".to_string()),
                agent: "codex".to_string(),
                project: Some(project),
            },
        )
        .unwrap();

        let registry = read_skills_registry(&paths).unwrap();
        let adopted_names = registry
            .skills
            .into_iter()
            .map(|skill| skill.name)
            .collect::<Vec<_>>();

        assert_eq!(report.imported, 1);
        assert_eq!(adopted_names, vec!["frontend-design".to_string()]);
    }

    #[test]
    fn project_status_table_uses_native_agent_space_columns() {
        let output = render_project_status(
            vec![SkillInstance {
                id: "instance".to_string(),
                name: "frontend-design".to_string(),
                agent_id: AgentId::new("codex"),
                scope: SkillInstanceScope::Project {
                    name: "project".to_string(),
                    path: "/tmp/project".into(),
                },
                skill_dir: "/tmp/project/.agents/skills/frontend-design".into(),
                enabled_path: "/tmp/project/.agents/skills/frontend-design/SKILL.md".into(),
                disabled_path: "/tmp/project/.agents/skills/frontend-design/SKILL.md.disabled"
                    .into(),
                toggle_state: ToggleState::Enabled,
                source_kind: SkillInstanceSourceKind::ProjectAgentSpace,
                writable: true,
                metadata: None,
                content_hash: Some("hash".to_string()),
                updated_at: Some("2026-06-02T00:00:00Z".to_string()),
            }],
            OutputFormat::Table,
        )
        .unwrap()
        .unwrap();

        assert!(output.contains("Skill Dir"));
        assert!(output.contains("Writable"));
        assert!(!output.contains("Missing Managed Source"));
        assert!(!output.contains("Outdated"));
        assert!(output.contains("true"));
    }

    #[test]
    fn project_status_table_reports_project_skill_dir() {
        let output = render_project_status(
            vec![SkillInstance {
                id: "instance".to_string(),
                name: "frontend-design".to_string(),
                agent_id: AgentId::new("codex"),
                scope: SkillInstanceScope::Project {
                    name: "project".to_string(),
                    path: "/tmp/project".into(),
                },
                skill_dir: "/tmp/project/.agents/skills/frontend-design".into(),
                enabled_path: "/tmp/project/.agents/skills/frontend-design/SKILL.md".into(),
                disabled_path: "/tmp/project/.agents/skills/frontend-design/SKILL.md.disabled"
                    .into(),
                toggle_state: ToggleState::Enabled,
                source_kind: SkillInstanceSourceKind::ProjectAgentSpace,
                writable: true,
                metadata: None,
                content_hash: Some("hash".to_string()),
                updated_at: Some("2026-06-02T00:00:00Z".to_string()),
            }],
            OutputFormat::Table,
        )
        .unwrap()
        .unwrap();

        assert!(output.contains("Skill Dir"));
        assert!(output.contains("/tmp/project/.agents/skills/frontend-design"));
    }

    #[test]
    fn native_toggle_rejects_plugin_package_and_capability_with_guidance() {
        let temp_dir = TempDir::new().unwrap();
        let paths = test_paths(&temp_dir);
        let home = Utf8PathBuf::from_path_buf(temp_dir.path().join("home")).unwrap();
        let package = home.join(".codex/plugins/cache/openai-bundled/browser/1.0.0");
        write_file(
            &package.join(".codex-plugin/plugin.json"),
            r#"{"name":"browser","display_name":"Browser"}"#,
        );
        write_skill(&package.join("skills/browser"), "# Browser Skill\n");

        let package_error = reject_native_plugin_target(&paths, &home, "browser@openai-bundled")
            .unwrap_err()
            .to_string();
        assert_eq!(
            package_error,
            "This is a plugin package. Use skill-kits plugin enable or skill-kits plugin disable."
        );

        let capability_error = reject_native_plugin_target(&paths, &home, "Browser Skill")
            .unwrap_err()
            .to_string();
        assert_eq!(
            capability_error,
            "This is a plugin-provided Skill. Disable or uninstall the parent plugin instead."
        );
    }
}
