use skill_kits::core::config::{read_config, write_config, Config};
use skill_kits::core::error::SkillKitsError;
use skill_kits::core::lock::StateLock;
use skill_kits::core::paths::{ensure_app_dirs, AppPaths};
use skill_kits::core::registry::{
    read_deployments_registry, read_skills_registry, update_skills_registry,
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
