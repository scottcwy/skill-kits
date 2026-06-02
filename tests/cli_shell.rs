use assert_cmd::Command as AssertCommand;
use clap::Parser;
use serde::Serialize;
use skill_kits::cli::args::{
    Cli, Command, InstallCommand, ListKind, OutputFormat, PluginCommand, ProjectCommand,
    ProjectRedeployArgs, ProjectRemoveArgs, ProjectSkillAgentArgs, ProjectStatusArgs,
};
use skill_kits::cli::handlers::exit_code_for_error;
use skill_kits::cli::output::{format_table, TableColumn};
use skill_kits::core::{SkillKitsError, SkillSource};

#[test]
fn clap_defaults_to_status_for_plain_cli() {
    let cli = Cli::parse_from(["skill-kits"]);

    assert!(cli.command.is_none());
}

#[test]
fn clap_parses_top_level_gui_without_command() {
    let cli = Cli::parse_from(["skill-kits", "--gui"]);

    assert!(cli.gui);
    assert!(cli.command.is_none());
}

#[test]
fn clap_parses_documented_commands() {
    assert!(matches!(
        Cli::parse_from(["skill-kits", "list"]).command,
        Some(Command::List {
            kind: ListKind::Skill,
            format: OutputFormat::Table
        })
    ));
    assert!(matches!(
        Cli::parse_from(["skill-kits", "status", "--format", "json"]).command,
        Some(Command::Status {
            format: OutputFormat::Json
        })
    ));
    assert!(matches!(
        Cli::parse_from(["skill-kits", "install", "local", "/tmp/skill"]).command,
        Some(Command::Install {
            command: InstallCommand::Local { path }
        }) if path == "/tmp/skill"
    ));
    assert!(matches!(
        Cli::parse_from(["skill-kits", "uninstall", "frontend-design"]).command,
        Some(Command::Uninstall { skill }) if skill == "frontend-design"
    ));
    assert!(matches!(
        Cli::parse_from(["skill-kits", "enable", "instance-id"]).command,
        Some(Command::Enable { query }) if query == "instance-id"
    ));
    assert!(matches!(
        Cli::parse_from(["skill-kits", "disable", "instance-id"]).command,
        Some(Command::Disable { query }) if query == "instance-id"
    ));
    assert!(matches!(
        Cli::parse_from(["skill-kits", "scan", "frontend-design", "--format", "json"]).command,
        Some(Command::Scan {
            skill: Some(skill),
            format: OutputFormat::Json,
        }) if skill == "frontend-design"
    ));
    assert!(matches!(
        Cli::parse_from(["skill-kits", "doctor", "--fix"]).command,
        Some(Command::Doctor { fix: true })
    ));
    assert!(matches!(
        Cli::parse_from(["skill-kits", "adopt", "--global-agent", "codex"]).command,
        Some(Command::Adopt { global_agent }) if global_agent == "codex"
    ));
}

#[test]
fn clap_parses_plugin_list_kinds_and_commands() {
    assert!(matches!(
        Cli::parse_from(["skill-kits", "list", "--kind", "plugin"]).command,
        Some(Command::List {
            kind: ListKind::Plugin,
            format: OutputFormat::Table
        })
    ));
    assert!(matches!(
        Cli::parse_from([
            "skill-kits",
            "list",
            "--kind",
            "runtime-capability",
            "--format",
            "json"
        ])
        .command,
        Some(Command::List {
            kind: ListKind::RuntimeCapability,
            format: OutputFormat::Json
        })
    ));
    assert!(matches!(
        Cli::parse_from(["skill-kits", "plugin", "list"]).command,
        Some(Command::Plugin {
            command: PluginCommand::List {
                format: OutputFormat::Table
            }
        })
    ));
    assert!(matches!(
        Cli::parse_from(["skill-kits", "plugin", "status", "browser@openai-bundled"]).command,
        Some(Command::Plugin {
            command: PluginCommand::Status { query, format: OutputFormat::Table }
        }) if query == "browser@openai-bundled"
    ));
    assert!(matches!(
        Cli::parse_from(["skill-kits", "plugin", "enable", "browser@openai-bundled"]).command,
        Some(Command::Plugin {
            command: PluginCommand::Enable { query }
        }) if query == "browser@openai-bundled"
    ));
    assert!(matches!(
        Cli::parse_from(["skill-kits", "plugin", "disable", "browser@openai-bundled"]).command,
        Some(Command::Plugin {
            command: PluginCommand::Disable { query }
        }) if query == "browser@openai-bundled"
    ));
    assert!(matches!(
        Cli::parse_from(["skill-kits", "plugin", "scan", "--format", "json"]).command,
        Some(Command::Plugin {
            command: PluginCommand::Scan {
                format: OutputFormat::Json
            }
        })
    ));
}

#[test]
fn clap_parses_project_commands() {
    assert!(matches!(
        Cli::parse_from([
            "skill-kits",
            "project",
            "status",
            "--project",
            "/tmp/app",
            "--format",
            "json",
        ])
        .command,
        Some(Command::Project {
            command: ProjectCommand::Status(ProjectStatusArgs {
                project: Some(project),
                format: OutputFormat::Json,
            })
        }) if project == "/tmp/app"
    ));
    assert!(matches!(
        Cli::parse_from(["skill-kits", "project", "adopt", "--agent", "codex"]).command,
        Some(Command::Project {
            command: ProjectCommand::Adopt(args)
        }) if args.skill.is_none() && args.agent == "codex" && args.project.is_none()
    ));
    assert!(matches!(
        Cli::parse_from([
            "skill-kits",
            "project",
            "adopt",
            "frontend-design",
            "--agent",
            "codex",
            "--project",
            "/tmp/app",
        ])
        .command,
        Some(Command::Project {
            command: ProjectCommand::Adopt(args)
        }) if args.skill.as_deref() == Some("frontend-design")
            && args.agent == "codex"
            && args.project.as_deref() == Some(camino::Utf8Path::new("/tmp/app"))
    ));
    assert!(matches!(
        Cli::parse_from([
            "skill-kits",
            "project",
            "deploy",
            "frontend-design",
            "--agent",
            "codex",
        ])
        .command,
        Some(Command::Project {
            command: ProjectCommand::Deploy(ProjectSkillAgentArgs {
                skill,
                agent,
                project: None,
            })
        }) if skill == "frontend-design" && agent == "codex"
    ));
    assert!(matches!(
        Cli::parse_from(["skill-kits", "project", "enable", "skill", "--agent", "codex"])
            .command,
        Some(Command::Project {
            command: ProjectCommand::Enable(ProjectSkillAgentArgs { skill, agent, .. })
        }) if skill == "skill" && agent == "codex"
    ));
    assert!(matches!(
        Cli::parse_from(["skill-kits", "project", "disable", "skill", "--agent", "codex"])
            .command,
        Some(Command::Project {
            command: ProjectCommand::Disable(ProjectSkillAgentArgs { skill, agent, .. })
        }) if skill == "skill" && agent == "codex"
    ));
    assert!(matches!(
        Cli::parse_from([
            "skill-kits",
            "project",
            "redeploy",
            "skill",
            "--agent",
            "codex",
            "--overwrite",
        ])
        .command,
        Some(Command::Project {
            command: ProjectCommand::Redeploy(ProjectRedeployArgs {
                skill,
                agent,
                overwrite: true,
                promote: false,
                ..
            })
        }) if skill == "skill" && agent == "codex"
    ));
    assert!(matches!(
        Cli::parse_from([
            "skill-kits",
            "project",
            "remove",
            "skill",
            "--agent",
            "codex",
            "--force",
        ])
        .command,
        Some(Command::Project {
            command: ProjectCommand::Remove(ProjectRemoveArgs {
                skill,
                agent,
                force: true,
                ..
            })
        }) if skill == "skill" && agent == "codex"
    ));
}

#[test]
fn clap_reports_help_and_version() {
    let help = Cli::try_parse_from(["skill-kits", "--help"]).unwrap_err();
    assert_eq!(help.kind(), clap::error::ErrorKind::DisplayHelp);

    let version = Cli::try_parse_from(["skill-kits", "--version"]).unwrap_err();
    assert_eq!(version.kind(), clap::error::ErrorKind::DisplayVersion);
}

#[test]
fn binary_reports_version() {
    AssertCommand::cargo_bin("skill-kits")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn clap_rejects_yaml_output() {
    let err = Cli::try_parse_from(["skill-kits", "list", "--format", "yaml"]).unwrap_err();

    assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
}

#[test]
fn json_output_is_valid() {
    #[derive(Serialize)]
    struct Row {
        skill: &'static str,
        risk_count: usize,
    }

    let json = skill_kits::cli::output::to_json(&Row {
        skill: "frontend-design",
        risk_count: 0,
    })
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["skill"], "frontend-design");
    assert_eq!(parsed["risk_count"], 0);
}

#[test]
fn table_output_contains_expected_columns() {
    let table = format_table(
        &["Skill", "Status", "Risks"],
        &[vec![
            TableColumn::from("frontend-design"),
            TableColumn::from("managed"),
            TableColumn::from(0usize),
        ]],
    );

    assert!(table.contains("Skill"));
    assert!(table.contains("Status"));
    assert!(table.contains("Risks"));
    assert!(table.contains("frontend-design"));
}

#[test]
fn conflict_maps_to_exit_code_3() {
    let code = exit_code_for_error(&SkillKitsError::DeployConflict {
        target: "/tmp/project/.agents/skills/frontend-design".into(),
    });

    assert_eq!(code, 3);
}

#[test]
fn registry_busy_maps_to_exit_code_4() {
    assert_eq!(exit_code_for_error(&SkillKitsError::RegistryBusy), 4);
}

#[test]
fn blocked_operation_errors_map_to_exit_code_3() {
    assert_eq!(
        exit_code_for_error(&SkillKitsError::MissingManagedSource {
            skill_id: "missing".into(),
            deployment_id: "deployment".to_string(),
        }),
        3
    );
    assert_eq!(
        exit_code_for_error(&SkillKitsError::InvalidToggleState {
            path: "/tmp/project/.agents/skills/skill".into(),
        }),
        3
    );
    assert_eq!(
        exit_code_for_error(&SkillKitsError::AmbiguousSkill {
            query: "skill".to_string(),
            matches: vec!["one".into(), "two".into()],
        }),
        3
    );
    assert_eq!(
        exit_code_for_error(&SkillKitsError::InvalidSkillDir {
            path: "/tmp/not-a-skill".into(),
            reason: "missing SKILL.md".to_string(),
        }),
        1
    );
}

#[test]
fn skill_not_found_and_project_not_found_are_general_errors() {
    assert_eq!(
        exit_code_for_error(&SkillKitsError::SkillNotFound {
            query: "missing".to_string(),
        }),
        1
    );
    assert_eq!(
        exit_code_for_error(&SkillKitsError::ProjectNotFound {
            path: "/tmp/missing-project".into(),
        }),
        1
    );
}

#[test]
fn project_redeploy_overwrite_and_promote_conflict_is_invalid_args() {
    let err = Cli::try_parse_from([
        "skill-kits",
        "project",
        "redeploy",
        "skill",
        "--agent",
        "codex",
        "--overwrite",
        "--promote",
    ])
    .unwrap_err();

    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn skill_source_json_shape_is_stable_for_cli() {
    let source = SkillSource::Local {
        source_path: "/tmp/source".into(),
    };
    let json = skill_kits::cli::output::to_json(&source).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["kind"], "local");
    assert_eq!(parsed["source_path"], "/tmp/source");
}
