use skill_kits::core::agents::{
    add_custom_agent_config, configured_global_skill_dirs_for, remove_custom_agent_config,
    reset_agent_project_skill_dirs, update_agent_project_skill_dirs, AgentConfig, AgentKind,
};
use skill_kits::core::config::{read_config, write_config, Config};
use skill_kits::core::error::SkillKitsError;
use skill_kits::core::ids::AgentId;
use skill_kits::core::lock::StateLock;
use skill_kits::core::paths::{ensure_app_dirs, AppPaths};
use skill_kits::core::registry::{
    read_deployments_registry, read_skills_registry, update_registry_files, update_skills_registry,
    write_deployments_registry, write_skills_registry, DeploymentsRegistry, SkillsRegistry,
};
use tempfile::TempDir;

fn test_paths(temp_dir: &TempDir) -> AppPaths {
    AppPaths::from_data_root(
        camino::Utf8PathBuf::from_path_buf(temp_dir.path().join(".skill-kits")).unwrap(),
    )
}

#[test]
fn missing_registry_initializes_empty() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);

    ensure_app_dirs(&paths).unwrap();

    assert_eq!(read_config(&paths).unwrap(), Config::default());
    assert_eq!(
        read_skills_registry(&paths).unwrap(),
        SkillsRegistry::default()
    );
    assert_eq!(
        read_deployments_registry(&paths).unwrap(),
        DeploymentsRegistry::default()
    );
    assert!(paths.config_file.exists());
    assert!(paths.skills_registry_file.exists());
    assert!(paths.deployments_registry_file.exists());
}

#[test]
fn invalid_toml_returns_error() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    std::fs::write(&paths.skills_registry_file, "version = [").unwrap();

    let err = read_skills_registry(&paths).unwrap_err();

    assert!(
        matches!(err, SkillKitsError::RegistryParse { path, .. } if path == paths.skills_registry_file)
    );
}

#[test]
fn state_lock_prevents_concurrent_writes() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    let _held = StateLock::acquire(&paths).unwrap();

    let err = write_config(&paths, &Config::default()).unwrap_err();

    assert!(matches!(err, SkillKitsError::RegistryBusy));
}

#[test]
fn update_skills_registry_reads_latest_state_under_lock() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();

    update_skills_registry(&paths, |registry| {
        registry.version = 7;
        Ok(())
    })
    .unwrap();

    assert_eq!(read_skills_registry(&paths).unwrap().version, 7);
}

#[test]
fn update_registry_files_reads_and_writes_skills_and_deployments_under_one_lock() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();

    let result = update_registry_files(&paths, |registries| {
        registries.skills.version = 7;
        registries.deployments.version = 9;
        registries.write_skills = true;
        registries.write_deployments = true;
        Ok("updated")
    })
    .unwrap();

    assert_eq!(result, "updated");
    assert_eq!(read_skills_registry(&paths).unwrap().version, 7);
    assert_eq!(read_deployments_registry(&paths).unwrap().version, 9);
}

#[test]
fn atomic_write_does_not_produce_half_written_target_file() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();

    let config = Config {
        theme: "dark".to_string(),
        ..Config::default()
    };
    write_config(&paths, &config).unwrap();

    let original = std::fs::read_to_string(&paths.config_file).unwrap();
    std::fs::write(paths.config_file.with_extension("toml.tmp"), "partial").unwrap();

    assert_eq!(
        std::fs::read_to_string(&paths.config_file).unwrap(),
        original
    );

    let replacement = Config {
        theme: "light".to_string(),
        ..Config::default()
    };
    write_config(&paths, &replacement).unwrap();

    assert_eq!(read_config(&paths).unwrap(), replacement);
    assert!(!std::fs::read_to_string(&paths.config_file)
        .unwrap()
        .contains("partial"));
}

#[test]
fn registry_read_write_round_trips() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();

    let skills = SkillsRegistry {
        version: 1,
        skills: Vec::new(),
    };
    let deployments = DeploymentsRegistry {
        version: 1,
        deployments: Vec::new(),
    };

    write_skills_registry(&paths, &skills).unwrap();
    write_deployments_registry(&paths, &deployments).unwrap();

    assert_eq!(read_skills_registry(&paths).unwrap(), skills);
    assert_eq!(read_deployments_registry(&paths).unwrap(), deployments);
}

#[test]
fn add_custom_agent_config_writes_enabled_custom_agent() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();

    let agent = add_custom_agent_config(
        &paths,
        AgentId::new("zed"),
        "Zed".to_string(),
        ".zed/skills".into(),
    )
    .unwrap();

    assert_eq!(agent.id, AgentId::new("zed"));
    assert_eq!(agent.label, "Zed");
    assert_eq!(agent.kind, AgentKind::Custom);
    assert_eq!(
        agent.project_skill_dirs,
        vec![camino::Utf8PathBuf::from(".zed/skills")]
    );
    assert!(agent.enabled);
    assert!(read_config(&paths)
        .unwrap()
        .agents
        .iter()
        .any(|configured| configured.id == AgentId::new("zed")));
}

#[test]
fn add_custom_agent_config_rejects_duplicate_agent_id_and_preserves_config() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    let config = Config::default();
    write_config(&paths, &config).unwrap();

    let err = add_custom_agent_config(
        &paths,
        AgentId::new("codex"),
        "Duplicate Codex".to_string(),
        ".duplicate/skills".into(),
    )
    .unwrap_err();

    assert!(matches!(
        err,
        SkillKitsError::AgentAlreadyConfigured { agent_id } if agent_id == AgentId::new("codex")
    ));
    assert_eq!(read_config(&paths).unwrap(), config);
}

#[test]
fn add_custom_agent_config_rejects_empty_id_and_label() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    let config = Config::default();
    write_config(&paths, &config).unwrap();

    let empty_id = add_custom_agent_config(
        &paths,
        AgentId::new(" "),
        "Zed".to_string(),
        ".zed/skills".into(),
    )
    .unwrap_err();
    let empty_label = add_custom_agent_config(
        &paths,
        AgentId::new("zed"),
        " ".to_string(),
        ".zed/skills".into(),
    )
    .unwrap_err();

    assert!(matches!(
        empty_id,
        SkillKitsError::InvalidAgentConfig { .. }
    ));
    assert!(matches!(
        empty_label,
        SkillKitsError::InvalidAgentConfig { .. }
    ));
    assert_eq!(read_config(&paths).unwrap(), config);
}

#[test]
fn add_custom_agent_config_rejects_absolute_project_dir() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    let config = Config::default();
    write_config(&paths, &config).unwrap();

    let err = add_custom_agent_config(
        &paths,
        AgentId::new("zed"),
        "Zed".to_string(),
        "/tmp/zed-skills".into(),
    )
    .unwrap_err();

    assert!(matches!(err, SkillKitsError::InvalidSkillDir { .. }));
    assert_eq!(read_config(&paths).unwrap(), config);
}

#[test]
fn add_custom_agent_config_rejects_parent_traversal_project_dir() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    let config = Config::default();
    write_config(&paths, &config).unwrap();

    let err = add_custom_agent_config(
        &paths,
        AgentId::new("zed"),
        "Zed".to_string(),
        "../outside".into(),
    )
    .unwrap_err();

    assert!(matches!(err, SkillKitsError::InvalidSkillDir { .. }));
    assert_eq!(read_config(&paths).unwrap(), config);
}

#[test]
fn update_agent_project_skill_dirs_updates_existing_agent_and_preserves_other_agents() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![
                AgentConfig {
                    id: AgentId::new("codex"),
                    label: "Codex".to_string(),
                    kind: AgentKind::BuiltIn,
                    global_skill_dirs: vec!["~/.codex/skills".into()],
                    project_skill_dirs: vec![".agents/skills".into()],
                    enabled: true,
                },
                AgentConfig {
                    id: AgentId::new("custom"),
                    label: "Custom".to_string(),
                    kind: AgentKind::Custom,
                    global_skill_dirs: vec!["~/custom/global".into()],
                    project_skill_dirs: vec![".custom/skills".into()],
                    enabled: true,
                },
            ],
            ..Config::default()
        },
    )
    .unwrap();

    let updated = update_agent_project_skill_dirs(
        &paths,
        &AgentId::new("custom"),
        vec![".custom/new-skills".into(), ".custom/shared".into()],
    )
    .unwrap();

    assert_eq!(
        updated.project_skill_dirs,
        vec![
            camino::Utf8PathBuf::from(".custom/new-skills"),
            camino::Utf8PathBuf::from(".custom/shared"),
        ]
    );
    assert_eq!(
        updated.global_skill_dirs,
        vec![camino::Utf8PathBuf::from("~/custom/global")]
    );
    let config = read_config(&paths).unwrap();
    assert_eq!(config.agents[0].id, AgentId::new("codex"));
    assert_eq!(
        config.agents[0].project_skill_dirs,
        vec![camino::Utf8PathBuf::from(".agents/skills")]
    );
    assert_eq!(config.agents[1], updated);
}

#[test]
fn configured_global_skill_dirs_uses_configured_custom_and_built_in_dirs() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![
                AgentConfig {
                    id: AgentId::new("codex"),
                    label: "Codex".to_string(),
                    kind: AgentKind::BuiltIn,
                    global_skill_dirs: vec!["~/my-codex/skills".into()],
                    project_skill_dirs: vec![".agents/skills".into()],
                    enabled: true,
                },
                AgentConfig {
                    id: AgentId::new("custom"),
                    label: "Custom".to_string(),
                    kind: AgentKind::Custom,
                    global_skill_dirs: vec!["/tmp/custom-skills".into()],
                    project_skill_dirs: vec![".custom/skills".into()],
                    enabled: true,
                },
            ],
            ..Config::default()
        },
    )
    .unwrap();

    let codex_dirs = configured_global_skill_dirs_for(&paths, &AgentId::new("codex")).unwrap();
    assert!(codex_dirs.contains(&camino::Utf8PathBuf::from("~/my-codex/skills")));
    assert!(codex_dirs.contains(&camino::Utf8PathBuf::from("~/.codex/skills")));
    assert!(codex_dirs.contains(&camino::Utf8PathBuf::from("~/.codex/plugins/cache")));
    assert!(codex_dirs.contains(&camino::Utf8PathBuf::from("~/.codex/vendor_imports")));
    assert!(!codex_dirs.contains(&camino::Utf8PathBuf::from("~/.skills-manager/skills")));
    assert_eq!(
        configured_global_skill_dirs_for(&paths, &AgentId::new("custom")).unwrap(),
        vec![camino::Utf8PathBuf::from("/tmp/custom-skills")]
    );
}

#[test]
fn update_agent_project_skill_dirs_returns_agent_not_found_for_missing_agent() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    let config = Config::default();
    write_config(&paths, &config).unwrap();

    let err = update_agent_project_skill_dirs(
        &paths,
        &AgentId::new("missing"),
        vec![".missing/skills".into()],
    )
    .unwrap_err();

    assert!(matches!(
        err,
        SkillKitsError::AgentNotFound { agent_id } if agent_id == AgentId::new("missing")
    ));
    assert_eq!(read_config(&paths).unwrap(), config);
}

#[test]
fn update_agent_project_skill_dirs_rejects_absolute_dirs() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![AgentConfig {
                id: AgentId::new("custom"),
                label: "Custom".to_string(),
                kind: AgentKind::Custom,
                global_skill_dirs: Vec::new(),
                project_skill_dirs: vec![".custom/skills".into()],
                enabled: true,
            }],
            ..Config::default()
        },
    )
    .unwrap();

    let err = update_agent_project_skill_dirs(
        &paths,
        &AgentId::new("custom"),
        vec!["/tmp/absolute".into()],
    )
    .unwrap_err();

    assert!(matches!(err, SkillKitsError::InvalidSkillDir { .. }));
    assert_eq!(
        read_config(&paths).unwrap().agents[0].project_skill_dirs,
        vec![camino::Utf8PathBuf::from(".custom/skills")]
    );
}

#[test]
fn reset_agent_project_skill_dirs_restores_built_in_default_and_preserves_custom_agents() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![
                AgentConfig {
                    id: AgentId::new("codex"),
                    label: "Codex".to_string(),
                    kind: AgentKind::BuiltIn,
                    global_skill_dirs: vec!["~/.codex/skills".into()],
                    project_skill_dirs: vec![".codex/custom".into()],
                    enabled: true,
                },
                AgentConfig {
                    id: AgentId::new("zed"),
                    label: "Zed".to_string(),
                    kind: AgentKind::Custom,
                    global_skill_dirs: Vec::new(),
                    project_skill_dirs: vec![".zed/skills".into()],
                    enabled: true,
                },
            ],
            ..Config::default()
        },
    )
    .unwrap();

    let reset = reset_agent_project_skill_dirs(&paths, &AgentId::new("codex")).unwrap();

    assert_eq!(reset.kind, AgentKind::BuiltIn);
    assert_eq!(
        reset.project_skill_dirs,
        vec![camino::Utf8PathBuf::from(".agents/skills")]
    );
    let config = read_config(&paths).unwrap();
    assert_eq!(config.agents[0], reset);
    assert_eq!(config.agents[1].id, AgentId::new("zed"));
}

#[test]
fn reset_agent_project_skill_dirs_rejects_custom_agent() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    let config = Config {
        agents: vec![AgentConfig {
            id: AgentId::new("zed"),
            label: "Zed".to_string(),
            kind: AgentKind::Custom,
            global_skill_dirs: Vec::new(),
            project_skill_dirs: vec![".zed/skills".into()],
            enabled: true,
        }],
        ..Config::default()
    };
    write_config(&paths, &config).unwrap();

    let err = reset_agent_project_skill_dirs(&paths, &AgentId::new("zed")).unwrap_err();

    assert!(matches!(err, SkillKitsError::InvalidAgentConfig { .. }));
    assert_eq!(read_config(&paths).unwrap(), config);
}

#[test]
fn remove_custom_agent_config_removes_only_custom_agents() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![
                AgentConfig {
                    id: AgentId::new("codex"),
                    label: "Codex".to_string(),
                    kind: AgentKind::BuiltIn,
                    global_skill_dirs: vec!["~/.codex/skills".into()],
                    project_skill_dirs: vec![".agents/skills".into()],
                    enabled: true,
                },
                AgentConfig {
                    id: AgentId::new("zed"),
                    label: "Zed".to_string(),
                    kind: AgentKind::Custom,
                    global_skill_dirs: Vec::new(),
                    project_skill_dirs: vec![".zed/skills".into()],
                    enabled: true,
                },
            ],
            ..Config::default()
        },
    )
    .unwrap();

    let removed = remove_custom_agent_config(&paths, &AgentId::new("zed")).unwrap();

    assert_eq!(removed.id, AgentId::new("zed"));
    let config = read_config(&paths).unwrap();
    assert_eq!(config.agents.len(), 1);
    assert_eq!(config.agents[0].id, AgentId::new("codex"));
}

#[test]
fn remove_custom_agent_config_rejects_built_in_agent() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    let config = Config::default();
    write_config(&paths, &config).unwrap();

    let err = remove_custom_agent_config(&paths, &AgentId::new("codex")).unwrap_err();

    assert!(matches!(err, SkillKitsError::InvalidAgentConfig { .. }));
    assert_eq!(read_config(&paths).unwrap(), config);
}
