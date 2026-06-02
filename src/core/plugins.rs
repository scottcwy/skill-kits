use crate::core::{
    error::{Result, SkillKitsError},
    fs::{atomic_write_string, safe_read_to_string},
    ids::AgentId,
    lock::StateLock,
    paths::AppPaths,
    registry::SkillMetadata,
    skills::load_skill_metadata_from_file,
};
use camino::{Utf8Path, Utf8PathBuf};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    fs,
};
use toml_edit::{value, DocumentMut, Item, Table};
use walkdir::WalkDir;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PluginPackage {
    pub id: String,
    pub agent_id: AgentId,
    pub plugin_key: String,
    pub name: String,
    pub display_name: String,
    pub provider: String,
    pub version: Option<String>,
    pub package_path: Utf8PathBuf,
    pub manifest_path: Option<Utf8PathBuf>,
    pub status: PluginStatus,
    pub can_toggle: bool,
    pub capabilities: Vec<PluginRuntimeCapability>,
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PluginStatus {
    Enabled,
    Disabled,
    Unknown,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PluginRuntimeCapability {
    pub id: String,
    pub parent_plugin_id: String,
    pub name: String,
    pub kind: RuntimeCapabilityKind,
    pub path: Utf8PathBuf,
    pub read_only: bool,
    pub metadata: Option<SkillMetadata>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RuntimeCapabilityKind {
    PluginProvidedSkill,
    Command,
    Agent,
    Asset,
    App,
    Unknown,
}

#[derive(Clone, Debug)]
struct PluginCandidate {
    provider: String,
    path_name: String,
    version: String,
    package_path: Utf8PathBuf,
}

#[derive(Clone, Debug, Default)]
struct ManifestMetadata {
    name: Option<String>,
    display_name: Option<String>,
    version: Option<String>,
}

#[derive(Clone, Debug)]
enum ConfigShape {
    Valid(HashMap<String, PluginStatus>),
    Invalid,
}

pub fn scan_plugin_packages(
    app_paths: &AppPaths,
    home_dir: &Utf8Path,
) -> Result<Vec<PluginPackage>> {
    let candidates = latest_candidates(codex_plugin_cache_root(home_dir))?;
    let config_path = codex_config_path(home_dir);
    let config_shape = read_plugin_config_shape(&config_path)?;
    let mut packages = Vec::new();

    for candidate in candidates {
        packages.push(build_plugin_package(
            app_paths,
            home_dir,
            candidate,
            &config_path,
            &config_shape,
        )?);
    }

    packages.sort_by(|left, right| {
        (
            left.provider.as_str(),
            left.display_name.as_str(),
            left.plugin_key.as_str(),
        )
            .cmp(&(
                right.provider.as_str(),
                right.display_name.as_str(),
                right.plugin_key.as_str(),
            ))
    });
    Ok(packages)
}

pub fn enable_plugin(
    app_paths: &AppPaths,
    home_dir: &Utf8Path,
    query: &str,
) -> Result<PluginPackage> {
    set_plugin_enabled(app_paths, home_dir, query, true)
}

pub fn disable_plugin(
    app_paths: &AppPaths,
    home_dir: &Utf8Path,
    query: &str,
) -> Result<PluginPackage> {
    set_plugin_enabled(app_paths, home_dir, query, false)
}

pub fn plugin_config_path(home_dir: &Utf8Path) -> Utf8PathBuf {
    codex_config_path(home_dir)
}

pub fn find_plugin_by_query(plugins: &[PluginPackage], query: &str) -> Result<PluginPackage> {
    let mut matches = plugins
        .iter()
        .filter(|plugin| plugin_matches_query(plugin, query))
        .cloned()
        .collect::<Vec<_>>();

    if matches.len() == 1 {
        return Ok(matches.remove(0));
    }
    if matches.is_empty() {
        return Err(SkillKitsError::PluginNotFound {
            query: query.to_string(),
        });
    }

    matches.sort_by(|left, right| {
        left.plugin_key
            .cmp(&right.plugin_key)
            .then(left.id.cmp(&right.id))
    });
    Err(SkillKitsError::AmbiguousPlugin {
        query: query.to_string(),
        matches: matches
            .into_iter()
            .map(|plugin| plugin.plugin_key)
            .collect(),
    })
}

pub fn query_matches_plugin_capability(plugins: &[PluginPackage], query: &str) -> bool {
    plugins.iter().any(|plugin| {
        plugin.capabilities.iter().any(|capability| {
            capability.id == query
                || capability.name == query
                || capability
                    .path
                    .file_stem()
                    .is_some_and(|name| name == query)
                || capability
                    .path
                    .parent()
                    .and_then(Utf8Path::file_name)
                    .is_some_and(|name| name == query)
        })
    })
}

pub fn flattened_capabilities(plugins: &[PluginPackage]) -> Vec<PluginRuntimeCapability> {
    plugins
        .iter()
        .flat_map(|plugin| plugin.capabilities.clone())
        .collect()
}

fn set_plugin_enabled(
    app_paths: &AppPaths,
    home_dir: &Utf8Path,
    query: &str,
    enabled: bool,
) -> Result<PluginPackage> {
    let plugin = find_plugin_by_query(&scan_plugin_packages(app_paths, home_dir)?, query)?;
    if plugin.status == PluginStatus::Invalid {
        write_plugin_enablement(app_paths, home_dir, &plugin.plugin_key, enabled)?;
    }
    if !plugin.can_toggle {
        return Err(SkillKitsError::PluginToggleBlocked {
            plugin_key: plugin.plugin_key,
            reason: "config shape is invalid".to_string(),
        });
    }
    write_plugin_enablement(app_paths, home_dir, &plugin.plugin_key, enabled)?;
    find_plugin_by_query(&scan_plugin_packages(app_paths, home_dir)?, &plugin.id)
}

fn write_plugin_enablement(
    app_paths: &AppPaths,
    home_dir: &Utf8Path,
    plugin_key: &str,
    enabled: bool,
) -> Result<()> {
    let _lock = StateLock::acquire(app_paths)?;
    let config_path = codex_config_path(home_dir);
    let contents = if config_path.exists() {
        safe_read_to_string(&config_path)?
    } else {
        String::new()
    };
    let mut document =
        contents
            .parse::<DocumentMut>()
            .map_err(|source| SkillKitsError::InvalidPluginConfig {
                path: config_path.clone(),
                reason: source.to_string(),
            })?;

    ensure_plugins_table(&config_path, &mut document)?;
    ensure_plugin_entry_table(&config_path, &mut document, plugin_key)?;
    document["plugins"][plugin_key]["enabled"] = value(enabled);
    atomic_write_string(&config_path, &document.to_string())
}

fn ensure_plugins_table(config_path: &Utf8Path, document: &mut DocumentMut) -> Result<()> {
    if document.get("plugins").is_none() || document["plugins"].is_none() {
        document["plugins"] = Item::Table(Table::new());
        return Ok(());
    }

    if document["plugins"].is_table() {
        Ok(())
    } else {
        Err(SkillKitsError::InvalidPluginConfig {
            path: config_path.to_path_buf(),
            reason: "plugins is not a table".to_string(),
        })
    }
}

fn ensure_plugin_entry_table(
    config_path: &Utf8Path,
    document: &mut DocumentMut,
    plugin_key: &str,
) -> Result<()> {
    if document["plugins"].get(plugin_key).is_none() {
        document["plugins"][plugin_key] = Item::Table(Table::new());
        return Ok(());
    }

    if document["plugins"][plugin_key].is_table() {
        Ok(())
    } else {
        Err(SkillKitsError::InvalidPluginConfig {
            path: config_path.to_path_buf(),
            reason: format!("plugin entry {plugin_key} is not a table"),
        })
    }
}

fn build_plugin_package(
    _app_paths: &AppPaths,
    _home_dir: &Utf8Path,
    candidate: PluginCandidate,
    _config_path: &Utf8Path,
    config_shape: &ConfigShape,
) -> Result<PluginPackage> {
    let manifest_path = candidate.package_path.join(".codex-plugin/plugin.json");
    let manifest = read_manifest_metadata(&manifest_path)?;
    let name = manifest
        .name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| candidate.path_name.clone());
    let display_name = manifest
        .display_name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| name.clone());
    let version = manifest.version.or(Some(candidate.version));
    let plugin_key = format!("{name}@{}", candidate.provider);
    let status = plugin_status(config_shape, &plugin_key);
    let can_toggle = matches!(status, PluginStatus::Enabled | PluginStatus::Disabled);
    let id = plugin_id(&AgentId::new("codex"), &plugin_key, &candidate.package_path);
    let capabilities = discover_runtime_capabilities(&candidate.package_path, &id)?;
    let updated_at = package_updated_at(&candidate.package_path);

    Ok(PluginPackage {
        id,
        agent_id: AgentId::new("codex"),
        plugin_key,
        name,
        display_name,
        provider: candidate.provider,
        version,
        package_path: candidate.package_path,
        manifest_path: manifest_path.exists().then_some(manifest_path),
        status,
        can_toggle,
        capabilities,
        updated_at,
    })
}

fn latest_candidates(cache_root: Utf8PathBuf) -> Result<Vec<PluginCandidate>> {
    if !cache_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut by_plugin: BTreeMap<(String, String), Vec<PluginCandidate>> = BTreeMap::new();
    for provider_entry in fs::read_dir(cache_root.as_std_path())? {
        let provider_entry = provider_entry?;
        if !provider_entry.file_type()?.is_dir() {
            continue;
        }
        let provider_path = utf8_path(provider_entry.path())?;
        let Some(provider) = provider_path.file_name().map(ToOwned::to_owned) else {
            continue;
        };
        for plugin_entry in fs::read_dir(provider_path.as_std_path())? {
            let plugin_entry = plugin_entry?;
            if !plugin_entry.file_type()?.is_dir() {
                continue;
            }
            let plugin_path = utf8_path(plugin_entry.path())?;
            let Some(path_name) = plugin_path.file_name().map(ToOwned::to_owned) else {
                continue;
            };
            for version_entry in fs::read_dir(plugin_path.as_std_path())? {
                let version_entry = version_entry?;
                if !version_entry.file_type()?.is_dir() {
                    continue;
                }
                let package_path = utf8_path(version_entry.path())?;
                let Some(version) = package_path.file_name().map(ToOwned::to_owned) else {
                    continue;
                };
                by_plugin
                    .entry((provider.clone(), path_name.clone()))
                    .or_default()
                    .push(PluginCandidate {
                        provider: provider.clone(),
                        path_name: path_name.clone(),
                        version,
                        package_path,
                    });
            }
        }
    }

    let mut candidates = Vec::new();
    for (_, mut versions) in by_plugin {
        versions.sort_by(|left, right| compare_versions(&right.version, &left.version));
        if let Some(candidate) = versions.into_iter().next() {
            candidates.push(candidate);
        }
    }
    Ok(candidates)
}

fn compare_versions(left: &str, right: &str) -> Ordering {
    match (parse_version(left), parse_version(right)) {
        (Some(left), Some(right)) => left.cmp(&right),
        _ => left.cmp(right),
    }
}

fn parse_version(version: &str) -> Option<Version> {
    Version::parse(version.trim_start_matches('v')).ok()
}

fn read_manifest_metadata(path: &Utf8Path) -> Result<ManifestMetadata> {
    if !path.exists() {
        return Ok(ManifestMetadata::default());
    }

    let contents = match safe_read_to_string(path) {
        Ok(contents) => contents,
        Err(_) => return Ok(ManifestMetadata::default()),
    };
    let Ok(json) = serde_json::from_str::<JsonValue>(&contents) else {
        return Ok(ManifestMetadata::default());
    };

    Ok(ManifestMetadata {
        name: json_string(&json, "name"),
        display_name: json_string(&json, "display_name")
            .or_else(|| json_string(&json, "displayName"))
            .or_else(|| json_string(&json, "title")),
        version: json_string(&json, "version"),
    })
}

fn json_string(json: &JsonValue, key: &str) -> Option<String> {
    json.get(key)
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn discover_runtime_capabilities(
    package_path: &Utf8Path,
    parent_plugin_id: &str,
) -> Result<Vec<PluginRuntimeCapability>> {
    let mut capabilities = Vec::new();
    discover_skill_capabilities(package_path, parent_plugin_id, &mut capabilities)?;
    discover_file_capabilities(
        package_path,
        parent_plugin_id,
        "commands",
        RuntimeCapabilityKind::Command,
        &["md"],
        &mut capabilities,
    )?;
    discover_file_capabilities(
        package_path,
        parent_plugin_id,
        "agents",
        RuntimeCapabilityKind::Agent,
        &["md", "yaml", "yml"],
        &mut capabilities,
    )?;
    discover_tree_capabilities(
        package_path,
        parent_plugin_id,
        "assets",
        RuntimeCapabilityKind::Asset,
        &mut capabilities,
    )?;
    discover_tree_capabilities(
        package_path,
        parent_plugin_id,
        "website",
        RuntimeCapabilityKind::App,
        &mut capabilities,
    )?;
    discover_tree_capabilities(
        package_path,
        parent_plugin_id,
        "apps",
        RuntimeCapabilityKind::App,
        &mut capabilities,
    )?;
    capabilities.sort_by(|left, right| {
        capability_sort_rank(&left.kind)
            .cmp(&capability_sort_rank(&right.kind))
            .then(left.name.cmp(&right.name))
            .then(left.path.cmp(&right.path))
    });
    Ok(capabilities)
}

fn discover_skill_capabilities(
    package_path: &Utf8Path,
    parent_plugin_id: &str,
    capabilities: &mut Vec<PluginRuntimeCapability>,
) -> Result<()> {
    let skills_dir = package_path.join("skills");
    if !skills_dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(skills_dir.as_std_path())? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let skill_dir = utf8_path(entry.path())?;
        let skill_file = skill_dir.join("SKILL.md");
        if !skill_file.is_file() {
            continue;
        }
        let metadata = load_skill_metadata_from_file(&skill_file).ok().flatten();
        let name = metadata
            .as_ref()
            .and_then(|metadata| metadata.title.clone())
            .unwrap_or_else(|| dir_or_file_name(&skill_dir));
        capabilities.push(build_capability(
            parent_plugin_id,
            name,
            RuntimeCapabilityKind::PluginProvidedSkill,
            skill_file,
            metadata,
        ));
    }
    Ok(())
}

fn discover_file_capabilities(
    package_path: &Utf8Path,
    parent_plugin_id: &str,
    dir_name: &str,
    kind: RuntimeCapabilityKind,
    extensions: &[&str],
    capabilities: &mut Vec<PluginRuntimeCapability>,
) -> Result<()> {
    let root = package_path.join(dir_name);
    if !root.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(root.as_std_path())? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = utf8_path(entry.path())?;
        let extension = path.extension().unwrap_or_default();
        if !extensions.iter().any(|allowed| *allowed == extension) {
            continue;
        }
        capabilities.push(build_capability(
            parent_plugin_id,
            path.file_stem()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| dir_or_file_name(&path)),
            kind.clone(),
            path,
            None,
        ));
    }
    Ok(())
}

fn discover_tree_capabilities(
    package_path: &Utf8Path,
    parent_plugin_id: &str,
    dir_name: &str,
    kind: RuntimeCapabilityKind,
    capabilities: &mut Vec<PluginRuntimeCapability>,
) -> Result<()> {
    let root = package_path.join(dir_name);
    if !root.is_dir() {
        return Ok(());
    }

    let mut entries = Vec::new();
    for entry in WalkDir::new(&root).min_depth(1).max_depth(1) {
        let entry = entry.map_err(|source| std::io::Error::other(source.to_string()))?;
        entries.push(utf8_path(entry.path().to_path_buf())?);
    }
    entries.sort();
    for path in entries {
        capabilities.push(build_capability(
            parent_plugin_id,
            dir_or_file_name(&path),
            kind.clone(),
            path,
            None,
        ));
    }
    Ok(())
}

fn build_capability(
    parent_plugin_id: &str,
    name: String,
    kind: RuntimeCapabilityKind,
    path: Utf8PathBuf,
    metadata: Option<SkillMetadata>,
) -> PluginRuntimeCapability {
    PluginRuntimeCapability {
        id: capability_id(parent_plugin_id, &kind, &path),
        parent_plugin_id: parent_plugin_id.to_string(),
        name,
        kind,
        path,
        read_only: true,
        metadata,
    }
}

fn capability_sort_rank(kind: &RuntimeCapabilityKind) -> u8 {
    match kind {
        RuntimeCapabilityKind::PluginProvidedSkill => 0,
        RuntimeCapabilityKind::Command => 1,
        RuntimeCapabilityKind::Agent => 2,
        RuntimeCapabilityKind::Asset => 3,
        RuntimeCapabilityKind::App => 4,
        RuntimeCapabilityKind::Unknown => 5,
    }
}

fn read_plugin_config_shape(config_path: &Utf8Path) -> Result<ConfigShape> {
    if !config_path.exists() {
        return Ok(ConfigShape::Valid(HashMap::new()));
    }
    let contents = safe_read_to_string(config_path)?;
    let document =
        contents
            .parse::<DocumentMut>()
            .map_err(|source| SkillKitsError::InvalidPluginConfig {
                path: config_path.to_path_buf(),
                reason: source.to_string(),
            })?;
    let Some(plugins) = document.get("plugins") else {
        return Ok(ConfigShape::Valid(HashMap::new()));
    };
    let Some(plugins_table) = plugins.as_table() else {
        return Ok(ConfigShape::Invalid);
    };

    let mut states = HashMap::new();
    for (plugin_key, item) in plugins_table.iter() {
        let Some(table) = item.as_table() else {
            return Ok(ConfigShape::Invalid);
        };
        if let Some(enabled) = table.get("enabled").and_then(Item::as_bool) {
            states.insert(
                plugin_key.to_string(),
                if enabled {
                    PluginStatus::Enabled
                } else {
                    PluginStatus::Disabled
                },
            );
        }
    }
    Ok(ConfigShape::Valid(states))
}

fn plugin_status(config_shape: &ConfigShape, plugin_key: &str) -> PluginStatus {
    match config_shape {
        ConfigShape::Valid(states) => states
            .get(plugin_key)
            .cloned()
            .unwrap_or(PluginStatus::Enabled),
        ConfigShape::Invalid => PluginStatus::Invalid,
    }
}

fn plugin_matches_query(plugin: &PluginPackage, query: &str) -> bool {
    plugin.id == query
        || plugin.plugin_key == query
        || plugin.name == query
        || plugin.display_name == query
}

fn plugin_id(agent_id: &AgentId, plugin_key: &str, package_path: &Utf8Path) -> String {
    let canonical = canonical_or_original(package_path);
    let mut hasher = Sha256::new();
    hasher.update(agent_id.as_str().as_bytes());
    hasher.update([0]);
    hasher.update(plugin_key.as_bytes());
    hasher.update([0]);
    hasher.update(canonical.as_str().as_bytes());
    hex::encode(hasher.finalize())
}

fn capability_id(parent_plugin_id: &str, kind: &RuntimeCapabilityKind, path: &Utf8Path) -> String {
    let canonical = canonical_or_original(path);
    let mut hasher = Sha256::new();
    hasher.update(parent_plugin_id.as_bytes());
    hasher.update([0]);
    hasher.update(format!("{kind:?}").as_bytes());
    hasher.update([0]);
    hasher.update(canonical.as_str().as_bytes());
    hex::encode(hasher.finalize())
}

fn canonical_or_original(path: &Utf8Path) -> Utf8PathBuf {
    fs::canonicalize(path.as_std_path())
        .ok()
        .and_then(|path| Utf8PathBuf::from_path_buf(path).ok())
        .unwrap_or_else(|| path.to_path_buf())
}

fn package_updated_at(path: &Utf8Path) -> Option<String> {
    let modified = fs::metadata(path.as_std_path()).ok()?.modified().ok()?;
    let secs = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(secs.to_string())
}

fn dir_or_file_name(path: &Utf8Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.to_string())
}

fn codex_plugin_cache_root(home_dir: &Utf8Path) -> Utf8PathBuf {
    let plugins_root = home_dir.join(".codex/plugins");
    if plugins_root.file_name() == Some("cache") {
        plugins_root
    } else {
        plugins_root.join("cache")
    }
}

fn codex_config_path(home_dir: &Utf8Path) -> Utf8PathBuf {
    home_dir.join(".codex/config.toml")
}

fn utf8_path(path: std::path::PathBuf) -> Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path).map_err(|path| SkillKitsError::InvalidSkillDir {
        path: Utf8PathBuf::from(path.to_string_lossy().to_string()),
        reason: "path is not UTF-8".to_string(),
    })
}
