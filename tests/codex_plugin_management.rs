use camino::{Utf8Path, Utf8PathBuf};
use skill_kits::core::{
    agent_space::scan_agent_spaces,
    config::{write_config, Config},
    paths::{ensure_app_dirs, AppPaths},
    plugins::{
        disable_plugin, enable_plugin, scan_plugin_packages, PluginStatus, RuntimeCapabilityKind,
    },
};
use tempfile::TempDir;

fn test_paths(temp_dir: &TempDir) -> AppPaths {
    AppPaths::from_data_root(Utf8PathBuf::from_path_buf(temp_dir.path().join("data")).unwrap())
}

fn home_path(temp_dir: &TempDir) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(temp_dir.path().join("home")).unwrap()
}

fn write_file(path: &Utf8Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn plugin_package(home: &Utf8Path, provider: &str, name: &str, version: &str) -> Utf8PathBuf {
    home.join(format!(".codex/plugins/cache/{provider}/{name}/{version}"))
}

fn write_manifest(package: &Utf8Path, body: &str) {
    write_file(&package.join(".codex-plugin/plugin.json"), body);
}

#[test]
fn discovers_manifest_plugin_and_bundled_capabilities() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    let package = plugin_package(&home, "openai-curated", "github", "1.2.3");
    write_manifest(
        &package,
        r#"{"name":"github","display_name":"GitHub Tools","version":"1.2.3"}"#,
    );
    write_file(
        &package.join("skills/github/SKILL.md"),
        "+++\ntitle = \"GitHub Skill\"\ndescription = \"Works with GitHub.\"\n+++\n",
    );
    write_file(&package.join("commands/review.md"), "# Review\n");
    write_file(&package.join("agents/reviewer.yaml"), "name: reviewer\n");
    write_file(&package.join("assets/logo.txt"), "asset\n");
    write_file(&package.join("apps/panel/index.html"), "<main></main>\n");

    let plugins = scan_plugin_packages(&paths, &home).unwrap();

    assert_eq!(plugins.len(), 1);
    let plugin = &plugins[0];
    assert_eq!(plugin.plugin_key, "github@openai-curated");
    assert_eq!(plugin.display_name, "GitHub Tools");
    assert_eq!(plugin.status, PluginStatus::Enabled);
    assert_eq!(
        plugin.manifest_path,
        Some(package.join(".codex-plugin/plugin.json"))
    );
    assert!(plugin.can_toggle);
    assert_eq!(
        plugin
            .capabilities
            .iter()
            .map(|capability| &capability.kind)
            .collect::<Vec<_>>(),
        vec![
            &RuntimeCapabilityKind::PluginProvidedSkill,
            &RuntimeCapabilityKind::Command,
            &RuntimeCapabilityKind::Agent,
            &RuntimeCapabilityKind::Asset,
            &RuntimeCapabilityKind::App,
        ]
    );
    let skill = plugin
        .capabilities
        .iter()
        .find(|capability| capability.kind == RuntimeCapabilityKind::PluginProvidedSkill)
        .unwrap();
    assert_eq!(skill.name, "GitHub Skill");
    assert!(skill.read_only);
    assert_eq!(
        skill.metadata.as_ref().unwrap().description.as_deref(),
        Some("Works with GitHub.")
    );
}

#[test]
fn discovers_latest_version_with_semver_and_lexicographic_fallback() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_manifest(
        &plugin_package(&home, "openai-curated", "github", "v1.9.0"),
        r#"{"name":"github"}"#,
    );
    write_manifest(
        &plugin_package(&home, "openai-curated", "github", "2.0.0"),
        r#"{"name":"github"}"#,
    );
    write_manifest(
        &plugin_package(&home, "test-marketplace", "local", "beta"),
        r#"{"name":"local"}"#,
    );
    write_manifest(
        &plugin_package(&home, "test-marketplace", "local", "alpha"),
        r#"{"name":"local"}"#,
    );

    let plugins = scan_plugin_packages(&paths, &home).unwrap();

    let github = plugins
        .iter()
        .find(|plugin| plugin.plugin_key == "github@openai-curated")
        .unwrap();
    assert_eq!(github.version.as_deref(), Some("2.0.0"));
    let local = plugins
        .iter()
        .find(|plugin| plugin.plugin_key == "local@test-marketplace")
        .unwrap();
    assert_eq!(local.version.as_deref(), Some("beta"));
}

#[test]
fn falls_back_to_cache_layout_when_manifest_is_missing() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    std::fs::create_dir_all(plugin_package(
        &home,
        "test-marketplace",
        "my-plugin",
        "abcd1234",
    ))
    .unwrap();

    let plugins = scan_plugin_packages(&paths, &home).unwrap();

    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].plugin_key, "my-plugin@test-marketplace");
    assert_eq!(plugins[0].name, "my-plugin");
    assert_eq!(plugins[0].version.as_deref(), Some("abcd1234"));
    assert_eq!(plugins[0].manifest_path, None);
}

#[test]
fn reads_and_writes_plugin_enablement_without_touching_package_files() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    let package = plugin_package(&home, "openai-bundled", "browser", "26.0.0");
    write_manifest(&package, r#"{"name":"browser"}"#);
    let skill_file = package.join("skills/browser/SKILL.md");
    write_file(&skill_file, "# Browser Skill\n");
    let codex_config = home.join(".codex/config.toml");
    write_file(
        &codex_config,
        "model = \"gpt-5\"\n\n[plugins.\"browser@openai-bundled\"]\nenabled = false\n",
    );

    let disabled = scan_plugin_packages(&paths, &home).unwrap();
    assert_eq!(disabled[0].status, PluginStatus::Disabled);

    enable_plugin(&paths, &home, "browser@openai-bundled").unwrap();
    let config_after_enable = std::fs::read_to_string(&codex_config).unwrap();
    assert!(config_after_enable.contains("model = \"gpt-5\""));
    assert!(config_after_enable.contains("enabled = true"));
    assert_eq!(
        std::fs::read_to_string(&skill_file).unwrap(),
        "# Browser Skill\n"
    );

    disable_plugin(&paths, &home, "browser@openai-bundled").unwrap();
    let config_after_disable = std::fs::read_to_string(&codex_config).unwrap();
    assert!(config_after_disable.contains("enabled = false"));
    assert!(!package.join("skills/browser/SKILL.md.disabled").exists());
}

#[test]
fn rejects_toggle_when_codex_plugin_config_shape_is_invalid() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    let package = plugin_package(&home, "openai-bundled", "browser", "26.0.0");
    write_manifest(&package, r#"{"name":"browser"}"#);
    let codex_config = home.join(".codex/config.toml");
    write_file(&codex_config, "plugins = \"invalid\"\n");

    let plugins = scan_plugin_packages(&paths, &home).unwrap();
    assert_eq!(plugins[0].status, PluginStatus::Invalid);
    assert!(!plugins[0].can_toggle);
    let error = disable_plugin(&paths, &home, "browser@openai-bundled")
        .expect_err("invalid plugins table must reject writes")
        .to_string();
    assert!(error.contains("invalid Codex plugin config"));

    write_file(
        &codex_config,
        "[plugins]\n\"browser@openai-bundled\" = \"invalid\"\n",
    );
    let error = enable_plugin(&paths, &home, "browser@openai-bundled")
        .expect_err("invalid plugin entry must reject writes")
        .to_string();
    assert!(error.contains("invalid Codex plugin config"));
}

#[test]
fn routine_native_scan_does_not_create_plugin_skill_instances() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = home_path(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_file(
        &home.join(".codex/plugins/cache/openai-curated/github/1.0.0/skills/github/SKILL.md"),
        "# Plugin Skill\n",
    );
    write_file(&home.join(".codex/skills/native/SKILL.md"), "# Native\n");

    let instances = scan_agent_spaces(&paths, &home).unwrap();

    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].skill_dir, home.join(".codex/skills/native"));
}
