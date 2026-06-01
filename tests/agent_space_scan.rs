use camino::{Utf8Path, Utf8PathBuf};
use skill_kits::core::{
    agent_space::{
        scan_agent_spaces, SkillInstanceScope, SkillInstanceSourceKind, AGENT_SPACE_PLUGIN_DEPTH,
    },
    agents::{AgentConfig, AgentKind},
    config::{write_config, Config, RecentProject},
    hash::hash_skill_dir,
    ids::AgentId,
    paths::{ensure_app_dirs, AppPaths},
    registry::{
        write_deployments_registry, write_skills_registry, DeploymentRecord, DeploymentsRegistry,
        SkillsRegistry, ToggleState,
    },
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

fn write_test_config(paths: &AppPaths, home: &Utf8Path, recent_projects: Vec<RecentProject>) {
    write_config(
        paths,
        &Config {
            agents: vec![AgentConfig {
                id: AgentId::new("codex"),
                label: "Codex".to_string(),
                kind: AgentKind::BuiltIn,
                global_skill_dirs: vec![
                    "~/.codex/skills".into(),
                    "~/.codex/plugins/cache".into(),
                    "~/.codex/vendor_imports".into(),
                ],
                project_skill_dirs: vec![".agents/skills".into()],
                enabled: true,
            }],
            recent_projects,
            ..Config::default()
        },
    )
    .unwrap();
    std::fs::create_dir_all(home).unwrap();
}

#[test]
fn scan_agent_spaces_finds_enabled_and_disabled_global_instances() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_test_config(&paths, &home, Vec::new());

    let enabled = home.join(".codex/skills/enabled-skill");
    write_skill_file(
        &enabled,
        "SKILL.md",
        "+++\ntitle = \"Enabled Title\"\ndescription = \"Reads from SKILL.md.\"\n+++\n",
    );
    let disabled = home.join(".codex/skills/disabled-skill");
    write_skill_file(
        &disabled,
        "SKILL.md.disabled",
        "+++\ntitle = \"Disabled Title\"\ndescription = \"Reads from disabled file.\"\n+++\n",
    );

    let instances = scan_agent_spaces(&paths, &home).unwrap();

    let enabled_instance = instances
        .iter()
        .find(|instance| instance.skill_dir == enabled)
        .expect("enabled instance");
    assert_eq!(enabled_instance.name, "Enabled Title");
    assert_eq!(enabled_instance.agent_id, AgentId::new("codex"));
    assert_eq!(enabled_instance.scope, SkillInstanceScope::Global);
    assert_eq!(enabled_instance.toggle_state, ToggleState::Enabled);
    assert_eq!(
        enabled_instance.source_kind,
        SkillInstanceSourceKind::AgentSpace
    );
    assert!(enabled_instance.writable);
    assert_eq!(
        enabled_instance
            .metadata
            .as_ref()
            .unwrap()
            .description
            .as_deref(),
        Some("Reads from SKILL.md.")
    );
    assert!(enabled_instance.content_hash.is_some());

    let disabled_instance = instances
        .iter()
        .find(|instance| instance.skill_dir == disabled)
        .expect("disabled instance");
    assert_eq!(disabled_instance.name, "Disabled Title");
    assert_eq!(disabled_instance.toggle_state, ToggleState::Disabled);
    assert_eq!(
        disabled_instance
            .metadata
            .as_ref()
            .unwrap()
            .description
            .as_deref(),
        Some("Reads from disabled file.")
    );
    assert!(disabled_instance.content_hash.is_some());
}

#[test]
fn disabled_hash_matches_enabled_hash_after_toggle_filename_normalization() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_test_config(&paths, &home, Vec::new());

    let skill_dir = home.join(".codex/skills/toggle-stable");
    let body = "# Toggle Stable\n\nsame content\n";
    write_skill_file(&skill_dir, "SKILL.md.disabled", body);
    let disabled_hash = scan_agent_spaces(&paths, &home)
        .unwrap()
        .into_iter()
        .find(|instance| instance.skill_dir == skill_dir)
        .unwrap()
        .content_hash
        .unwrap();

    std::fs::rename(
        skill_dir.join("SKILL.md.disabled"),
        skill_dir.join("SKILL.md"),
    )
    .unwrap();
    let enabled_hash = hash_skill_dir(&skill_dir).unwrap();

    assert_eq!(disabled_hash, enabled_hash);
}

#[test]
fn both_present_is_strict_invalid_without_metadata_or_hash() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_test_config(&paths, &home, Vec::new());

    let conflict = home.join(".codex/skills/conflict-skill");
    write_skill_file(
        &conflict,
        "SKILL.md",
        "+++\ntitle = \"Enabled Side\"\n+++\n",
    );
    write_skill_file(
        &conflict,
        "SKILL.md.disabled",
        "+++\ntitle = \"Disabled Side\"\n+++\n",
    );

    let instance = scan_agent_spaces(&paths, &home)
        .unwrap()
        .into_iter()
        .find(|instance| instance.skill_dir == conflict)
        .expect("conflict instance");

    assert_eq!(instance.name, "conflict-skill");
    assert_eq!(instance.toggle_state, ToggleState::InvalidBothPresent);
    assert!(instance.metadata.is_none());
    assert!(instance.content_hash.is_none());
    assert!(!instance.writable);
}

#[test]
fn legacy_deployment_without_toggle_file_becomes_missing_instance() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    let project = Utf8PathBuf::from_path_buf(temp_dir.path().join("sample-app")).unwrap();
    ensure_app_dirs(&paths).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    write_test_config(
        &paths,
        &home,
        vec![RecentProject {
            name: "sample-app".to_string(),
            path: project.clone(),
            last_opened_at: "2026-06-01T00:00:00Z".to_string(),
        }],
    );
    let missing_dir = project.join(".agents/skills/missing-skill");
    std::fs::create_dir_all(&missing_dir).unwrap();
    write_deployments_registry(
        &paths,
        &DeploymentsRegistry {
            version: 1,
            deployments: vec![DeploymentRecord {
                id: "codex-missing-skill-sample-app".to_string(),
                skill_id: "missing-skill-12345678".into(),
                agent_id: AgentId::new("codex"),
                project_name: "sample-app".to_string(),
                project_path: project.clone(),
                deployment_path: missing_dir.clone(),
                skill_name: "missing-skill".to_string(),
                baseline_hash: "baseline".to_string(),
                deployed_from_hash: "source".to_string(),
                created_at: "2026-06-01T00:00:00Z".to_string(),
                updated_at: "2026-06-01T00:00:00Z".to_string(),
            }],
        },
    )
    .unwrap();

    let instance = scan_agent_spaces(&paths, &home)
        .unwrap()
        .into_iter()
        .find(|instance| instance.skill_dir == missing_dir)
        .expect("missing legacy instance");

    assert_eq!(instance.name, "missing-skill");
    assert_eq!(instance.toggle_state, ToggleState::InvalidBothMissing);
    assert_eq!(
        instance.source_kind,
        SkillInstanceSourceKind::ProjectDeployment
    );
    assert_eq!(
        instance.scope,
        SkillInstanceScope::Project {
            name: "sample-app".to_string(),
            path: project
        }
    );
    assert!(!instance.writable);
    assert!(instance.content_hash.is_none());
}

#[test]
fn plugin_cache_and_vendor_are_bounded_recursive_read_only_instances() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_test_config(&paths, &home, Vec::new());

    let plugin_skill = home.join(".codex/plugins/cache/openai/example/skills/plugin-skill");
    write_skill_file(&plugin_skill, "SKILL.md", "# Plugin Skill\n");
    let too_deep = home.join(format!(
        ".codex/plugins/cache/{}",
        ["a"; AGENT_SPACE_PLUGIN_DEPTH + 2].join("/")
    ));
    write_skill_file(&too_deep, "SKILL.md", "# Too Deep\n");
    let vendor_skill = home.join(".codex/vendor_imports/vendor-one/nested/vendor-skill");
    write_skill_file(&vendor_skill, "SKILL.md.disabled", "# Vendor Skill\n");

    let instances = scan_agent_spaces(&paths, &home).unwrap();

    let plugin = instances
        .iter()
        .find(|instance| instance.skill_dir == plugin_skill)
        .expect("plugin instance");
    assert_eq!(plugin.source_kind, SkillInstanceSourceKind::PluginCache);
    assert_eq!(plugin.toggle_state, ToggleState::Enabled);
    assert!(!plugin.writable);

    let vendor = instances
        .iter()
        .find(|instance| instance.skill_dir == vendor_skill)
        .expect("vendor instance");
    assert_eq!(vendor.source_kind, SkillInstanceSourceKind::Vendor);
    assert_eq!(vendor.toggle_state, ToggleState::Disabled);
    assert!(!vendor.writable);

    assert!(
        instances
            .iter()
            .all(|instance| instance.skill_dir != too_deep),
        "recursive plugin scan must stop at depth {AGENT_SPACE_PLUGIN_DEPTH}"
    );
}

#[test]
fn plugin_cache_classification_uses_exact_path_components() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    let root_with_similar_name = home.join("plugins/cache-data/skills");
    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![AgentConfig {
                id: AgentId::new("custom"),
                label: "Custom".to_string(),
                kind: AgentKind::Custom,
                global_skill_dirs: vec![root_with_similar_name.clone()],
                project_skill_dirs: vec![".custom/skills".into()],
                enabled: true,
            }],
            recent_projects: Vec::new(),
            ..Config::default()
        },
    )
    .unwrap();
    let skill_dir = root_with_similar_name.join("normal-skill");
    write_skill_file(&skill_dir, "SKILL.md", "# Normal Skill\n");

    let instance = scan_agent_spaces(&paths, &home)
        .unwrap()
        .into_iter()
        .find(|instance| instance.skill_dir == skill_dir)
        .expect("normal instance");

    assert_eq!(instance.source_kind, SkillInstanceSourceKind::AgentSpace);
    assert!(instance.writable);
}

#[test]
fn vendor_classification_uses_codex_vendor_imports_component() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    let similar_root = home.join("custom-agent/vendor_imports/skills");
    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![AgentConfig {
                id: AgentId::new("custom"),
                label: "Custom".to_string(),
                kind: AgentKind::Custom,
                global_skill_dirs: vec![similar_root.clone()],
                project_skill_dirs: vec![".custom/skills".into()],
                enabled: true,
            }],
            recent_projects: Vec::new(),
            ..Config::default()
        },
    )
    .unwrap();
    let skill_dir = similar_root.join("normal-skill");
    write_skill_file(&skill_dir, "SKILL.md", "# Normal Skill\n");

    let instance = scan_agent_spaces(&paths, &home)
        .unwrap()
        .into_iter()
        .find(|instance| instance.skill_dir == skill_dir)
        .expect("normal instance");

    assert_eq!(instance.source_kind, SkillInstanceSourceKind::AgentSpace);
    assert!(instance.writable);
}

#[test]
fn recent_projects_are_scanned_as_project_deployments() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    let project = Utf8PathBuf::from_path_buf(temp_dir.path().join("sample-app")).unwrap();
    ensure_app_dirs(&paths).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    write_test_config(
        &paths,
        &home,
        vec![RecentProject {
            name: "sample-app".to_string(),
            path: project.clone(),
            last_opened_at: "2026-06-01T00:00:00Z".to_string(),
        }],
    );
    let project_skill = project.join(".agents/skills/project-skill");
    write_skill_file(&project_skill, "SKILL.md", "# Project Skill\n");

    let instances = scan_agent_spaces(&paths, &home).unwrap();

    let instance = instances
        .iter()
        .find(|instance| instance.skill_dir == project_skill)
        .expect("project instance");
    assert_eq!(
        instance.source_kind,
        SkillInstanceSourceKind::ProjectDeployment
    );
    assert_eq!(
        instance.scope,
        SkillInstanceScope::Project {
            name: "sample-app".to_string(),
            path: project
        }
    );
    assert_eq!(instance.toggle_state, ToggleState::Enabled);
    assert!(instance.writable);
}

#[test]
fn same_physical_skill_dir_declared_by_two_agents_yields_two_instances() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    let shared_root = home.join("shared/skills");
    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![
                AgentConfig {
                    id: AgentId::new("codex"),
                    label: "Codex".to_string(),
                    kind: AgentKind::BuiltIn,
                    global_skill_dirs: vec![shared_root.clone()],
                    project_skill_dirs: vec![".agents/skills".into()],
                    enabled: true,
                },
                AgentConfig {
                    id: AgentId::new("custom"),
                    label: "Custom".to_string(),
                    kind: AgentKind::Custom,
                    global_skill_dirs: vec![shared_root.clone()],
                    project_skill_dirs: vec![".custom/skills".into()],
                    enabled: true,
                },
            ],
            recent_projects: Vec::new(),
            ..Config::default()
        },
    )
    .unwrap();
    let skill_dir = shared_root.join("shared-skill");
    write_skill_file(&skill_dir, "SKILL.md", "# Shared Skill\n");

    let instances = scan_agent_spaces(&paths, &home).unwrap();
    let matching = instances
        .iter()
        .filter(|instance| instance.skill_dir == skill_dir)
        .collect::<Vec<_>>();

    assert_eq!(matching.len(), 2);
    assert_ne!(matching[0].id, matching[1].id);
    assert!(matching
        .iter()
        .any(|instance| instance.agent_id == AgentId::new("codex")));
    assert!(matching
        .iter()
        .any(|instance| instance.agent_id == AgentId::new("custom")));
}

#[test]
fn agent_space_scan_does_not_write_registry_files() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_test_config(&paths, &home, Vec::new());
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    let skills_before = std::fs::read_to_string(&paths.skills_registry_file).unwrap();
    let deployments_before = std::fs::read_to_string(&paths.deployments_registry_file).unwrap();

    let skill_dir = home.join(".codex/skills/unmanaged");
    write_skill_file(&skill_dir, "SKILL.md", "# Unmanaged\n");
    let _instances = scan_agent_spaces(&paths, &home).unwrap();

    assert_eq!(
        std::fs::read_to_string(&paths.skills_registry_file).unwrap(),
        skills_before
    );
    assert_eq!(
        std::fs::read_to_string(&paths.deployments_registry_file).unwrap(),
        deployments_before
    );
}
