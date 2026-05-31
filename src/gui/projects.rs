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
    if let Some(status) = model
        .selected_deployment_status()
        .or_else(|| model.deployment_statuses.first())
    {
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
                lines: vec![
                    "Enable, disable, redeploy, promote, and remove emit intents.".to_string(),
                    "This project copy has local changes. Removing it deletes only this deployed Skill, not the Agent skill root.".to_string(),
                ],
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
                lines: vec![
                    "No startup project scan has run.".to_string(),
                    "Refresh emits an explicit project scan intent.".to_string(),
                ],
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
