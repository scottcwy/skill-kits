use crate::core::{
    agents::project_skill_dirs_for,
    error::{Result, SkillKitsError},
    fs::ensure_dir,
    hash::hash_skill_dir,
    ids::{unique_skill_id, AgentId, SkillId},
    paths::AppPaths,
    registry::{
        read_deployments_registry, read_skills_registry, write_deployments_registry,
        write_skills_registry, DeploymentRecord, DeploymentStatus, ManagedSkill, SkillSource,
        ToggleState,
    },
};
use camino::{Utf8Path, Utf8PathBuf};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

#[derive(Clone, Debug)]
pub struct ProjectDeployRequest<'a> {
    pub app_paths: &'a AppPaths,
    pub project_path: &'a Utf8Path,
    pub agent_id: &'a AgentId,
    pub skill_query: &'a str,
}

#[derive(Clone, Debug)]
pub struct ProjectSkillRequest<'a> {
    pub app_paths: &'a AppPaths,
    pub project_path: &'a Utf8Path,
    pub agent_id: &'a AgentId,
    pub skill_query: &'a str,
}

#[derive(Clone, Debug)]
pub struct ProjectRedeployRequest<'a> {
    pub app_paths: &'a AppPaths,
    pub project_path: &'a Utf8Path,
    pub agent_id: &'a AgentId,
    pub skill_query: &'a str,
    pub overwrite: bool,
    pub promote: bool,
}

#[derive(Clone, Debug)]
pub struct ProjectRemoveRequest<'a> {
    pub app_paths: &'a AppPaths,
    pub project_path: &'a Utf8Path,
    pub agent_id: &'a AgentId,
    pub skill_query: &'a str,
    pub force: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RedeployOutcome {
    Overwritten(DeploymentStatus),
    Promoted(ManagedSkill),
}

#[derive(Clone, Debug)]
pub struct ProjectScope {
    pub name: String,
    pub path: Utf8PathBuf,
}

pub fn resolve_project_scope(project_path: Option<&Utf8Path>) -> Result<ProjectScope> {
    let path = match project_path {
        Some(path) => path.to_path_buf(),
        None => {
            let cwd = std::env::current_dir().map_err(SkillKitsError::from)?;
            Utf8PathBuf::from_path_buf(cwd).map_err(|path| SkillKitsError::ProjectNotFound {
                path: Utf8PathBuf::from(path.to_string_lossy().to_string()),
            })?
        }
    };
    if !path.exists() {
        return Err(SkillKitsError::ProjectNotFound {
            path: path.to_path_buf(),
        });
    }
    Ok(ProjectScope {
        name: path
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| path.to_string()),
        path: path.to_path_buf(),
    })
}

pub fn project_deployment_status(
    app_paths: &AppPaths,
    project_path: &Utf8Path,
    agent_id: &AgentId,
    skill_query: &str,
) -> Result<DeploymentStatus> {
    let deployments = read_deployments_registry(app_paths)?;
    let deployment = deployment_for_query(
        &deployments.deployments,
        app_paths,
        project_path,
        agent_id,
        skill_query,
    )?;
    deployment_status(app_paths, deployment)
}

pub fn deploy_project_skill(request: ProjectDeployRequest<'_>) -> Result<DeploymentStatus> {
    let managed_skill = find_managed_skill(request.app_paths, request.skill_query)?;
    let deployment_root = project_skill_root(request.project_path, request.agent_id)?;
    let deployment_dir = deployment_root.join(&managed_skill.name);
    let enabled_path = deployment_dir.join("SKILL.md");
    let disabled_path = deployment_dir.join("SKILL.md.disabled");

    let existing_deployment = read_deployments_registry(request.app_paths)?
        .deployments
        .into_iter()
        .find(|record| {
            record.deployment_path == deployment_dir && record.skill_id == managed_skill.id
        });
    if deployment_dir.exists() {
        match existing_deployment {
            Some(record) => {
                let current_hash = hash_project_deployment_dir(&deployment_dir)?;
                if current_hash != record.baseline_hash {
                    return Err(SkillKitsError::DeploymentDrift {
                        deployment_id: record.id,
                    });
                }
            }
            None => {
                return Err(SkillKitsError::DeployConflict {
                    target: deployment_dir,
                });
            }
        }
    }

    if deployment_dir.exists() {
        std::fs::remove_dir_all(&deployment_dir)?;
    }
    copy_dir_contents(&managed_skill.managed_path, &deployment_dir)?;
    if disabled_path.exists() {
        std::fs::rename(&disabled_path, &enabled_path)?;
    }

    let managed_hash = hash_skill_dir(&managed_skill.managed_path)?;
    update_managed_skill_hash(request.app_paths, &managed_skill.id, &managed_hash)?;
    let deployed_hash = hash_project_deployment_dir(&deployment_dir)?;
    let deployment = upsert_deployment_record(
        request.app_paths,
        request.project_path,
        request.agent_id,
        &managed_skill,
        &deployment_dir,
        managed_hash,
        deployed_hash,
    )?;
    deployment_status(request.app_paths, &deployment)
}

pub fn enable_project_skill(request: ProjectSkillRequest<'_>) -> Result<DeploymentStatus> {
    let deployment = deployment_dir_for_request(&request)?;
    ensure_toggle_can_change(&deployment)?;
    let disabled = deployment.join("SKILL.md.disabled");
    let enabled = deployment.join("SKILL.md");
    if disabled.exists() {
        std::fs::rename(&disabled, &enabled)?;
    }
    refresh_status(
        request.app_paths,
        request.project_path,
        request.agent_id,
        request.skill_query,
    )
}

pub fn disable_project_skill(request: ProjectSkillRequest<'_>) -> Result<DeploymentStatus> {
    let deployment = deployment_dir_for_request(&request)?;
    ensure_toggle_can_change(&deployment)?;
    let disabled = deployment.join("SKILL.md.disabled");
    let enabled = deployment.join("SKILL.md");
    if enabled.exists() {
        std::fs::rename(&enabled, &disabled)?;
    }
    refresh_status(
        request.app_paths,
        request.project_path,
        request.agent_id,
        request.skill_query,
    )
}

pub fn redeploy_project_skill(request: ProjectRedeployRequest<'_>) -> Result<RedeployOutcome> {
    let mut status = project_deployment_status(
        request.app_paths,
        request.project_path,
        request.agent_id,
        request.skill_query,
    )?;
    if request.promote {
        let fork = promote_project_skill(
            request.app_paths,
            request.project_path,
            request.agent_id,
            request.skill_query,
            &status,
        )?;
        return Ok(RedeployOutcome::Promoted(fork));
    }
    if status.missing_managed_source {
        return Err(SkillKitsError::MissingManagedSource {
            skill_id: status.record.skill_id.clone(),
            deployment_id: status.record.id.clone(),
        });
    }
    if status.drift && !request.overwrite {
        return Err(SkillKitsError::DeploymentDrift {
            deployment_id: status.record.id,
        });
    }
    if request.overwrite || !status.drift {
        let managed = find_managed_skill(request.app_paths, status.record.skill_id.as_str())?;
        let deployment_dir = status.record.deployment_path.clone();
        overwrite_from_managed(&managed, &deployment_dir)?;
        let managed_hash = hash_skill_dir(&managed.managed_path)?;
        let deployed_hash = hash_project_deployment_dir(&deployment_dir)?;
        status = upsert_existing_deployment(
            request.app_paths,
            &status.record,
            managed_hash,
            deployed_hash,
        )?;
        return Ok(RedeployOutcome::Overwritten(status));
    }
    Err(SkillKitsError::DeploymentDrift {
        deployment_id: status.record.id,
    })
}

pub fn remove_project_skill(request: ProjectRemoveRequest<'_>) -> Result<()> {
    let status = project_deployment_status(
        request.app_paths,
        request.project_path,
        request.agent_id,
        request.skill_query,
    )?;
    if status.drift && !request.force {
        return Err(SkillKitsError::UnsafeRemoveRequiresForce {
            deployment_id: status.record.id,
        });
    }
    if status.record.deployment_path.exists() {
        std::fs::remove_dir_all(&status.record.deployment_path)?;
    }
    let mut deployments = read_deployments_registry(request.app_paths)?;
    deployments
        .deployments
        .retain(|record| record.id != status.record.id);
    write_deployments_registry(request.app_paths, &deployments)?;
    Ok(())
}

fn promote_project_skill(
    app_paths: &AppPaths,
    project_path: &Utf8Path,
    agent_id: &AgentId,
    skill_query: &str,
    status: &DeploymentStatus,
) -> Result<ManagedSkill> {
    let deployment_id = status.record.id.clone();
    let fork_name = format!("{}-promoted", status.record.skill_name);
    let project_hash = hash_skill_dir(&status.record.deployment_path)?;
    let mut skills = read_skills_registry(app_paths)?;
    let fork_id = unique_skill_id(
        &fork_name,
        &project_hash,
        skills.skills.iter().map(|skill| &skill.id),
    )
    .into_string();
    let managed_path = app_paths.skills_dir.join(&fork_id);
    copy_deployment_to_managed(&status.record.deployment_path, &managed_path)?;
    let content_hash = hash_skill_dir(&managed_path)?;
    let fork = ManagedSkill {
        id: SkillId::new(&fork_id),
        name: fork_name,
        source: SkillSource::PromotedFromProject {
            deployment_id,
            project_path: project_path.to_path_buf(),
        },
        managed_path: managed_path.clone(),
        content_hash: content_hash.clone(),
        metadata: None,
        created_at: now_string(),
        updated_at: now_string(),
    };
    skills.skills.push(fork.clone());
    write_skills_registry(app_paths, &skills)?;
    relink_deployment_to_fork(
        app_paths,
        &status.record,
        &fork.id,
        content_hash.clone(),
        content_hash,
    )?;
    let _ = (agent_id, skill_query);
    Ok(fork)
}

fn relink_deployment_to_fork(
    app_paths: &AppPaths,
    existing: &DeploymentRecord,
    fork_id: &SkillId,
    managed_hash: String,
    deployed_hash: String,
) -> Result<()> {
    let mut record = existing.clone();
    record.skill_id = fork_id.clone();
    record.deployed_from_hash = managed_hash;
    record.baseline_hash = deployed_hash;
    record.updated_at = now_string();
    let mut deployments = read_deployments_registry(app_paths)?;
    deployments
        .deployments
        .retain(|entry| entry.id != record.id);
    deployments.deployments.push(record);
    write_deployments_registry(app_paths, &deployments)
}

fn refresh_status(
    app_paths: &AppPaths,
    project_path: &Utf8Path,
    agent_id: &AgentId,
    skill_query: &str,
) -> Result<DeploymentStatus> {
    let deployments = read_deployments_registry(app_paths)?;
    let deployment = deployment_for_query(
        &deployments.deployments,
        app_paths,
        project_path,
        agent_id,
        skill_query,
    )?;
    deployment_status(app_paths, deployment)
}

fn deployment_dir_for_request(request: &ProjectSkillRequest<'_>) -> Result<Utf8PathBuf> {
    let status = project_deployment_status(
        request.app_paths,
        request.project_path,
        request.agent_id,
        request.skill_query,
    )?;
    Ok(status.record.deployment_path)
}

fn deployment_status(
    app_paths: &AppPaths,
    deployment: &DeploymentRecord,
) -> Result<DeploymentStatus> {
    let current_hash = if deployment.deployment_path.exists() {
        Some(hash_project_deployment_dir(&deployment.deployment_path)?)
    } else {
        None
    };
    let toggle = toggle_state(&deployment.deployment_path);
    let managed = read_skills_registry(app_paths)?;
    let missing_managed_source = !managed
        .skills
        .iter()
        .any(|skill| skill.id == deployment.skill_id);
    let outdated = managed
        .skills
        .iter()
        .find(|skill| skill.id == deployment.skill_id)
        .map(|skill| {
            !skill.content_hash.is_empty() && skill.content_hash != deployment.deployed_from_hash
        })
        .unwrap_or(false);
    let drift = current_hash
        .as_ref()
        .map(|hash| hash != &deployment.baseline_hash)
        .unwrap_or(false);
    Ok(DeploymentStatus {
        record: deployment.clone(),
        toggle,
        current_hash,
        drift,
        outdated,
        missing_managed_source,
    })
}

fn upsert_deployment_record(
    app_paths: &AppPaths,
    project_path: &Utf8Path,
    agent_id: &AgentId,
    managed_skill: &ManagedSkill,
    deployment_dir: &Utf8Path,
    managed_hash: String,
    deployed_hash: String,
) -> Result<DeploymentRecord> {
    let project_name = project_path
        .file_name()
        .map(|name| name.to_string())
        .unwrap_or_else(|| project_path.to_string());
    let record = DeploymentRecord {
        id: deployment_id(agent_id, &managed_skill.id, project_path),
        skill_id: managed_skill.id.clone(),
        agent_id: agent_id.clone(),
        project_name,
        project_path: project_path.to_path_buf(),
        deployment_path: deployment_dir.to_path_buf(),
        skill_name: managed_skill.name.clone(),
        baseline_hash: deployed_hash,
        deployed_from_hash: managed_hash,
        created_at: now_string(),
        updated_at: now_string(),
    };
    let mut deployments = read_deployments_registry(app_paths)?;
    deployments
        .deployments
        .retain(|existing| existing.id != record.id);
    deployments.deployments.push(record.clone());
    write_deployments_registry(app_paths, &deployments)?;
    Ok(record)
}

fn upsert_existing_deployment(
    app_paths: &AppPaths,
    existing: &DeploymentRecord,
    managed_hash: String,
    deployed_hash: String,
) -> Result<DeploymentStatus> {
    let mut record = existing.clone();
    record.deployed_from_hash = managed_hash;
    record.baseline_hash = deployed_hash;
    record.updated_at = now_string();
    let mut deployments = read_deployments_registry(app_paths)?;
    deployments
        .deployments
        .retain(|entry| entry.id != record.id);
    deployments.deployments.push(record.clone());
    write_deployments_registry(app_paths, &deployments)?;
    deployment_status(app_paths, &record)
}

fn project_skill_root(project_path: &Utf8Path, agent_id: &AgentId) -> Result<Utf8PathBuf> {
    let dirs = project_skill_dirs_for(agent_id).ok_or_else(|| SkillKitsError::AgentNotFound {
        agent_id: agent_id.clone(),
    })?;
    Ok(project_path.join(dirs.first().expect("built-in agents have project dirs")))
}

fn deployment_for_query<'a>(
    deployments: &'a [DeploymentRecord],
    _app_paths: &AppPaths,
    project_path: &Utf8Path,
    agent_id: &AgentId,
    skill_query: &str,
) -> Result<&'a DeploymentRecord> {
    let deployment_root = project_skill_root(project_path, agent_id)?;
    deployments
        .iter()
        .find(|deployment| {
            deployment.project_path == project_path
                && deployment.agent_id == *agent_id
                && (deployment.skill_name == skill_query
                    || deployment.deployment_path == deployment_root.join(skill_query))
        })
        .ok_or_else(|| SkillKitsError::SkillNotFound {
            query: skill_query.to_string(),
        })
}

fn find_managed_skill(app_paths: &AppPaths, skill_query: &str) -> Result<ManagedSkill> {
    let skills = read_skills_registry(app_paths)?;
    skills
        .skills
        .into_iter()
        .find(|skill| skill.id.as_str() == skill_query || skill.name == skill_query)
        .ok_or_else(|| SkillKitsError::SkillNotFound {
            query: skill_query.to_string(),
        })
}

fn overwrite_from_managed(managed_skill: &ManagedSkill, deployment_dir: &Utf8Path) -> Result<()> {
    if deployment_dir.exists() {
        std::fs::remove_dir_all(deployment_dir)?;
    }
    copy_dir_contents(&managed_skill.managed_path, deployment_dir)?;
    let disabled = deployment_dir.join("SKILL.md.disabled");
    if disabled.exists() {
        std::fs::rename(&disabled, deployment_dir.join("SKILL.md"))?;
    }
    Ok(())
}

pub(crate) fn copy_deployment_to_managed(
    source_dir: &Utf8Path,
    target_dir: &Utf8Path,
) -> Result<()> {
    copy_dir_contents(source_dir, target_dir)
}

fn copy_dir_contents(source_dir: &Utf8Path, target_dir: &Utf8Path) -> Result<()> {
    if target_dir.exists() {
        std::fs::remove_dir_all(target_dir)?;
    }
    ensure_dir(target_dir)?;
    for entry in WalkDir::new(source_dir).min_depth(1) {
        let entry = entry.map_err(|err| SkillKitsError::InvalidSkillDir {
            path: source_dir.to_path_buf(),
            reason: err.to_string(),
        })?;
        let entry_path =
            Utf8PathBuf::from_path_buf(entry.path().to_path_buf()).map_err(|path| {
                SkillKitsError::InvalidSkillDir {
                    path: Utf8PathBuf::from(path.to_string_lossy().to_string()),
                    reason: "path is not UTF-8".to_string(),
                }
            })?;
        let rel = entry_path.strip_prefix(source_dir).unwrap();
        let target = target_dir.join(rel);
        if entry.file_type().is_dir() {
            ensure_dir(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                ensure_dir(parent)?;
            }
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

pub(crate) fn hash_project_deployment_dir(dir: &Utf8Path) -> Result<String> {
    let mut files = Vec::new();
    for entry in WalkDir::new(dir).follow_links(false) {
        let entry = entry.map_err(|err| SkillKitsError::InvalidSkillDir {
            path: dir.to_path_buf(),
            reason: err.to_string(),
        })?;
        if entry.file_type().is_dir() {
            continue;
        }

        let path = Utf8PathBuf::from_path_buf(entry.path().to_path_buf()).map_err(|path| {
            SkillKitsError::InvalidSkillDir {
                path: Utf8PathBuf::from(path.to_string_lossy().to_string()),
                reason: "path is not UTF-8".to_string(),
            }
        })?;
        let rel = path.strip_prefix(dir).unwrap();
        if should_ignore_hash_path(rel) {
            continue;
        }
        files.push(path);
    }

    files.sort();
    let mut hasher = Sha256::new();
    for file in files {
        let rel = file.strip_prefix(dir).unwrap();
        let normalized = normalize_project_hash_path(rel);
        hasher.update(normalized.as_bytes());
        hasher.update([0]);
        let bytes = std::fs::read(file.as_std_path())?;
        hasher.update(bytes.len().to_le_bytes());
        hasher.update([0]);
        hasher.update(bytes);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn normalize_project_hash_path(path: &Utf8Path) -> String {
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

fn toggle_state(dir: &Utf8Path) -> ToggleState {
    let enabled = dir.join("SKILL.md").exists();
    let disabled = dir.join("SKILL.md.disabled").exists();
    match (enabled, disabled) {
        (true, false) => ToggleState::Enabled,
        (false, true) => ToggleState::Disabled,
        (true, true) => ToggleState::InvalidBothPresent,
        (false, false) => ToggleState::InvalidBothMissing,
    }
}

fn ensure_toggle_can_change(dir: &Utf8Path) -> Result<()> {
    match toggle_state(dir) {
        ToggleState::Enabled | ToggleState::Disabled => Ok(()),
        ToggleState::InvalidBothPresent | ToggleState::InvalidBothMissing => {
            Err(SkillKitsError::InvalidToggleState {
                path: dir.to_path_buf(),
            })
        }
    }
}

pub(crate) fn deployment_id(
    agent_id: &AgentId,
    skill_id: &SkillId,
    project_path: &Utf8Path,
) -> String {
    format!(
        "{}-{}-{}",
        agent_id.as_str(),
        skill_id.as_str(),
        short_hash(project_path.as_str())
    )
}

pub(crate) fn now_string() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
}

fn short_hash(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let hash = hex::encode(hasher.finalize());
    hash[..8].to_string()
}

fn update_managed_skill_hash(app_paths: &AppPaths, skill_id: &SkillId, hash: &str) -> Result<()> {
    let mut skills = read_skills_registry(app_paths)?;
    if let Some(skill) = skills.skills.iter_mut().find(|skill| skill.id == *skill_id) {
        skill.content_hash = hash.to_string();
        skill.updated_at = now_string();
    }
    write_skills_registry(app_paths, &skills)
}
