use crate::gui::state::{GuiModel, InspectorSection, RenderRow, RenderableView};

pub fn view_name() -> &'static str {
    "Dashboard"
}

pub fn renderable(model: &GuiModel) -> RenderableView {
    let summary = &model.dashboard;
    let main_rows = vec![
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
                lines: vec!["Global Inventory".to_string()],
            },
            InspectorSection {
                title: "Recent Projects".to_string(),
                lines: project_lines,
            },
            InspectorSection {
                title: "Health".to_string(),
                lines: vec![
                    "Registry summaries loaded".to_string(),
                    "Project scans run only on request".to_string(),
                ],
            },
        ],
    }
}

fn row(id: &str, label: &str, value: usize) -> RenderRow {
    RenderRow {
        id: id.to_string(),
        cells: vec![label.to_string(), value.to_string()],
    }
}
