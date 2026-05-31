use crate::core::{
    config::{read_config, write_config},
    paths::{ensure_app_dirs, AppPaths},
    project,
    registry::{read_deployments_registry, read_skills_registry, DeploymentRecord, ToggleState},
    Result,
};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;
use std::fs;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub fixed_count: usize,
    pub issues: Vec<DoctorIssue>,
}

impl DoctorReport {
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.severity == DoctorSeverity::Error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DoctorIssue {
    pub code: DoctorIssueCode,
    pub severity: DoctorSeverity,
    pub path: Option<Utf8PathBuf>,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorIssueCode {
    DataRootUnavailable,
    InvalidToml,
    MissingManagedDirectory,
    MissingSkillMarkdown,
    StaleLock,
    ActiveLock,
    StrandedTempFile,
    InvalidAgentProjectDirectory,
    MissingRecentProject,
    MissingProject,
    InvalidToggle,
    MissingDeployment,
    MissingManagedSource,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorSeverity {
    Error,
    Warning,
    Fixed,
}

pub fn run_doctor(paths: &AppPaths, fix: bool) -> Result<DoctorReport> {
    let mut issues = Vec::new();
    let mut fixed_count = 0;

    if let Err(error) = ensure_app_dirs(paths) {
        issues.push(issue(
            DoctorIssueCode::DataRootUnavailable,
            DoctorSeverity::Error,
            Some(paths.data_root.clone()),
            format!("data root is unavailable: {error}"),
        ));
        return Ok(report(issues, fixed_count));
    }

    check_lock(paths, fix, &mut issues, &mut fixed_count)?;
    check_temp_files(paths, fix, &mut issues, &mut fixed_count)?;
    check_config(paths, fix, &mut issues, &mut fixed_count)?;
    let skills = check_skills(paths, &mut issues);
    check_deployments(paths, skills.as_ref().ok(), &mut issues);

    Ok(report(issues, fixed_count))
}

fn check_config(
    paths: &AppPaths,
    fix: bool,
    issues: &mut Vec<DoctorIssue>,
    fixed_count: &mut usize,
) -> Result<()> {
    let mut config = match read_config(paths) {
        Ok(config) => config,
        Err(error) => {
            issues.push(issue(
                DoctorIssueCode::InvalidToml,
                DoctorSeverity::Error,
                Some(paths.config_file.clone()),
                format!("config TOML is invalid: {error}"),
            ));
            return Ok(());
        }
    };

    for agent in &config.agents {
        for dir in &agent.project_skill_dirs {
            if dir.is_absolute() {
                issues.push(issue(
                    DoctorIssueCode::InvalidAgentProjectDirectory,
                    DoctorSeverity::Error,
                    Some(dir.clone()),
                    format!(
                        "agent {} project skill directory must be relative",
                        agent.id
                    ),
                ));
            }
        }
    }

    let original_len = config.recent_projects.len();
    let mut missing = Vec::new();
    config.recent_projects.retain(|project| {
        let exists = project.path.exists();
        if !exists {
            missing.push(project.path.clone());
        }
        exists || !fix
    });

    for path in missing {
        let severity = if fix {
            DoctorSeverity::Fixed
        } else {
            DoctorSeverity::Warning
        };
        issues.push(issue(
            DoctorIssueCode::MissingRecentProject,
            severity,
            Some(path),
            if fix {
                "removed missing Recent Project".to_string()
            } else {
                "Recent Project path no longer exists".to_string()
            },
        ));
    }

    if fix && config.recent_projects.len() != original_len {
        write_config(paths, &config)?;
        *fixed_count += original_len - config.recent_projects.len();
    }

    Ok(())
}

fn check_skills(
    paths: &AppPaths,
    issues: &mut Vec<DoctorIssue>,
) -> std::result::Result<crate::core::registry::SkillsRegistry, ()> {
    let skills = match read_skills_registry(paths) {
        Ok(skills) => skills,
        Err(error) => {
            issues.push(issue(
                DoctorIssueCode::InvalidToml,
                DoctorSeverity::Error,
                Some(paths.skills_registry_file.clone()),
                format!("skills registry TOML is invalid: {error}"),
            ));
            return Err(());
        }
    };

    for skill in &skills.skills {
        if !skill.managed_path.exists() {
            issues.push(issue(
                DoctorIssueCode::MissingManagedDirectory,
                DoctorSeverity::Error,
                Some(skill.managed_path.clone()),
                format!("managed Skill directory for {} is missing", skill.id),
            ));
            continue;
        }
        if !skill.managed_path.join("SKILL.md").exists()
            && !skill.managed_path.join("SKILL.md.disabled").exists()
        {
            issues.push(issue(
                DoctorIssueCode::MissingSkillMarkdown,
                DoctorSeverity::Error,
                Some(skill.managed_path.clone()),
                format!("managed Skill {} is missing SKILL.md", skill.id),
            ));
        }
    }

    Ok(skills)
}

fn check_deployments(
    paths: &AppPaths,
    skills: Option<&crate::core::registry::SkillsRegistry>,
    issues: &mut Vec<DoctorIssue>,
) {
    let deployments = match read_deployments_registry(paths) {
        Ok(deployments) => deployments,
        Err(error) => {
            issues.push(issue(
                DoctorIssueCode::InvalidToml,
                DoctorSeverity::Error,
                Some(paths.deployments_registry_file.clone()),
                format!("deployments registry TOML is invalid: {error}"),
            ));
            return;
        }
    };

    for deployment in &deployments.deployments {
        if !deployment.project_path.exists() {
            issues.push(issue(
                DoctorIssueCode::MissingProject,
                DoctorSeverity::Warning,
                Some(deployment.project_path.clone()),
                format!("recorded project {} is missing", deployment.project_name),
            ));
            continue;
        }
        if !deployment.deployment_path.exists() {
            issues.push(issue(
                DoctorIssueCode::MissingDeployment,
                DoctorSeverity::Warning,
                Some(deployment.deployment_path.clone()),
                format!("deployment {} is missing", deployment.id),
            ));
        }
        if has_missing_managed_source(skills, deployment) {
            issues.push(issue(
                DoctorIssueCode::MissingManagedSource,
                DoctorSeverity::Error,
                Some(deployment.deployment_path.clone()),
                format!(
                    "deployment {} references missing managed source {}",
                    deployment.id, deployment.skill_id
                ),
            ));
        }
        match project::project_deployment_status(
            paths,
            &deployment.project_path,
            &deployment.agent_id,
            &deployment.skill_name,
        ) {
            Ok(status) => match status.toggle {
                ToggleState::Enabled | ToggleState::Disabled => {}
                ToggleState::InvalidBothPresent | ToggleState::InvalidBothMissing => {
                    issues.push(issue(
                        DoctorIssueCode::InvalidToggle,
                        DoctorSeverity::Error,
                        Some(deployment.deployment_path.clone()),
                        format!("deployment {} has invalid toggle state", deployment.id),
                    ));
                }
            },
            Err(error) => issues.push(issue(
                DoctorIssueCode::MissingDeployment,
                DoctorSeverity::Warning,
                Some(deployment.deployment_path.clone()),
                format!(
                    "deployment {} status could not be read: {error}",
                    deployment.id
                ),
            )),
        }
    }
}

fn check_lock(
    paths: &AppPaths,
    fix: bool,
    issues: &mut Vec<DoctorIssue>,
    fixed_count: &mut usize,
) -> Result<()> {
    if !paths.state_lock.exists() {
        return Ok(());
    }
    let stale = lock_is_stale(&paths.state_lock);
    if stale && fix {
        fs::remove_file(&paths.state_lock)?;
        issues.push(issue(
            DoctorIssueCode::StaleLock,
            DoctorSeverity::Fixed,
            Some(paths.state_lock.clone()),
            "removed stale state lock".to_string(),
        ));
        *fixed_count += 1;
    } else if stale {
        issues.push(issue(
            DoctorIssueCode::StaleLock,
            DoctorSeverity::Error,
            Some(paths.state_lock.clone()),
            "state lock appears stale".to_string(),
        ));
    } else {
        issues.push(issue(
            DoctorIssueCode::ActiveLock,
            DoctorSeverity::Warning,
            Some(paths.state_lock.clone()),
            "state lock belongs to a running process".to_string(),
        ));
    }
    Ok(())
}

fn check_temp_files(
    paths: &AppPaths,
    fix: bool,
    issues: &mut Vec<DoctorIssue>,
    fixed_count: &mut usize,
) -> Result<()> {
    for dir in [&paths.data_root, &paths.registry_dir] {
        if !dir.exists() {
            continue;
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = Utf8PathBuf::from_path_buf(entry.path()).map_err(|path| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("path is not UTF-8: {}", path.display()),
                )
            })?;
            if !is_leftover_temp_file(&path) {
                continue;
            }
            if fix {
                fs::remove_file(&path)?;
                issues.push(issue(
                    DoctorIssueCode::StrandedTempFile,
                    DoctorSeverity::Fixed,
                    Some(path),
                    "removed leftover temp file".to_string(),
                ));
                *fixed_count += 1;
            } else {
                issues.push(issue(
                    DoctorIssueCode::StrandedTempFile,
                    DoctorSeverity::Warning,
                    Some(path),
                    "leftover temp file found".to_string(),
                ));
            }
        }
    }
    Ok(())
}

fn is_leftover_temp_file(path: &Utf8Path) -> bool {
    path.file_name()
        .map(|name| name.ends_with(".tmp"))
        .unwrap_or(false)
}

fn has_missing_managed_source(
    skills: Option<&crate::core::registry::SkillsRegistry>,
    deployment: &DeploymentRecord,
) -> bool {
    skills
        .map(|skills| {
            !skills
                .skills
                .iter()
                .any(|skill| skill.id == deployment.skill_id)
        })
        .unwrap_or(false)
}

fn lock_is_stale(path: &Utf8Path) -> bool {
    let Ok(contents) = fs::read_to_string(path) else {
        return true;
    };
    let Some(pid) = contents
        .lines()
        .find_map(|line| line.strip_prefix("pid="))
        .and_then(|value| value.trim().parse::<u32>().ok())
    else {
        return true;
    };
    !process_exists(pid)
}

#[cfg(unix)]
fn process_exists(pid: u32) -> bool {
    std::path::Path::new("/proc").join(pid.to_string()).exists()
        || std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
}

#[cfg(not(unix))]
fn process_exists(_pid: u32) -> bool {
    false
}

fn report(issues: Vec<DoctorIssue>, fixed_count: usize) -> DoctorReport {
    let ok = !issues
        .iter()
        .any(|issue| issue.severity == DoctorSeverity::Error);
    DoctorReport {
        ok,
        fixed_count,
        issues,
    }
}

fn issue(
    code: DoctorIssueCode,
    severity: DoctorSeverity,
    path: Option<Utf8PathBuf>,
    message: String,
) -> DoctorIssue {
    DoctorIssue {
        code,
        severity,
        path,
        message,
    }
}
