use crate::core::agents::AgentKind;
use crate::gui::state::{GuiModel, InspectorSection, RenderRow, RenderableView};

pub fn view_name() -> &'static str {
    "Agents"
}

pub fn renderable(model: &GuiModel) -> RenderableView {
    let mut main_rows = model
        .agents
        .iter()
        .map(|agent| RenderRow {
            id: agent.id.to_string(),
            cells: vec![
                agent.label.clone(),
                directories_label(
                    &agent
                        .project_skill_dirs
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>(),
                ),
                enabled_label(agent.enabled),
                validation_label(agent),
            ],
        })
        .collect::<Vec<_>>();

    if !model
        .agents
        .iter()
        .any(|agent| matches!(agent.kind, AgentKind::Custom))
    {
        main_rows.push(RenderRow {
            id: "custom-agents".to_string(),
            cells: vec![
                "Custom Agents".to_string(),
                "-".to_string(),
                "Not configured".to_string(),
                "Ready".to_string(),
            ],
        });
    }

    RenderableView {
        view: model.active_view,
        title: view_name().to_string(),
        columns: vec![
            "Agent".to_string(),
            "Project skill directories".to_string(),
            "Enabled".to_string(),
            "Validation".to_string(),
        ],
        main_rows,
        inspector_sections: inspector_sections(model),
        empty_message: model.agents.is_empty().then_some(
            "No Agents configured. Add a custom Agent or restore the built-in defaults.",
        ),
    }
}

fn inspector_sections(model: &GuiModel) -> Vec<InspectorSection> {
    if let Some(draft) = model.agent_editor_draft() {
        return vec![
            InspectorSection {
                title: "Editing".to_string(),
                lines: vec![
                    draft.label_text.clone(),
                    format!("Agent id {}", draft.id_text),
                    format!("Project dir {}", draft.project_dir_text),
                ],
            },
            InspectorSection {
                title: "Actions".to_string(),
                lines: vec![
                    "Save writes the Agent project directory to local Skill-kits config."
                        .to_string(),
                    "Project directories must be relative paths.".to_string(),
                ],
            },
        ];
    }

    let agent = model.selected_agent().or_else(|| model.agents.first());
    match agent {
        Some(agent) => vec![
            InspectorSection {
                title: "Summary".to_string(),
                lines: vec![
                    agent.label.clone(),
                    format!("Kind {:?}", agent.kind),
                    enabled_label(agent.enabled),
                ],
            },
            InspectorSection {
                title: "Project Directories".to_string(),
                lines: agent
                    .project_skill_dirs
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            },
            InspectorSection {
                title: "Actions".to_string(),
                lines: vec![
                    "Edit path updates this Agent project Skill directory.".to_string(),
                    "Add custom creates a local config entry for another Agent.".to_string(),
                    "No global Agent sync settings in v0.1.".to_string(),
                ],
            },
        ],
        None => vec![InspectorSection {
            title: "Empty".to_string(),
            lines: vec!["Configure an Agent project skill directory.".to_string()],
        }],
    }
}

fn directories_label(dirs: &[String]) -> String {
    if dirs.is_empty() {
        "-".to_string()
    } else {
        dirs.join(", ")
    }
}

fn enabled_label(enabled: bool) -> String {
    if enabled {
        "Enabled".to_string()
    } else {
        "Disabled".to_string()
    }
}

fn validation_label(agent: &crate::core::agents::AgentConfig) -> String {
    if agent.project_skill_dirs.iter().any(|dir| dir.is_absolute()) {
        "Invalid absolute project directory".to_string()
    } else {
        "Ready".to_string()
    }
}
