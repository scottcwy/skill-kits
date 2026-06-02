use camino::{Utf8Path, Utf8PathBuf};
use skill_kits::core::{
    agent_space::{
        disable_skill_instance, enable_skill_instance, scan_agent_spaces, SkillInstanceRequest,
        SkillInstanceSourceKind,
    },
    agents::{AgentConfig, AgentKind},
    config::{write_config, Config, RecentProject},
    hash::hash_skill_dir,
    ids::{AgentId, SkillId},
    paths::{ensure_app_dirs, AppPaths},
    registry::{
        write_deployments_registry, write_skills_registry, DeploymentRecord, DeploymentsRegistry,
        ManagedSkill, SkillSource, SkillsRegistry, ToggleState,
    },
    SkillKitsError,
};
use tempfile::TempDir;

fn test_paths(temp_dir: &TempDir) -> AppPaths {
    AppPaths::from_data_root(Utf8PathBuf::from_path_buf(temp_dir.path().join("data")).unwrap())
}

fn home_path(temp_dir: &TempDir) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(temp_dir.path().join("home")).unwrap()
}

fn write_skill_file(skill_dir: &Utf8Path, file_name: &str, body: &str) {
    std::fs::create_dir_all(skill_dir).unwrap();
    std::fs::write(skill_dir.join(file_name), body).unwrap();
}

fn write_config_for_codex(
    paths: &AppPaths,
    recent_projects: Vec<RecentProject>,
    global_dirs: Vec<Utf8PathBuf>,
) {
    write_config(
        paths,
        &Config {
            agents: vec![AgentConfig {
                id: AgentId::new("codex"),
                label: "Codex".to_string(),
                kind: AgentKind::BuiltIn,
                global_skill_dirs: global_dirs,
                project_skill_dirs: vec![".agents/skills".into()],
                enabled: true,
            }],
            recent_projects,
            ..Config::default()
        },
    )
    .unwrap();
}

fn instance_id_for(paths: &AppPaths, home: &Utf8Path, skill_dir: &Utf8Path) -> String {
    scan_agent_spaces(paths, home)
        .unwrap()
        .into_iter()
        .find(|instance| instance.skill_dir == skill_dir)
        .expect("skill instance")
        .id
}

fn request<'a>(
    paths: &'a AppPaths,
    home: &'a Utf8Path,
    instance_id: &'a str,
) -> SkillInstanceRequest<'a> {
    SkillInstanceRequest {
        app_paths: paths,
        home_dir: home,
        instance_id,
    }
}

fn managed_skill(name: &str, managed_path: Utf8PathBuf) -> ManagedSkill {
    ManagedSkill {
        id: SkillId::new(format!("{name}-managed")),
        name: name.to_string(),
        source: SkillSource::Local {
            source_path: managed_path.clone(),
        },
        managed_path,
        content_hash: "managed-hash".to_string(),
        metadata: None,
        created_at: "2026-06-01T00:00:00Z".to_string(),
        updated_at: "2026-06-01T00:00:00Z".to_string(),
    }
}

#[test]
fn disable_renames_only_selected_enabled_instance_without_deleting_directory() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config_for_codex(&paths, Vec::new(), vec!["~/.codex/skills".into()]);
    let skill_dir = home.join(".codex/skills/toggle-me");
    write_skill_file(&skill_dir, "SKILL.md", "# Toggle Me\n");
    std::fs::write(skill_dir.join("notes.txt"), "keep me").unwrap();
    let instance_id = instance_id_for(&paths, &home, &skill_dir);

    let instance = disable_skill_instance(request(&paths, &home, &instance_id)).unwrap();

    assert_eq!(instance.toggle_state, ToggleState::Disabled);
    assert!(skill_dir.is_dir());
    assert!(!skill_dir.join("SKILL.md").exists());
    assert!(skill_dir.join("SKILL.md.disabled").exists());
    assert_eq!(
        std::fs::read_to_string(skill_dir.join("notes.txt")).unwrap(),
        "keep me"
    );
}

#[test]
fn enable_reverses_disabled_instance_and_rescan_preserves_state() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config_for_codex(&paths, Vec::new(), vec!["~/.codex/skills".into()]);
    let skill_dir = home.join(".codex/skills/toggle-me");
    write_skill_file(&skill_dir, "SKILL.md.disabled", "# Toggle Me\n");
    let instance_id = instance_id_for(&paths, &home, &skill_dir);

    let instance = enable_skill_instance(request(&paths, &home, &instance_id)).unwrap();

    assert_eq!(instance.toggle_state, ToggleState::Enabled);
    assert!(skill_dir.join("SKILL.md").exists());
    assert!(!skill_dir.join("SKILL.md.disabled").exists());
    let rescanned = scan_agent_spaces(&paths, &home)
        .unwrap()
        .into_iter()
        .find(|instance| instance.id == instance_id)
        .unwrap();
    assert_eq!(rescanned.toggle_state, ToggleState::Enabled);
}

#[test]
fn toggle_does_not_mutate_managed_inventory_or_same_name_project_copy() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    let project = Utf8PathBuf::from_path_buf(temp_dir.path().join("sample-app")).unwrap();
    ensure_app_dirs(&paths).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    write_config_for_codex(
        &paths,
        vec![RecentProject {
            name: "sample-app".to_string(),
            path: project.clone(),
            last_opened_at: "2026-06-01T00:00:00Z".to_string(),
        }],
        vec!["~/.codex/skills".into()],
    );
    let global_skill = home.join(".codex/skills/same-name");
    write_skill_file(&global_skill, "SKILL.md", "# Global\n");
    let project_skill = project.join(".agents/skills/same-name");
    write_skill_file(&project_skill, "SKILL.md", "# Project\n");
    let managed_path = paths.skills_dir.join("same-name-managed");
    write_skill_file(&managed_path, "SKILL.md", "# Managed\n");
    let mut managed = managed_skill("same-name", managed_path.clone());
    managed.content_hash = hash_skill_dir(&managed_path).unwrap();
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![managed.clone()],
        },
    )
    .unwrap();
    let before_hash = hash_skill_dir(&managed_path).unwrap();
    let instance_id = instance_id_for(&paths, &home, &global_skill);

    disable_skill_instance(request(&paths, &home, &instance_id)).unwrap();

    assert!(global_skill.join("SKILL.md.disabled").exists());
    assert!(project_skill.join("SKILL.md").exists());
    assert!(!project_skill.join("SKILL.md.disabled").exists());
    assert!(managed_path.join("SKILL.md").exists());
    assert_eq!(hash_skill_dir(&managed_path).unwrap(), before_hash);
}

#[test]
fn project_deployment_instance_toggle_renames_only_that_project_copy() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    let project = Utf8PathBuf::from_path_buf(temp_dir.path().join("sample-app")).unwrap();
    ensure_app_dirs(&paths).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    write_config_for_codex(
        &paths,
        vec![RecentProject {
            name: "sample-app".to_string(),
            path: project.clone(),
            last_opened_at: "2026-06-01T00:00:00Z".to_string(),
        }],
        vec!["~/.codex/skills".into()],
    );
    let global_skill = home.join(".codex/skills/project-toggle");
    write_skill_file(&global_skill, "SKILL.md", "# Global copy\n");
    let project_skill = project.join(".agents/skills/project-toggle");
    write_skill_file(&project_skill, "SKILL.md", "# Project copy\n");
    let instance_id = instance_id_for(&paths, &home, &project_skill);

    let disabled = disable_skill_instance(request(&paths, &home, &instance_id)).unwrap();

    assert_eq!(disabled.toggle_state, ToggleState::Disabled);
    assert!(project_skill.join("SKILL.md.disabled").exists());
    assert!(!project_skill.join("SKILL.md").exists());
    assert!(global_skill.join("SKILL.md").exists());
    assert!(!global_skill.join("SKILL.md.disabled").exists());

    let enabled = enable_skill_instance(request(&paths, &home, &instance_id)).unwrap();

    assert_eq!(enabled.toggle_state, ToggleState::Enabled);
    assert!(project_skill.join("SKILL.md").exists());
    assert!(!project_skill.join("SKILL.md.disabled").exists());
}

#[test]
fn toggle_blocks_plugin_vendor_invalid_and_missing_instances() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    let project = Utf8PathBuf::from_path_buf(temp_dir.path().join("sample-app")).unwrap();
    ensure_app_dirs(&paths).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    write_config_for_codex(
        &paths,
        vec![RecentProject {
            name: "sample-app".to_string(),
            path: project.clone(),
            last_opened_at: "2026-06-01T00:00:00Z".to_string(),
        }],
        vec![
            "~/.codex/skills".into(),
            "~/.codex/plugins/cache".into(),
            "~/.codex/vendor_imports".into(),
        ],
    );

    let plugin = home.join(".codex/plugins/cache/openai/browser/skills/browser-skill");
    write_skill_file(&plugin, "SKILL.md", "# Plugin\n");
    let vendor = home.join(".codex/vendor_imports/vendor-one/skills/vendor-skill");
    write_skill_file(&vendor, "SKILL.md.disabled", "# Vendor\n");
    let invalid = home.join(".codex/skills/invalid-skill");
    write_skill_file(&invalid, "SKILL.md", "# Enabled\n");
    write_skill_file(&invalid, "SKILL.md.disabled", "# Disabled\n");
    let missing = project.join(".agents/skills/missing-skill");
    std::fs::create_dir_all(&missing).unwrap();
    write_deployments_registry(
        &paths,
        &DeploymentsRegistry {
            version: 1,
            deployments: vec![DeploymentRecord {
                id: "codex-missing-skill-sample-app".to_string(),
                skill_id: SkillId::new("missing-skill-managed"),
                agent_id: AgentId::new("codex"),
                project_name: "sample-app".to_string(),
                project_path: project,
                deployment_path: missing.clone(),
                skill_name: "missing-skill".to_string(),
                baseline_hash: "baseline".to_string(),
                deployed_from_hash: "source".to_string(),
                created_at: "2026-06-01T00:00:00Z".to_string(),
                updated_at: "2026-06-01T00:00:00Z".to_string(),
            }],
        },
    )
    .unwrap();

    let instances = scan_agent_spaces(&paths, &home).unwrap();
    for skill_dir in [&plugin, &vendor, &invalid, &missing] {
        let instance = instances
            .iter()
            .find(|instance| instance.skill_dir == *skill_dir)
            .expect("blocked instance");
        let err = disable_skill_instance(request(&paths, &home, &instance.id)).unwrap_err();
        assert!(matches!(err, SkillKitsError::InvalidToggleState { .. }));
    }

    let plugin_instance = instances
        .iter()
        .find(|instance| instance.skill_dir == plugin)
        .unwrap();
    assert_eq!(
        plugin_instance.source_kind,
        SkillInstanceSourceKind::PluginCache
    );
    assert!(!plugin.join("SKILL.md.disabled").exists());
}
