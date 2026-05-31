use assert_cmd::Command;
use camino::{Utf8Path, Utf8PathBuf};
use skill_kits::core::{
    config::{Config, RecentProject},
    doctor::{run_doctor, DoctorIssueCode, DoctorSeverity},
    ids::{AgentId, SkillId},
    paths::{ensure_app_dirs, AppPaths},
    registry::{DeploymentRecord, DeploymentsRegistry, ManagedSkill, SkillSource, SkillsRegistry},
    status::{global_status, HealthState},
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

fn write_skill(path: &Utf8Path, body: &str) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::write(path.join("SKILL.md"), body).unwrap();
}

fn managed_skill(paths: &AppPaths, id: &str, name: &str) -> ManagedSkill {
    ManagedSkill {
        id: SkillId::new(id),
        name: name.to_string(),
        source: SkillSource::Local {
            source_path: paths.skills_dir.join(id),
        },
        managed_path: paths.skills_dir.join(id),
        content_hash: "hash".to_string(),
        metadata: None,
        created_at: "2026-05-31T00:00:00Z".to_string(),
        updated_at: "2026-05-31T00:00:00Z".to_string(),
    }
}

fn deployment_record(project: &Utf8Path, skill_id: &str, skill_name: &str) -> DeploymentRecord {
    DeploymentRecord {
        id: format!("codex-{skill_id}-abc12345"),
        skill_id: SkillId::new(skill_id),
        agent_id: AgentId::new("codex"),
        project_name: "project".to_string(),
        project_path: project.to_path_buf(),
        deployment_path: project.join(".agents/skills").join(skill_name),
        skill_name: skill_name.to_string(),
        baseline_hash: "baseline".to_string(),
        deployed_from_hash: "hash".to_string(),
        created_at: "2026-05-31T00:00:00Z".to_string(),
        updated_at: "2026-05-31T00:00:00Z".to_string(),
    }
}

#[test]
fn doctor_reports_invalid_toml_missing_managed_directory_invalid_toggle_and_stale_lock() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    std::fs::write(&paths.config_file, "version = [").unwrap();
    write_toml(
        &paths.skills_registry_file,
        &SkillsRegistry {
            version: 1,
            skills: vec![managed_skill(&paths, "missing-a1b2c3d4", "missing")],
        },
    );
    let project = Utf8PathBuf::from_path_buf(temp_dir.path().join("project")).unwrap();
    write_skill(&project.join(".agents/skills/broken"), "# Enabled\n");
    std::fs::write(
        project.join(".agents/skills/broken/SKILL.md.disabled"),
        "# Disabled\n",
    )
    .unwrap();
    write_toml(
        &paths.deployments_registry_file,
        &DeploymentsRegistry {
            version: 1,
            deployments: vec![deployment_record(&project, "missing-a1b2c3d4", "broken")],
        },
    );
    std::fs::write(&paths.state_lock, "pid=999999").unwrap();

    let report = run_doctor(&paths, false).unwrap();

    assert!(!report.ok);
    assert!(report.has_errors());
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == DoctorIssueCode::InvalidToml));
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == DoctorIssueCode::MissingManagedDirectory));
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == DoctorIssueCode::InvalidToggle));
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == DoctorIssueCode::StaleLock));
}

#[test]
fn doctor_reports_missing_managed_source_without_fixing_it() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    let project = Utf8PathBuf::from_path_buf(temp_dir.path().join("project")).unwrap();
    write_skill(&project.join(".agents/skills/orphan"), "# Orphan\n");
    write_toml(&paths.skills_registry_file, &SkillsRegistry::default());
    write_toml(
        &paths.deployments_registry_file,
        &DeploymentsRegistry {
            version: 1,
            deployments: vec![deployment_record(&project, "orphan-a1b2c3d4", "orphan")],
        },
    );

    let report = run_doctor(&paths, true).unwrap();

    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == DoctorIssueCode::MissingManagedSource));
    assert!(project.join(".agents/skills/orphan/SKILL.md").exists());
    assert_eq!(report.fixed_count, 0);
}

#[test]
fn doctor_fix_removes_stale_lock_missing_recent_project_and_temp_files_only() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    let existing_project = Utf8PathBuf::from_path_buf(temp_dir.path().join("existing")).unwrap();
    std::fs::create_dir_all(&existing_project).unwrap();
    let missing_project = Utf8PathBuf::from_path_buf(temp_dir.path().join("missing")).unwrap();
    write_toml(
        &paths.config_file,
        &Config {
            recent_projects: vec![
                RecentProject {
                    name: "existing".to_string(),
                    path: existing_project.clone(),
                    last_opened_at: "2026-05-31T00:00:00Z".to_string(),
                },
                RecentProject {
                    name: "missing".to_string(),
                    path: missing_project,
                    last_opened_at: "2026-05-31T00:00:00Z".to_string(),
                },
            ],
            ..Config::default()
        },
    );
    write_toml(&paths.skills_registry_file, &SkillsRegistry::default());
    write_toml(
        &paths.deployments_registry_file,
        &DeploymentsRegistry::default(),
    );
    std::fs::write(&paths.state_lock, "pid=999999").unwrap();
    std::fs::write(paths.registry_dir.join("skills.toml.tmp"), "partial").unwrap();
    let project_copy = existing_project.join(".agents/skills/keep");
    write_skill(&project_copy, "# Keep\n");

    let report = run_doctor(&paths, true).unwrap();

    assert!(report
        .issues
        .iter()
        .any(|issue| issue.severity == DoctorSeverity::Fixed
            && issue.code == DoctorIssueCode::StaleLock));
    assert!(!paths.state_lock.exists());
    assert!(!paths.registry_dir.join("skills.toml.tmp").exists());
    assert!(project_copy.join("SKILL.md").exists());
    let config = skill_kits::core::config::read_config(&paths).unwrap();
    assert_eq!(config.recent_projects.len(), 1);
    assert_eq!(config.recent_projects[0].path, existing_project);
}

#[test]
fn global_status_includes_expected_counts_and_health() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_skill(&paths.skills_dir.join("one-a1b2c3d4"), "# One\n");
    write_toml(
        &paths.skills_registry_file,
        &SkillsRegistry {
            version: 1,
            skills: vec![managed_skill(&paths, "one-a1b2c3d4", "one")],
        },
    );
    write_toml(
        &paths.deployments_registry_file,
        &DeploymentsRegistry::default(),
    );
    let project = Utf8PathBuf::from_path_buf(temp_dir.path().join("project")).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    write_toml(
        &paths.config_file,
        &Config {
            recent_projects: vec![RecentProject {
                name: "project".to_string(),
                path: project,
                last_opened_at: "2026-05-31T00:00:00Z".to_string(),
            }],
            ..Config::default()
        },
    );

    let status = global_status(&paths).unwrap();

    assert_eq!(status.managed_skill_count, 1);
    assert_eq!(status.agent_count, 3);
    assert_eq!(status.enabled_agent_count, 3);
    assert_eq!(status.recent_project_count, 1);
    assert_eq!(status.registry_health, HealthState::Ok);
    assert_eq!(status.lock_health, HealthState::Ok);
    assert_eq!(status.cache_health, HealthState::Ok);
    assert_eq!(status.risk_count, 0);
}

#[test]
fn cli_doctor_exits_5_when_issues_remain() {
    let temp_dir = TempDir::new().unwrap();
    let data_root = temp_dir.path().join(".skill-kits");
    std::fs::create_dir_all(&data_root).unwrap();
    std::fs::write(data_root.join("config.toml"), "version = [").unwrap();

    Command::cargo_bin("skill-kits")
        .unwrap()
        .env("HOME", temp_dir.path())
        .arg("doctor")
        .assert()
        .code(5);
}
