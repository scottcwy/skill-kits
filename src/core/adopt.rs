use crate::core::{
    agents::{configured_global_skill_dirs_for, configured_project_skill_dirs_for},
    error::{Result, SkillKitsError},
    fs::{copy_dir_clean_source_to_empty_target, ensure_dir},
    hash::hash_skill_dir,
    ids::{unique_skill_id, AgentId, SkillId},
    paths::AppPaths,
    project::{copy_deployment_to_managed, deployment_id, hash_project_deployment_dir, now_string},
    registry::{
        read_skills_registry, update_registry_files, write_skills_registry, DeploymentRecord,
        ManagedSkill, SkillSource,
    },
    skills::{disabled_skill_markdown_path, load_skill_metadata, skill_markdown_path},
};
use camino::{Utf8Path, Utf8PathBuf};

#[derive(Clone, Debug)]
pub struct GlobalAgentAdoptRequest<'a> {
    pub app_paths: &'a AppPaths,
    pub agent_id: &'a AgentId,
    pub home_dir: &'a Utf8Path,
}

#[derive(Clone, Debug)]
pub struct ImportManagedCopyRequest<'a> {
    pub app_paths: &'a AppPaths,
    pub agent_id: &'a AgentId,
    pub source_path: &'a Utf8Path,
}

#[derive(Clone, Debug)]
pub struct ProjectAdoptRequest<'a> {
    pub app_paths: &'a AppPaths,
    pub project_path: &'a Utf8Path,
    pub agent_id: &'a AgentId,
    pub skill_name: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdoptReport {
    pub imported: usize,
    pub conflicts: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResilientAdoptReport {
    pub imported: usize,
    pub conflicts: usize,
    pub failures: usize,
}

pub fn global_agent_adopt(request: GlobalAgentAdoptRequest<'_>) -> Result<AdoptReport> {
    let global_roots = configured_global_skill_dirs_for(request.app_paths, request.agent_id)?;
    let mut imported = 0;
    let mut conflicts = 0;

    for global_root in global_roots {
        let global_root = expand_home(&global_root, request.home_dir);
        if !global_root.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&global_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let source_path = Utf8PathBuf::from_path_buf(entry.path()).map_err(|path| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("path is not UTF-8: {}", path.display()),
                )
            })?;
            if !is_global_adoptable_skill_dir(&source_path) {
                continue;
            }
            match adopt_global_skill(&request, &source_path)? {
                GlobalAdoptOutcome::Imported => imported += 1,
                GlobalAdoptOutcome::Skipped => {}
                GlobalAdoptOutcome::Conflict => conflicts += 1,
            }
        }
    }

    Ok(AdoptReport {
        imported,
        conflicts,
    })
}

pub fn import_managed_copy(request: ImportManagedCopyRequest<'_>) -> Result<ManagedSkill> {
    let skill_name = request
        .source_path
        .file_name()
        .map(|name| name.to_string())
        .unwrap_or_else(|| "skill".to_string());
    match adopt_global_skill(
        &GlobalAgentAdoptRequest {
            app_paths: request.app_paths,
            agent_id: request.agent_id,
            home_dir: Utf8Path::new(""),
        },
        request.source_path,
    )? {
        GlobalAdoptOutcome::Imported | GlobalAdoptOutcome::Skipped => {
            let skills = read_skills_registry(request.app_paths)?.skills;
            let source_match = skills.iter().position(|skill| {
                skill.name == skill_name
                    && matches!(
                        &skill.source,
                        SkillSource::GlobalAgentAdopt { source_path, .. }
                            if source_path == request.source_path
                    )
            });
            let name_match = skills.iter().position(|skill| skill.name == skill_name);
            source_match
                .or(name_match)
                .map(|index| skills[index].clone())
                .ok_or(SkillKitsError::SkillNotFound { query: skill_name })
        }
        GlobalAdoptOutcome::Conflict => Err(SkillKitsError::AdoptionConflict { name: skill_name }),
    }
}

pub fn global_agent_adopt_resilient(
    request: GlobalAgentAdoptRequest<'_>,
) -> Result<ResilientAdoptReport> {
    let global_roots = configured_global_skill_dirs_for(request.app_paths, request.agent_id)?;
    let mut imported = 0;
    let mut conflicts = 0;
    let mut failures = 0;
    let mut visited = Vec::new();

    for global_root in global_roots {
        let global_root = expand_home(&global_root, request.home_dir);
        if !global_root.exists() {
            continue;
        }
        let (skill_dirs, root_failures) = discover_adoptable_skill_dirs(&global_root);
        failures += root_failures;
        for source_path in skill_dirs {
            if visited.contains(&source_path) {
                continue;
            }
            visited.push(source_path.clone());
            match adopt_global_skill(&request, &source_path) {
                Ok(GlobalAdoptOutcome::Imported) => imported += 1,
                Ok(GlobalAdoptOutcome::Skipped) => {}
                Ok(GlobalAdoptOutcome::Conflict) => conflicts += 1,
                Err(_) => failures += 1,
            }
        }
    }

    Ok(ResilientAdoptReport {
        imported,
        conflicts,
        failures,
    })
}

fn discover_adoptable_skill_dirs(root: &Utf8Path) -> (Vec<Utf8PathBuf>, usize) {
    let mut discovered = Vec::new();
    let mut failures = 0;
    discover_adoptable_skill_dirs_inner(root, &mut discovered, &mut failures);
    discovered.sort();
    discovered.dedup();
    (discovered, failures)
}

fn discover_adoptable_skill_dirs_inner(
    dir: &Utf8Path,
    discovered: &mut Vec<Utf8PathBuf>,
    failures: &mut usize,
) {
    if is_global_adoptable_skill_dir(dir) {
        discovered.push(dir.to_path_buf());
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => {
            *failures += 1;
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                *failures += 1;
                continue;
            }
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => {
                *failures += 1;
                continue;
            }
        };
        if !file_type.is_dir() {
            continue;
        }
        let child = match Utf8PathBuf::from_path_buf(entry.path()) {
            Ok(child) => child,
            Err(_) => {
                *failures += 1;
                continue;
            }
        };
        discover_adoptable_skill_dirs_inner(&child, discovered, failures);
    }
}

pub fn project_adopt(request: ProjectAdoptRequest<'_>) -> Result<AdoptReport> {
    let deployment_dir = project_skill_dir(
        request.app_paths,
        request.project_path,
        request.agent_id,
        request.skill_name,
    )?;
    if !deployment_dir.exists() {
        return Err(SkillKitsError::SkillNotFound {
            query: request.skill_name.to_string(),
        });
    }
    let managed_hash = hash_skill_dir(&deployment_dir)?;
    let baseline_hash = hash_project_deployment_dir(&deployment_dir)?;
    let existing_skill = read_skills_registry(request.app_paths)?
        .skills
        .into_iter()
        .find(|skill| skill.name == request.skill_name);
    if let Some(existing) = existing_skill.as_ref() {
        if existing.content_hash != managed_hash {
            return Err(SkillKitsError::AdoptionConflict {
                name: request.skill_name.to_string(),
            });
        }
        record_project_baseline(&request, &deployment_dir, existing, &baseline_hash)?;
        return Ok(AdoptReport {
            imported: 0,
            conflicts: 0,
        });
    }
    let skills = read_skills_registry(request.app_paths)?;
    let skill_id = unique_skill_id(
        request.skill_name,
        &managed_hash,
        skills.skills.iter().map(|skill| &skill.id),
    )
    .into_string();
    let managed_path = request.app_paths.skills_dir.join(&skill_id);
    copy_deployment_to_managed(&deployment_dir, &managed_path)?;
    let managed_skill = ManagedSkill {
        id: SkillId::new(&skill_id),
        name: request.skill_name.to_string(),
        source: SkillSource::ProjectAdopt {
            agent_id: request.agent_id.clone(),
            project_path: request.project_path.to_path_buf(),
            source_path: deployment_dir.clone(),
        },
        managed_path,
        content_hash: managed_hash.clone(),
        metadata: None,
        created_at: now_string(),
        updated_at: now_string(),
    };
    record_new_project_adopt(&request, &deployment_dir, managed_skill, &baseline_hash)?;
    Ok(AdoptReport {
        imported: 1,
        conflicts: 0,
    })
}

pub fn project_adopt_conflict_as_new(request: ProjectAdoptRequest<'_>) -> Result<AdoptReport> {
    let deployment_dir = project_skill_dir(
        request.app_paths,
        request.project_path,
        request.agent_id,
        request.skill_name,
    )?;
    if !deployment_dir.exists() {
        return Err(SkillKitsError::SkillNotFound {
            query: request.skill_name.to_string(),
        });
    }
    let managed_hash = hash_skill_dir(&deployment_dir)?;
    let baseline_hash = hash_project_deployment_dir(&deployment_dir)?;
    let skills = read_skills_registry(request.app_paths)?;
    let skill_id = unique_skill_id(
        request.skill_name,
        &managed_hash,
        skills.skills.iter().map(|skill| &skill.id),
    )
    .into_string();
    let managed_path = request.app_paths.skills_dir.join(&skill_id);
    copy_deployment_to_managed(&deployment_dir, &managed_path)?;
    let managed_skill = ManagedSkill {
        id: SkillId::new(&skill_id),
        name: request.skill_name.to_string(),
        source: SkillSource::ProjectAdopt {
            agent_id: request.agent_id.clone(),
            project_path: request.project_path.to_path_buf(),
            source_path: deployment_dir.clone(),
        },
        managed_path,
        content_hash: managed_hash.clone(),
        metadata: None,
        created_at: now_string(),
        updated_at: now_string(),
    };
    record_new_project_adopt(&request, &deployment_dir, managed_skill, &baseline_hash)?;
    Ok(AdoptReport {
        imported: 1,
        conflicts: 0,
    })
}

pub fn project_adopt_all(request: ProjectAdoptRequest<'_>) -> Result<AdoptReport> {
    let mut imported = 0;
    let mut conflicts = 0;
    let deployment_root =
        project_skill_root(request.app_paths, request.project_path, request.agent_id)?;
    if !deployment_root.exists() {
        return Ok(AdoptReport {
            imported,
            conflicts,
        });
    }
    for entry in std::fs::read_dir(&deployment_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let skill_name = entry.file_name().to_string_lossy().to_string();
        match project_adopt(ProjectAdoptRequest {
            app_paths: request.app_paths,
            project_path: request.project_path,
            agent_id: request.agent_id,
            skill_name: &skill_name,
        }) {
            Ok(report) => imported += report.imported,
            Err(SkillKitsError::AdoptionConflict { .. }) => conflicts += 1,
            Err(err) => return Err(err),
        }
    }
    Ok(AdoptReport {
        imported,
        conflicts,
    })
}

enum GlobalAdoptOutcome {
    Imported,
    Skipped,
    Conflict,
}

fn adopt_global_skill(
    request: &GlobalAgentAdoptRequest<'_>,
    source_path: &Utf8Path,
) -> Result<GlobalAdoptOutcome> {
    let mut skills = read_skills_registry(request.app_paths)?;
    let skill_name = source_path
        .file_name()
        .map(|name| name.to_string())
        .unwrap_or_else(|| "skill".to_string());
    let content_hash = hash_skill_dir(source_path)?;

    if let Some(existing) = skills.skills.iter().find(|skill| skill.name == skill_name) {
        if existing.content_hash == content_hash {
            return Ok(GlobalAdoptOutcome::Skipped);
        }
        return Ok(GlobalAdoptOutcome::Conflict);
    }

    ensure_dir(&request.app_paths.skills_dir)?;
    ensure_dir(&request.app_paths.registry_dir)?;
    ensure_dir(&request.app_paths.locks_dir)?;

    let skill_id = unique_skill_id(
        &skill_name,
        &content_hash,
        skills.skills.iter().map(|skill| &skill.id),
    );
    let managed_path = request.app_paths.skills_dir.join(skill_id.as_str());
    copy_dir_clean_source_to_empty_target(source_path, &managed_path)?;
    let created_at = now_string();
    let metadata = if skill_markdown_path(source_path).is_file() {
        load_skill_metadata(source_path)?
    } else {
        None
    };
    let managed_skill = ManagedSkill {
        id: skill_id,
        name: skill_name,
        source: SkillSource::GlobalAgentAdopt {
            agent_id: request.agent_id.clone(),
            source_path: source_path.to_path_buf(),
        },
        managed_path,
        content_hash,
        metadata,
        created_at: created_at.clone(),
        updated_at: created_at,
    };
    skills.skills.push(managed_skill);
    skills
        .skills
        .sort_by(|left, right| left.name.cmp(&right.name));
    write_skills_registry(request.app_paths, &skills)?;
    Ok(GlobalAdoptOutcome::Imported)
}

fn is_global_adoptable_skill_dir(path: &Utf8Path) -> bool {
    skill_markdown_path(path).is_file() || disabled_skill_markdown_path(path).is_file()
}

fn expand_home(path: &Utf8Path, home_dir: &Utf8Path) -> Utf8PathBuf {
    if let Some(rest) = path.as_str().strip_prefix("~/") {
        home_dir.join(rest)
    } else if path.as_str() == "~" {
        home_dir.to_path_buf()
    } else {
        path.to_path_buf()
    }
}

fn project_skill_root(
    app_paths: &AppPaths,
    project_path: &Utf8Path,
    agent_id: &AgentId,
) -> Result<Utf8PathBuf> {
    let dirs = configured_project_skill_dirs_for(app_paths, agent_id)?;
    Ok(project_path.join(dirs.first().expect("agents have project dirs")))
}

fn project_skill_dir(
    app_paths: &AppPaths,
    project_path: &Utf8Path,
    agent_id: &AgentId,
    skill_name: &str,
) -> Result<Utf8PathBuf> {
    Ok(project_skill_root(app_paths, project_path, agent_id)?.join(skill_name))
}

fn record_project_baseline(
    request: &ProjectAdoptRequest<'_>,
    deployment_dir: &Utf8Path,
    skill: &ManagedSkill,
    baseline_hash: &str,
) -> Result<()> {
    let record = project_adopt_deployment_record(request, deployment_dir, skill, baseline_hash);
    update_registry_files(request.app_paths, |registries| {
        registries
            .deployments
            .deployments
            .retain(|existing| existing.id != record.id);
        registries.deployments.deployments.push(record);
        registries.write_deployments = true;
        Ok(())
    })
}

fn record_new_project_adopt(
    request: &ProjectAdoptRequest<'_>,
    deployment_dir: &Utf8Path,
    managed_skill: ManagedSkill,
    baseline_hash: &str,
) -> Result<()> {
    update_registry_files(request.app_paths, |registries| {
        let record =
            project_adopt_deployment_record(request, deployment_dir, &managed_skill, baseline_hash);
        registries.skills.skills.push(managed_skill);
        registries
            .deployments
            .deployments
            .retain(|existing| existing.id != record.id);
        registries.deployments.deployments.push(record);
        registries.write_skills = true;
        registries.write_deployments = true;
        Ok(())
    })
}

fn project_adopt_deployment_record(
    request: &ProjectAdoptRequest<'_>,
    deployment_dir: &Utf8Path,
    skill: &ManagedSkill,
    baseline_hash: &str,
) -> DeploymentRecord {
    let project_name = request
        .project_path
        .file_name()
        .map(|name| name.to_string())
        .unwrap_or_else(|| request.project_path.to_string());
    DeploymentRecord {
        id: deployment_id(request.agent_id, &skill.id, request.project_path),
        skill_id: skill.id.clone(),
        agent_id: request.agent_id.clone(),
        project_name,
        project_path: request.project_path.to_path_buf(),
        deployment_path: deployment_dir.to_path_buf(),
        skill_name: request.skill_name.to_string(),
        baseline_hash: baseline_hash.to_string(),
        deployed_from_hash: skill.content_hash.clone(),
        created_at: now_string(),
        updated_at: now_string(),
    }
}
