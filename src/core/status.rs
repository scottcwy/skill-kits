use crate::core::{
    config::read_config,
    doctor::{run_doctor, DoctorSeverity},
    paths::AppPaths,
    registry::{read_deployments_registry, read_skills_registry},
    scan::scan_skill_dir,
    Result,
};
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GlobalStatus {
    pub managed_skill_count: usize,
    pub agent_count: usize,
    pub enabled_agent_count: usize,
    pub agent_config_state: AgentConfigState,
    pub recent_project_count: usize,
    pub registry_health: HealthState,
    pub lock_health: HealthState,
    pub cache_health: HealthState,
    pub risk_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentConfigState {
    Configured,
    Partial,
    Empty,
    Invalid,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    Ok,
    Warning,
    Error,
}

pub fn global_status(paths: &AppPaths) -> Result<GlobalStatus> {
    let config = read_config(paths)?;
    let skills = read_skills_registry(paths)?;
    let deployments = read_deployments_registry(paths)?;
    let doctor = run_doctor(paths, false)?;
    let registry_health = if doctor
        .issues
        .iter()
        .any(|issue| issue.severity == DoctorSeverity::Error)
    {
        HealthState::Error
    } else if doctor
        .issues
        .iter()
        .any(|issue| issue.severity == DoctorSeverity::Warning)
    {
        HealthState::Warning
    } else {
        HealthState::Ok
    };
    let lock_health = if doctor
        .issues
        .iter()
        .any(|issue| matches!(issue.code, crate::core::doctor::DoctorIssueCode::StaleLock))
    {
        HealthState::Error
    } else if doctor
        .issues
        .iter()
        .any(|issue| matches!(issue.code, crate::core::doctor::DoctorIssueCode::ActiveLock))
    {
        HealthState::Warning
    } else {
        HealthState::Ok
    };
    let cache_health = if paths.cache_dir.exists() {
        HealthState::Ok
    } else {
        HealthState::Warning
    };
    let risk_count = skills
        .skills
        .iter()
        .filter(|skill| skill.managed_path.exists())
        .try_fold(0usize, |count, skill| {
            Ok::<usize, crate::core::SkillKitsError>(
                count + scan_skill_dir(&skill.managed_path)?.len(),
            )
        })?;

    Ok(GlobalStatus {
        managed_skill_count: skills.skills.len(),
        agent_count: config.agents.len(),
        enabled_agent_count: config.agents.iter().filter(|agent| agent.enabled).count(),
        agent_config_state: agent_config_state(&config.agents),
        recent_project_count: config.recent_projects.len(),
        registry_health,
        lock_health,
        cache_health,
        risk_count: risk_count + deployment_risk_count(&deployments),
    })
}

fn agent_config_state(agents: &[crate::core::agents::AgentConfig]) -> AgentConfigState {
    if agents.is_empty() {
        AgentConfigState::Empty
    } else if agents
        .iter()
        .any(|agent| agent.project_skill_dirs.iter().any(|dir| dir.is_absolute()))
    {
        AgentConfigState::Invalid
    } else if agents.iter().all(|agent| agent.enabled) {
        AgentConfigState::Configured
    } else {
        AgentConfigState::Partial
    }
}

fn deployment_risk_count(deployments: &crate::core::registry::DeploymentsRegistry) -> usize {
    deployments
        .deployments
        .iter()
        .filter(|deployment| deployment.baseline_hash.is_empty())
        .count()
}
