use crate::core::{
    config::read_config,
    error::{Result, SkillKitsError},
    ids::{AgentId, SkillId},
    paths::AppPaths,
    registry::{
        read_deployments_registry, read_skills_registry, DeploymentsRegistry, ManagedSkill,
        SkillMetadata, SkillsRegistry, ToggleState,
    },
    skills::{disabled_skill_markdown_path, load_skill_metadata_from_file, skill_markdown_path},
};
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
};
use walkdir::WalkDir;

pub const AGENT_SPACE_PLUGIN_DEPTH: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SkillInstanceScope {
    Global,
    Project { name: String, path: Utf8PathBuf },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SkillInstanceSourceKind {
    AgentSpace,
    ProjectDeployment,
    PluginCache,
    Vendor,
    ManagedInventory,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillInstance {
    pub id: String,
    pub stable_id: Option<SkillId>,
    pub name: String,
    pub agent_id: AgentId,
    pub scope: SkillInstanceScope,
    pub skill_dir: Utf8PathBuf,
    pub enabled_path: Utf8PathBuf,
    pub disabled_path: Utf8PathBuf,
    pub toggle_state: ToggleState,
    pub source_kind: SkillInstanceSourceKind,
    pub managed: bool,
    pub writable: bool,
    pub metadata: Option<SkillMetadata>,
    pub content_hash: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SkillInstanceRequest<'a> {
    pub app_paths: &'a AppPaths,
    pub home_dir: &'a Utf8Path,
    pub instance_id: &'a str,
}

pub fn scan_agent_spaces(app_paths: &AppPaths, home_dir: &Utf8Path) -> Result<Vec<SkillInstance>> {
    let config = read_config(app_paths)?;
    let skills = read_skills_registry_if_present(app_paths)?.skills;
    let deployments = read_deployments_registry_if_present(app_paths)?.deployments;
    let managed_by_path = managed_by_path(&skills);
    let managed_by_name = managed_by_name(&skills);

    let mut instances = Vec::new();
    let mut seen = HashSet::new();

    for agent in config.agents.iter().filter(|agent| agent.enabled) {
        for root in &agent.global_skill_dirs {
            let root = expand_home(root, home_dir);
            let source_kind = source_kind_for_root(&root);
            let root_instances = if is_recursive_source(&source_kind) {
                scan_recursive_root(
                    &root,
                    agent.id.clone(),
                    SkillInstanceScope::Global,
                    source_kind,
                    &managed_by_path,
                    &managed_by_name,
                )?
            } else {
                scan_immediate_root(
                    &root,
                    agent.id.clone(),
                    SkillInstanceScope::Global,
                    source_kind,
                    &managed_by_path,
                    &managed_by_name,
                )?
            };
            push_unique(&mut instances, &mut seen, root_instances);
        }

        for recent in &config.recent_projects {
            for project_dir in &agent.project_skill_dirs {
                let root = recent.path.join(project_dir);
                let root_instances = scan_immediate_root(
                    &root,
                    agent.id.clone(),
                    SkillInstanceScope::Project {
                        name: recent.name.clone(),
                        path: recent.path.clone(),
                    },
                    SkillInstanceSourceKind::ProjectDeployment,
                    &managed_by_path,
                    &managed_by_name,
                )?;
                push_unique(&mut instances, &mut seen, root_instances);
            }
        }
    }

    for deployment in deployments {
        if toggle_state(&deployment.deployment_path) != ToggleState::InvalidBothMissing {
            continue;
        }
        let metadata = None;
        let content_hash = None;
        let scope = SkillInstanceScope::Project {
            name: deployment.project_name.clone(),
            path: deployment.project_path.clone(),
        };
        let instance = SkillInstance {
            id: instance_id(&deployment.agent_id, &scope, &deployment.deployment_path),
            stable_id: Some(deployment.skill_id.clone()),
            name: deployment.skill_name.clone(),
            agent_id: deployment.agent_id,
            enabled_path: skill_markdown_path(&deployment.deployment_path),
            disabled_path: disabled_skill_markdown_path(&deployment.deployment_path),
            skill_dir: deployment.deployment_path,
            toggle_state: ToggleState::InvalidBothMissing,
            source_kind: SkillInstanceSourceKind::ProjectDeployment,
            managed: managed_by_name.contains_key(&deployment.skill_name),
            writable: false,
            metadata,
            content_hash,
            updated_at: Some(deployment.updated_at),
            scope,
        };
        if seen.insert(instance.id.clone()) {
            instances.push(instance);
        }
    }

    instances.sort_by(|left, right| {
        (
            left.agent_id.as_str(),
            scope_sort_key(&left.scope),
            left.skill_dir.as_str(),
        )
            .cmp(&(
                right.agent_id.as_str(),
                scope_sort_key(&right.scope),
                right.skill_dir.as_str(),
            ))
    });
    Ok(instances)
}

pub fn enable_skill_instance(request: SkillInstanceRequest<'_>) -> Result<SkillInstance> {
    let instance = find_skill_instance(&request)?;
    ensure_instance_can_toggle(&instance)?;
    if instance.disabled_path.exists() {
        fs::rename(&instance.disabled_path, &instance.enabled_path)?;
    }
    find_skill_instance(&request)
}

pub fn disable_skill_instance(request: SkillInstanceRequest<'_>) -> Result<SkillInstance> {
    let instance = find_skill_instance(&request)?;
    ensure_instance_can_toggle(&instance)?;
    if instance.enabled_path.exists() {
        fs::rename(&instance.enabled_path, &instance.disabled_path)?;
    }
    find_skill_instance(&request)
}

fn find_skill_instance(request: &SkillInstanceRequest<'_>) -> Result<SkillInstance> {
    scan_agent_spaces(request.app_paths, request.home_dir)?
        .into_iter()
        .find(|instance| instance.id == request.instance_id)
        .ok_or_else(|| SkillKitsError::SkillNotFound {
            query: request.instance_id.to_string(),
        })
}

fn ensure_instance_can_toggle(instance: &SkillInstance) -> Result<()> {
    let source_allows_toggle = matches!(
        instance.source_kind,
        SkillInstanceSourceKind::AgentSpace | SkillInstanceSourceKind::ProjectDeployment
    );
    let valid_toggle = matches!(
        instance.toggle_state,
        ToggleState::Enabled | ToggleState::Disabled
    );
    if instance.writable && source_allows_toggle && valid_toggle {
        return Ok(());
    }
    Err(SkillKitsError::InvalidToggleState {
        path: instance.skill_dir.clone(),
    })
}

fn scan_immediate_root(
    root: &Utf8Path,
    agent_id: AgentId,
    scope: SkillInstanceScope,
    source_kind: SkillInstanceSourceKind,
    managed_by_path: &HashMap<Utf8PathBuf, SkillId>,
    managed_by_name: &HashMap<String, SkillId>,
) -> Result<Vec<SkillInstance>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }

    let mut instances = Vec::new();
    for entry in fs::read_dir(root.as_std_path())? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_dir() {
            continue;
        }
        let skill_dir = utf8_path(entry.path())?;
        if has_toggle_file(&skill_dir) {
            instances.push(build_instance(
                skill_dir,
                agent_id.clone(),
                scope.clone(),
                source_kind.clone(),
                managed_by_path,
                managed_by_name,
            )?);
        }
    }
    Ok(instances)
}

fn scan_recursive_root(
    root: &Utf8Path,
    agent_id: AgentId,
    scope: SkillInstanceScope,
    source_kind: SkillInstanceSourceKind,
    managed_by_path: &HashMap<Utf8PathBuf, SkillId>,
    managed_by_name: &HashMap<String, SkillId>,
) -> Result<Vec<SkillInstance>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }

    let mut instances = Vec::new();
    let mut walker = WalkDir::new(root)
        .follow_links(false)
        .max_depth(AGENT_SPACE_PLUGIN_DEPTH)
        .into_iter()
        .filter_entry(|entry| {
            if entry.depth() == 0 {
                return true;
            }
            let name = entry.file_name().to_string_lossy();
            !is_noisy_dir(&name)
        });

    while let Some(entry) = walker.next() {
        let entry = entry.map_err(|source| std::io::Error::other(source.to_string()))?;
        if !entry.file_type().is_dir() || entry.depth() == 0 {
            continue;
        }
        let skill_dir = utf8_path(entry.path().to_path_buf())?;
        if has_toggle_file(&skill_dir) {
            walker.skip_current_dir();
            instances.push(build_instance(
                skill_dir,
                agent_id.clone(),
                scope.clone(),
                source_kind.clone(),
                managed_by_path,
                managed_by_name,
            )?);
        }
    }
    Ok(instances)
}

fn read_skills_registry_if_present(app_paths: &AppPaths) -> Result<SkillsRegistry> {
    if app_paths.skills_registry_file.exists() {
        read_skills_registry(app_paths)
    } else {
        Ok(SkillsRegistry::default())
    }
}

fn read_deployments_registry_if_present(app_paths: &AppPaths) -> Result<DeploymentsRegistry> {
    if app_paths.deployments_registry_file.exists() {
        read_deployments_registry(app_paths)
    } else {
        Ok(DeploymentsRegistry::default())
    }
}

fn build_instance(
    skill_dir: Utf8PathBuf,
    agent_id: AgentId,
    scope: SkillInstanceScope,
    source_kind: SkillInstanceSourceKind,
    managed_by_path: &HashMap<Utf8PathBuf, SkillId>,
    managed_by_name: &HashMap<String, SkillId>,
) -> Result<SkillInstance> {
    let enabled_path = skill_markdown_path(&skill_dir);
    let disabled_path = disabled_skill_markdown_path(&skill_dir);
    let toggle_state = toggle_state(&skill_dir);
    let toggle_file = match toggle_state {
        ToggleState::Enabled => Some(enabled_path.as_path()),
        ToggleState::Disabled => Some(disabled_path.as_path()),
        ToggleState::InvalidBothPresent | ToggleState::InvalidBothMissing => None,
    };
    let metadata = toggle_file
        .map(load_skill_metadata_from_file)
        .transpose()?
        .flatten();
    let name = if matches!(toggle_state, ToggleState::InvalidBothPresent) {
        dir_name(&skill_dir)
    } else {
        metadata
            .as_ref()
            .and_then(|metadata| metadata.title.clone())
            .unwrap_or_else(|| dir_name(&skill_dir))
    };
    let stable_id = managed_by_path
        .get(&skill_dir)
        .cloned()
        .or_else(|| managed_by_name.get(&name).cloned())
        .or_else(|| managed_by_name.get(&dir_name(&skill_dir)).cloned());
    let managed = stable_id.is_some();
    let content_hash = if matches!(toggle_state, ToggleState::Enabled | ToggleState::Disabled) {
        Some(hash_agent_skill_dir(&skill_dir)?)
    } else {
        None
    };
    let updated_at = toggle_file.and_then(file_updated_at);
    let read_only_source = matches!(
        source_kind,
        SkillInstanceSourceKind::PluginCache
            | SkillInstanceSourceKind::Vendor
            | SkillInstanceSourceKind::ManagedInventory
    );
    let valid_toggle = matches!(toggle_state, ToggleState::Enabled | ToggleState::Disabled);
    let writable = valid_toggle && !read_only_source && is_writable_dir(&skill_dir);

    Ok(SkillInstance {
        id: instance_id(&agent_id, &scope, &skill_dir),
        stable_id,
        name,
        agent_id,
        scope,
        skill_dir: skill_dir.clone(),
        enabled_path,
        disabled_path,
        toggle_state,
        source_kind,
        managed,
        writable,
        metadata,
        content_hash,
        updated_at,
    })
}

fn hash_agent_skill_dir(skill_dir: &Utf8Path) -> Result<String> {
    let mut files = Vec::new();
    for entry in WalkDir::new(skill_dir).follow_links(false) {
        let entry = entry.map_err(|source| std::io::Error::other(source.to_string()))?;
        if entry.file_type().is_dir() {
            continue;
        }
        let path = utf8_path(entry.path().to_path_buf())?;
        let rel = path.strip_prefix(skill_dir).unwrap();
        if should_ignore_hash_path(rel) {
            continue;
        }
        files.push(path);
    }

    files.sort();
    let mut hasher = Sha256::new();
    for file in files {
        let rel = file.strip_prefix(skill_dir).unwrap();
        let normalized = normalize_hash_path(rel);
        hasher.update(normalized.as_bytes());
        hasher.update([0]);
        let bytes = fs::read(file.as_std_path())?;
        hasher.update(bytes.len().to_le_bytes());
        hasher.update([0]);
        hasher.update(bytes);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn instance_id(agent_id: &AgentId, scope: &SkillInstanceScope, skill_dir: &Utf8Path) -> String {
    let canonical = fs::canonicalize(skill_dir.as_std_path())
        .ok()
        .and_then(|path| Utf8PathBuf::from_path_buf(path).ok())
        .unwrap_or_else(|| skill_dir.to_path_buf());
    let mut hasher = Sha256::new();
    hasher.update(agent_id.as_str().as_bytes());
    hasher.update([0]);
    hasher.update(scope_key(scope).as_bytes());
    hasher.update([0]);
    hasher.update(canonical.as_str().as_bytes());
    hex::encode(hasher.finalize())
}

fn push_unique(
    instances: &mut Vec<SkillInstance>,
    seen: &mut HashSet<String>,
    candidates: Vec<SkillInstance>,
) {
    for instance in candidates {
        if seen.insert(instance.id.clone()) {
            instances.push(instance);
        }
    }
}

fn managed_by_path(skills: &[ManagedSkill]) -> HashMap<Utf8PathBuf, SkillId> {
    skills
        .iter()
        .map(|skill| (skill.managed_path.clone(), skill.id.clone()))
        .collect()
}

fn managed_by_name(skills: &[ManagedSkill]) -> HashMap<String, SkillId> {
    skills
        .iter()
        .map(|skill| (skill.name.clone(), skill.id.clone()))
        .collect()
}

fn expand_home(path: &Utf8Path, home_dir: &Utf8Path) -> Utf8PathBuf {
    if path == Utf8Path::new("~") {
        return home_dir.to_path_buf();
    }
    if let Some(rest) = path.as_str().strip_prefix("~/") {
        return home_dir.join(rest);
    }
    path.to_path_buf()
}

fn source_kind_for_root(root: &Utf8Path) -> SkillInstanceSourceKind {
    let components = root.components().map(|component| component.as_str());
    let mut previous = "";
    let mut source_kind = SkillInstanceSourceKind::AgentSpace;
    for component in components {
        if previous == "plugins" && component == "cache" {
            source_kind = SkillInstanceSourceKind::PluginCache;
            break;
        }
        if previous == ".codex" && component == "vendor_imports" {
            source_kind = SkillInstanceSourceKind::Vendor;
            break;
        }
        previous = component;
    }

    if matches!(source_kind, SkillInstanceSourceKind::PluginCache) {
        SkillInstanceSourceKind::PluginCache
    } else if matches!(source_kind, SkillInstanceSourceKind::Vendor) {
        SkillInstanceSourceKind::Vendor
    } else {
        SkillInstanceSourceKind::AgentSpace
    }
}

fn is_recursive_source(source_kind: &SkillInstanceSourceKind) -> bool {
    matches!(
        source_kind,
        SkillInstanceSourceKind::PluginCache | SkillInstanceSourceKind::Vendor
    )
}

fn has_toggle_file(skill_dir: &Utf8Path) -> bool {
    skill_markdown_path(skill_dir).exists() || disabled_skill_markdown_path(skill_dir).exists()
}

fn toggle_state(dir: &Utf8Path) -> ToggleState {
    let enabled = skill_markdown_path(dir).exists();
    let disabled = disabled_skill_markdown_path(dir).exists();
    match (enabled, disabled) {
        (true, false) => ToggleState::Enabled,
        (false, true) => ToggleState::Disabled,
        (true, true) => ToggleState::InvalidBothPresent,
        (false, false) => ToggleState::InvalidBothMissing,
    }
}

fn normalize_hash_path(path: &Utf8Path) -> String {
    if path.file_name() == Some("SKILL.md.disabled") {
        path.with_file_name("SKILL.md").to_string()
    } else {
        path.to_string()
    }
}

fn should_ignore_hash_path(path: &Utf8Path) -> bool {
    let file_name = path.file_name().unwrap_or_default();
    file_name == ".DS_Store"
        || (file_name.starts_with('.') && file_name.ends_with(".tmp"))
        || file_name.starts_with(".#")
        || file_name.ends_with('~')
        || file_name.ends_with(".swp")
        || file_name.ends_with(".swo")
        || file_name.starts_with(".skill-kits")
}

fn is_noisy_dir(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name,
            "node_modules"
                | "target"
                | "__pycache__"
                | "vendor"
                | "dist"
                | "build"
                | "venv"
                | ".venv"
        )
}

fn dir_name(path: &Utf8Path) -> String {
    path.file_name()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.to_string())
}

fn is_writable_dir(path: &Utf8Path) -> bool {
    fs::metadata(path.as_std_path())
        .map(|metadata| !metadata.permissions().readonly())
        .unwrap_or(false)
}

fn file_updated_at(path: &Utf8Path) -> Option<String> {
    let modified = fs::metadata(path.as_std_path()).ok()?.modified().ok()?;
    let secs = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(secs.to_string())
}

fn scope_key(scope: &SkillInstanceScope) -> String {
    match scope {
        SkillInstanceScope::Global => "global".to_string(),
        SkillInstanceScope::Project { path, .. } => format!("project:{path}"),
    }
}

fn scope_sort_key(scope: &SkillInstanceScope) -> String {
    match scope {
        SkillInstanceScope::Global => "0:global".to_string(),
        SkillInstanceScope::Project { name, path } => format!("1:{name}:{path}"),
    }
}

fn utf8_path(path: std::path::PathBuf) -> Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path).map_err(|path| SkillKitsError::InvalidSkillDir {
        path: Utf8PathBuf::from(path.to_string_lossy().to_string()),
        reason: "path is not UTF-8".to_string(),
    })
}
