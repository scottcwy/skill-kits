use crate::core::{
    fs::{copy_dir_clean_source_to_empty_target, ensure_dir},
    hash::hash_skill_dir,
    ids::{generate_skill_id, unique_skill_id, SkillId},
    paths::AppPaths,
    registry::{
        read_skills_registry, update_skills_registry, write_skills_registry, ManagedSkill,
        SkillSource, SkillsRegistry,
    },
    scan::scan_skill_dir,
    skills::{load_skill_metadata, validate_skill_dir},
    Result, SkillKitsError,
};
use camino::Utf8Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct InstallLocalRequest<'a> {
    pub source_path: &'a Utf8Path,
}

pub struct InstallLocalResult {
    pub skill: ManagedSkill,
    pub registry: SkillsRegistry,
    pub risk_findings: Vec<crate::core::scan::RiskFinding>,
}

pub struct UninstallSkillRequest<'a> {
    pub app_paths: &'a AppPaths,
    pub query: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UninstallSkillResult {
    pub skill_id: SkillId,
}

pub fn install_local_skill(
    request: InstallLocalRequest<'_>,
    app_paths: &AppPaths,
) -> Result<InstallLocalResult> {
    let source_path = request.source_path;
    validate_skill_dir(source_path)?;

    let content_hash = hash_skill_dir(source_path)?;
    let name = source_path
        .file_name()
        .map(|name| name.to_string())
        .unwrap_or_else(|| "skill".to_string());
    let source_path_for_registry = normalize_path(source_path);

    ensure_dir(&app_paths.skills_dir)?;
    ensure_dir(&app_paths.registry_dir)?;
    ensure_dir(&app_paths.locks_dir)?;

    let metadata = load_skill_metadata(source_path)?;
    let created_at = now_rfc3339();

    let (skill, registry) = update_skills_registry(app_paths, |registry| {
        let existing_ids: Vec<_> = registry
            .skills
            .iter()
            .map(|skill| skill.id.clone())
            .collect();
        let base_id = generate_skill_id(&name, &content_hash);
        let skill_id = if existing_ids
            .iter()
            .any(|existing| existing.as_str() == base_id.as_str())
        {
            unique_skill_id(&name, &content_hash, existing_ids.iter())
        } else {
            base_id
        };
        let managed_path = app_paths.skills_dir.join(skill_id.as_str());

        if managed_path.exists() {
            return Err(SkillKitsError::DeployConflict {
                target: managed_path,
            });
        }

        copy_dir_clean_source_to_empty_target(source_path, &managed_path)?;

        let skill = ManagedSkill {
            id: skill_id,
            name,
            source: SkillSource::Local {
                source_path: source_path_for_registry,
            },
            managed_path: managed_path.clone(),
            content_hash,
            metadata,
            created_at: created_at.clone(),
            updated_at: created_at,
        };

        registry.skills.push(skill.clone());
        Ok((skill, registry.clone()))
    })?;

    let risk_findings = scan_skill_dir(source_path)?;
    Ok(InstallLocalResult {
        skill,
        registry,
        risk_findings,
    })
}

pub fn uninstall_skill(request: UninstallSkillRequest<'_>) -> Result<UninstallSkillResult> {
    let mut registry = read_skills_registry(request.app_paths)?;
    let matches = registry
        .skills
        .iter()
        .enumerate()
        .filter(|(_, skill)| skill.id.as_str() == request.query || skill.name == request.query)
        .map(|(index, skill)| (index, skill.id.clone(), skill.managed_path.clone()))
        .collect::<Vec<_>>();

    let (index, skill_id, managed_path) = match matches.as_slice() {
        [] => {
            return Err(SkillKitsError::SkillNotFound {
                query: request.query.to_string(),
            })
        }
        [entry] => entry.clone(),
        entries => {
            return Err(SkillKitsError::AmbiguousSkill {
                query: request.query.to_string(),
                matches: entries.iter().map(|(_, id, _)| id.clone()).collect(),
            })
        }
    };

    if managed_path.exists() {
        std::fs::remove_dir_all(&managed_path)?;
    }
    registry.skills.remove(index);
    write_skills_registry(request.app_paths, &registry)?;
    Ok(UninstallSkillResult { skill_id })
}

fn now_rfc3339() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_unix_seconds_rfc3339(seconds)
}

fn normalize_path(path: &Utf8Path) -> camino::Utf8PathBuf {
    std::fs::canonicalize(path)
        .ok()
        .and_then(|path| camino::Utf8PathBuf::from_path_buf(path).ok())
        .unwrap_or_else(|| path.to_path_buf())
}

fn format_unix_seconds_rfc3339(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_unix_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_unix_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year, month as u32, day as u32)
}
