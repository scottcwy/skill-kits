use crate::gui::state::{GuiModel, InspectorSection, RenderRow, RenderableView};

pub fn view_name() -> &'static str {
    "Dashboard"
}

pub fn renderable(model: &GuiModel) -> RenderableView {
    let summary = &model.dashboard;
    let main_rows = vec![
        row(
            "agent-space",
            "Agent Space Skills",
            summary.agent_space_instance_count,
        ),
        row("managed", "Managed Skills", summary.managed_skill_count),
        RenderRow {
            id: "agents".to_string(),
            cells: vec![
                "Agents".to_string(),
                format!(
                    "{}/{} enabled",
                    summary.enabled_agent_count, summary.agent_count
                ),
            ],
        },
        row("projects", "Recent Projects", summary.recent_project_count),
        row(
            "deployments",
            "Project Deployments",
            summary.deployment_count,
        ),
    ];
    let project_lines = if model.project_summaries.is_empty() {
        vec!["No Recent Projects".to_string()]
    } else {
        model
            .project_summaries
            .iter()
            .map(|project| {
                format!(
                    "{} - {} deployment(s)",
                    project.name, project.deployment_count
                )
            })
            .collect()
    };

    RenderableView {
        view: model.active_view,
        title: view_name().to_string(),
        columns: vec!["Metric".to_string(), "Value".to_string()],
        main_rows,
        inspector_sections: vec![
            InspectorSection {
                title: "Scope".to_string(),
                lines: vec![
                    format!(
                        "Agent Space instances {}",
                        summary.agent_space_instance_count
                    ),
                    format!("Managed Inventory copies {}", summary.managed_skill_count),
                ],
            },
            InspectorSection {
                title: "Recent Projects".to_string(),
                lines: project_lines,
            },
            InspectorSection {
                title: "Health".to_string(),
                lines: vec![
                    format!("Registry {}", health_label(&summary.registry_health)),
                    format!("Lock {}", health_label(&summary.lock_health)),
                    format!("Cache {}", health_label(&summary.cache_health)),
                    format!("Risk findings {}", summary.risk_count),
                    format!("Outdated deployments {}", summary.outdated_deployment_count),
                    format!("Drifted deployments {}", summary.drifted_deployment_count),
                    format!("Invalid toggles {}", summary.invalid_toggle_count),
                    format!(
                        "Missing managed sources {}",
                        summary.missing_managed_source_count
                    ),
                ],
            },
        ],
        empty_message: None,
    }
}

fn health_label(health: &crate::core::status::HealthState) -> &'static str {
    match health {
        crate::core::status::HealthState::Ok => "Ok",
        crate::core::status::HealthState::Warning => "Warning",
        crate::core::status::HealthState::Error => "Error",
    }
}

fn row(id: &str, label: &str, value: usize) -> RenderRow {
    RenderRow {
        id: id.to_string(),
        cells: vec![label.to_string(), value.to_string()],
    }
}
