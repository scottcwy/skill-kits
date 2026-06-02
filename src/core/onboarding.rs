use crate::core::{
    agents::project_skill_dirs_for,
    config::{update_config, RecentProject},
    error::Result,
    paths::AppPaths,
    project::{now_string, toggle_state},
    registry::{read_deployments_registry, ToggleState},
};
use camino::{Utf8Path, Utf8PathBuf};

#[derive(Clone, Debug)]
pub struct ProjectOnboardingScanRequest<'a> {
    pub app_paths: &'a AppPaths,
    pub project_path: &'a Utf8Path,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectOnboardingScanReport {
    pub project_path: Utf8PathBuf,
    pub discovered: Vec<DiscoveredProjectSkill>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredProjectSkill {
    pub agent_id: crate::core::ids::AgentId,
    pub name: String,
    pub path: Utf8PathBuf,
    pub toggle: ToggleState,
}

pub fn project_onboarding_scan(
    request: ProjectOnboardingScanRequest<'_>,
) -> Result<ProjectOnboardingScanReport> {
    let project_path = request.project_path.to_path_buf();
    record_recent_project(request.app_paths, &project_path)?;

    let config = crate::core::config::read_config(request.app_paths)?;
    let deployments = read_deployments_registry(request.app_paths)?;
    let mut discovered = Vec::new();

    for agent in config.agents.into_iter().filter(|agent| agent.enabled) {
        let project_skill_dirs = if agent.project_skill_dirs.is_empty() {
            project_skill_dirs_for(&agent.id).unwrap_or_default()
        } else {
            agent.project_skill_dirs
        };
        for project_skill_dir in project_skill_dirs {
            let skill_root = project_path.join(project_skill_dir);
            if !skill_root.is_dir() {
                continue;
            }
            for entry in std::fs::read_dir(&skill_root)? {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    continue;
                }
                let skill_path = Utf8PathBuf::from_path_buf(entry.path()).map_err(|path| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("path is not UTF-8: {}", path.display()),
                    )
                })?;
                if deployments.deployments.iter().any(|deployment| {
                    deployment.project_path == project_path
                        && deployment.agent_id == agent.id
                        && deployment.deployment_path == skill_path
                }) {
                    continue;
                }
                let toggle = toggle_state(&skill_path);
                if matches!(
                    toggle,
                    ToggleState::InvalidBothMissing | ToggleState::InvalidBothPresent
                ) {
                    continue;
                }
                let name = skill_path
                    .file_name()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| skill_path.to_string());
                discovered.push(DiscoveredProjectSkill {
                    agent_id: agent.id.clone(),
                    name,
                    path: skill_path,
                    toggle,
                });
            }
        }
    }

    discovered.sort_by(|left, right| {
        left.agent_id
            .as_str()
            .cmp(right.agent_id.as_str())
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.path.cmp(&right.path))
    });

    Ok(ProjectOnboardingScanReport {
        project_path,
        discovered,
    })
}

pub(crate) fn record_recent_project(app_paths: &AppPaths, project_path: &Utf8Path) -> Result<()> {
    let project_name = project_path
        .file_name()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| project_path.to_string());
    let recent = RecentProject {
        name: project_name,
        path: project_path.to_path_buf(),
        last_opened_at: now_string(),
    };
    update_config(app_paths, |config| {
        config
            .recent_projects
            .retain(|project| project.path != recent.path);
        config.recent_projects.insert(0, recent);
        Ok(())
    })
}
