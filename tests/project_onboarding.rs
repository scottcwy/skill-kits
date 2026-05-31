use camino::{Utf8Path, Utf8PathBuf};
use skill_kits::core::{
    agents::{AgentConfig, AgentKind},
    config::{read_config, Config, RecentProject},
    ids::{AgentId, SkillId},
    onboarding::{project_onboarding_scan, ProjectOnboardingScanRequest},
    paths::AppPaths,
    registry::{
        DeploymentRecord, DeploymentsRegistry, ManagedSkill, SkillSource, SkillsRegistry,
        ToggleState,
    },
};
use tempfile::TempDir;

fn test_paths(temp_dir: &TempDir) -> AppPaths {
    AppPaths::from_data_root(
        Utf8PathBuf::from_path_buf(temp_dir.path().join(".skill-kits")).unwrap(),
    )
}

fn write_toml<T: serde::Serialize>(path: &Utf8Path, value: &T) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, toml::to_string_pretty(value).unwrap()).unwrap();
}

fn write_skill(path: &Utf8Path, file_name: &str) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::write(path.join(file_name), "# Skill\n").unwrap();
}

fn config_with_agents(agents: Vec<AgentConfig>) -> Config {
    Config {
        agents,
        recent_projects: Vec::new(),
        ..Config::default()
    }
}

fn agent(id: &str, project_skill_dir: &str, enabled: bool) -> AgentConfig {
    AgentConfig {
        id: AgentId::new(id),
        label: id.to_string(),
        kind: AgentKind::Custom,
        global_skill_dirs: Vec::new(),
        project_skill_dirs: vec![project_skill_dir.into()],
        enabled,
    }
}

#[test]
fn scan_records_recent_project_and_reports_unmanaged_skill_dirs() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = Utf8PathBuf::from_path_buf(temp_dir.path().join("project")).unwrap();
    write_toml(
        &paths.config_file,
        &config_with_agents(vec![agent("codex", ".agents/skills", true)]),
    );
    write_toml(
        &paths.skills_registry_file,
        &SkillsRegistry {
            version: 1,
            skills: Vec::new(),
        },
    );
    write_toml(
        &paths.deployments_registry_file,
        &DeploymentsRegistry::default(),
    );
    write_skill(&project.join(".agents/skills/enabled-skill"), "SKILL.md");
    write_skill(
        &project.join(".agents/skills/disabled-skill"),
        "SKILL.md.disabled",
    );
    write_skill(
        &project.join(".agents/skills/not-a-skill/nested"),
        "SKILL.md",
    );

    let report = project_onboarding_scan(ProjectOnboardingScanRequest {
        app_paths: &paths,
        project_path: &project,
    })
    .unwrap();

    assert_eq!(report.project_path, project);
    assert_eq!(report.discovered.len(), 2);
    assert_eq!(report.discovered[0].agent_id, AgentId::new("codex"));
    assert_eq!(report.discovered[0].name, "disabled-skill");
    assert_eq!(
        report.discovered[0].path,
        project.join(".agents/skills/disabled-skill")
    );
    assert_eq!(report.discovered[0].toggle, ToggleState::Disabled);
    assert_eq!(report.discovered[1].agent_id, AgentId::new("codex"));
    assert_eq!(report.discovered[1].name, "enabled-skill");
    assert_eq!(
        report.discovered[1].path,
        project.join(".agents/skills/enabled-skill")
    );
    assert_eq!(report.discovered[1].toggle, ToggleState::Enabled);

    let config = read_config(&paths).unwrap();
    assert_eq!(config.recent_projects.len(), 1);
    assert_eq!(config.recent_projects[0].name, "project");
    assert_eq!(config.recent_projects[0].path, project);
}

#[test]
fn scan_does_not_include_existing_deployment_records() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = Utf8PathBuf::from_path_buf(temp_dir.path().join("project")).unwrap();
    let managed_path = paths.skills_dir.join("managed-skill-a1b2c3d4");
    write_toml(
        &paths.config_file,
        &config_with_agents(vec![agent("codex", ".agents/skills", true)]),
    );
    write_skill(&managed_path, "SKILL.md");
    let managed = ManagedSkill {
        id: SkillId::new("managed-skill-a1b2c3d4"),
        name: "managed-skill".to_string(),
        source: SkillSource::Local {
            source_path: managed_path.clone(),
        },
        managed_path,
        content_hash: "managed-hash".to_string(),
        metadata: None,
        created_at: "2026-05-31T00:00:00Z".to_string(),
        updated_at: "2026-05-31T00:00:00Z".to_string(),
    };
    write_toml(
        &paths.skills_registry_file,
        &SkillsRegistry {
            version: 1,
            skills: vec![managed],
        },
    );
    write_skill(&project.join(".agents/skills/managed-skill"), "SKILL.md");
    write_skill(&project.join(".agents/skills/local-only"), "SKILL.md");
    write_toml(
        &paths.deployments_registry_file,
        &DeploymentsRegistry {
            version: 1,
            deployments: vec![DeploymentRecord {
                id: "codex-managed-skill-a1b2c3d4-project".to_string(),
                skill_id: SkillId::new("managed-skill-a1b2c3d4"),
                agent_id: AgentId::new("codex"),
                project_name: "project".to_string(),
                project_path: project.clone(),
                deployment_path: project.join(".agents/skills/managed-skill"),
                skill_name: "managed-skill".to_string(),
                baseline_hash: "baseline".to_string(),
                deployed_from_hash: "managed-hash".to_string(),
                created_at: "2026-05-31T00:00:00Z".to_string(),
                updated_at: "2026-05-31T00:00:00Z".to_string(),
            }],
        },
    );

    let report = project_onboarding_scan(ProjectOnboardingScanRequest {
        app_paths: &paths,
        project_path: &project,
    })
    .unwrap();

    assert_eq!(report.discovered.len(), 1);
    assert_eq!(report.discovered[0].name, "local-only");
    assert_eq!(
        report.discovered[0].path,
        project.join(".agents/skills/local-only")
    );
}

#[test]
fn scan_ignores_disabled_agents_and_unrelated_recent_projects() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = Utf8PathBuf::from_path_buf(temp_dir.path().join("project")).unwrap();
    let other_project = Utf8PathBuf::from_path_buf(temp_dir.path().join("other-project")).unwrap();
    write_toml(
        &paths.config_file,
        &Config {
            agents: vec![
                agent("codex", ".agents/skills", true),
                agent("claude", ".claude/skills", false),
            ],
            recent_projects: vec![RecentProject {
                name: "other-project".to_string(),
                path: other_project.clone(),
                last_opened_at: "old".to_string(),
            }],
            ..Config::default()
        },
    );
    write_toml(
        &paths.skills_registry_file,
        &SkillsRegistry {
            version: 1,
            skills: Vec::new(),
        },
    );
    write_toml(
        &paths.deployments_registry_file,
        &DeploymentsRegistry::default(),
    );
    write_skill(&project.join(".agents/skills/project-skill"), "SKILL.md");
    write_skill(
        &project.join(".claude/skills/disabled-agent-skill"),
        "SKILL.md",
    );
    write_skill(
        &other_project.join(".agents/skills/other-skill"),
        "SKILL.md",
    );

    let report = project_onboarding_scan(ProjectOnboardingScanRequest {
        app_paths: &paths,
        project_path: &project,
    })
    .unwrap();

    assert_eq!(report.discovered.len(), 1);
    assert_eq!(report.discovered[0].name, "project-skill");

    let config = read_config(&paths).unwrap();
    assert_eq!(config.recent_projects.len(), 2);
    assert_eq!(config.recent_projects[0].path, project);
    assert_eq!(config.recent_projects[1].path, other_project);
}
