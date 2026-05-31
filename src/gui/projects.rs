use crate::core::registry::{DeploymentStatus, ToggleState};
use crate::gui::state::{GuiModel, InspectorSection, RenderRow, RenderableView};

pub fn view_name() -> &'static str {
    "Projects"
}

pub fn renderable(model: &GuiModel) -> RenderableView {
    let selected_project = model.scope_project_path();
    let main_rows = model
        .deployment_statuses
        .iter()
        .filter(|status| {
            selected_project
                .as_ref()
                .map(|path| status.record.project_path == *path)
                .unwrap_or(true)
        })
        .map(|status| RenderRow {
            id: status.record.id.clone(),
            cells: vec![
                status.record.skill_name.clone(),
                status.record.agent_id.to_string(),
                toggle_label(&status.toggle),
                outdated_label(status),
                drift_label(status),
                missing_source_label(status),
                "Not scanned".to_string(),
                status.record.deployment_path.to_string(),
            ],
        })
        .collect();

    RenderableView {
        view: model.active_view,
        title: view_name().to_string(),
        columns: vec![
            "Skill".to_string(),
            "Agent".to_string(),
            "Toggle".to_string(),
            "Outdated".to_string(),
            "Drift".to_string(),
            "Missing managed source".to_string(),
            "Risk".to_string(),
            "Path".to_string(),
        ],
        main_rows,
        inspector_sections: inspector_sections(model),
    }
}

fn inspector_sections(model: &GuiModel) -> Vec<InspectorSection> {
    if let Some(status) = model.selected_deployment_status() {
        let deployment = &status.record;
        return vec![
            InspectorSection {
                title: "Deployment".to_string(),
                lines: vec![
                    deployment.skill_name.clone(),
                    format!("Agent {}", deployment.agent_id),
                    format!("Project {}", deployment.project_name),
                ],
            },
            InspectorSection {
                title: "Path".to_string(),
                lines: vec![deployment.deployment_path.to_string()],
            },
            InspectorSection {
                title: "Actions".to_string(),
                lines: action_lines(status),
            },
        ];
    }

    if let Some(project) = model.selected_project_summary() {
        return vec![
            InspectorSection {
                title: "Project".to_string(),
                lines: vec![project.name.clone(), project.path.to_string()],
            },
            InspectorSection {
                title: "Onboarding".to_string(),
                lines: onboarding_lines(project),
            },
            InspectorSection {
                title: "Git Ignore Guidance".to_string(),
                lines: vec!["Guidance only. Skill-kits does not edit .gitignore.".to_string()],
            },
        ];
    }

    vec![InspectorSection {
        title: "Empty".to_string(),
        lines: vec!["Open a project to scan project-level Skills.".to_string()],
    }]
}

fn onboarding_lines(project: &crate::gui::state::ProjectSummary) -> Vec<String> {
    let mut lines = project
        .last_adopt_all_result
        .as_ref()
        .map(|result| {
            vec![format!(
                "{} adopted, {} conflicts",
                result.imported, result.conflicts
            )]
        })
        .unwrap_or_default();

    if project.discovered_unmanaged_count > 0 {
        lines.extend([
            format!(
                "{} discovered project Skill(s) are available to adopt.",
                project.discovered_unmanaged_count
            ),
            "Adopt all emits a GUI intent; no Skill is adopted automatically.".to_string(),
        ]);
        return lines;
    }

    lines.extend([
        "No startup project scan has run.".to_string(),
        "Refresh emits an explicit project scan intent.".to_string(),
    ]);
    lines
}

fn action_lines(status: &DeploymentStatus) -> Vec<String> {
    if status.missing_managed_source {
        return vec!["Available actions: Promote to managed, Remove from project.".to_string()];
    }

    vec![
        "Enable, disable, redeploy, promote, and remove emit intents.".to_string(),
        "This project copy has local changes. Removing it deletes only this deployed Skill, not the Agent skill root.".to_string(),
    ]
}

fn toggle_label(toggle: &ToggleState) -> String {
    match toggle {
        ToggleState::Enabled => "Enabled".to_string(),
        ToggleState::Disabled => "Disabled".to_string(),
        ToggleState::InvalidBothPresent | ToggleState::InvalidBothMissing => "Invalid".to_string(),
    }
}

fn outdated_label(status: &DeploymentStatus) -> String {
    if status.outdated {
        "Outdated".to_string()
    } else {
        "No".to_string()
    }
}

fn drift_label(status: &DeploymentStatus) -> String {
    if status.drift {
        "Drift".to_string()
    } else {
        "No".to_string()
    }
}

fn missing_source_label(status: &DeploymentStatus) -> String {
    if status.missing_managed_source {
        "Missing managed source".to_string()
    } else {
        "No".to_string()
    }
}
