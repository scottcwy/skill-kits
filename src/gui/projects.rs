use crate::core::registry::{DeploymentStatus, ToggleState};
use crate::gui::state::{GuiModel, InspectorSection, RenderRow, RenderableView};

pub fn view_name() -> &'static str {
    "Project"
}

pub fn renderable(model: &GuiModel) -> RenderableView {
    let selected_project = model.scope_project_path();
    let mut main_rows: Vec<_> = model
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
                risk_label(model, status),
                status.record.deployment_path.to_string(),
            ],
        })
        .collect();
    if let Some(project) = model.selected_project_summary() {
        main_rows.extend(project.discovered_skills.iter().map(|skill| RenderRow {
            id: format!("discovered:{}:{}", skill.agent_id, skill.name),
            cells: vec![
                skill.name.clone(),
                skill.agent_id.to_string(),
                "Discovered".to_string(),
                toggle_label(&skill.toggle),
                "-".to_string(),
                "-".to_string(),
                "Not managed".to_string(),
                skill.path.to_string(),
            ],
        }));
    }
    let empty_message = if main_rows.is_empty() {
        Some(
            "No project deployments in this scope. Refresh a project, adopt existing Skills, or deploy a managed Skill.",
        )
    } else {
        None
    };

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
        empty_message,
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
        if let Some(skill) = model.selected_discovered_project_skill() {
            return vec![
                InspectorSection {
                    title: "Discovered Skill".to_string(),
                    lines: vec![
                        skill.name.clone(),
                        format!("Agent {}", skill.agent_id),
                        format!("Toggle {}", toggle_label(&skill.toggle)),
                        skill.path.to_string(),
                    ],
                },
                InspectorSection {
                    title: "Actions".to_string(),
                    lines: vec![
                        "Adopt selected imports only this project Skill.".to_string(),
                        "Adopt all imports all non-conflicting discovered Skills.".to_string(),
                    ],
                },
            ];
        }
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
                title: "Deploy Target".to_string(),
                lines: deploy_target_lines(model),
            },
            InspectorSection {
                title: "Git Ignore Guidance".to_string(),
                lines: vec!["Guidance only. Skill-kits does not edit .gitignore.".to_string()],
            },
        ];
    }

    vec![InspectorSection {
        title: "Empty".to_string(),
        lines: vec![
            "No Recent Project is selected.".to_string(),
            "Open a project from the Scope switcher before scanning or deploying.".to_string(),
        ],
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
            "Select a discovered Skill to adopt it individually, or adopt all non-conflicting Skills."
                .to_string(),
        ]);
        lines.extend(project.discovered_skills.iter().map(|skill| {
            format!(
                "{}/{} - {}",
                skill.agent_id,
                skill.name,
                toggle_label(&skill.toggle)
            )
        }));
        if !project.pending_conflicts.is_empty() {
            lines.push("Conflicts remain: import as new or skip.".to_string());
        }
        return lines;
    }

    if project.onboarding_scanned {
        lines.extend([
            "No unmanaged project Skills were found.".to_string(),
            "Deploy a managed Skill to this project, or add an Agent Skill directory and Refresh."
                .to_string(),
        ]);
    } else {
        lines.extend([
            "Project has not been scanned in this GUI session.".to_string(),
            "Refresh scans this project for existing Agent Skills without adopting automatically."
                .to_string(),
        ]);
    }
    lines
}

fn deploy_target_lines(model: &GuiModel) -> Vec<String> {
    let Some(target) = model.project_deploy_target() else {
        return vec!["Select a managed Skill and enabled Agent to deploy.".to_string()];
    };

    vec![
        format!("Skill {}", target.skill_name),
        format!("Agent {}", target.agent_label),
        format!("Target {}", target.target_path),
    ]
}

fn action_lines(status: &DeploymentStatus) -> Vec<String> {
    if is_invalid_toggle(status) {
        return vec![
            "Invalid toggle state blocks deployment actions.".to_string(),
            "Keep exactly one of SKILL.md or SKILL.md.disabled, then Refresh.".to_string(),
        ];
    }

    if status.missing_managed_source {
        return vec!["Available actions: Promote to managed, Remove from project.".to_string()];
    }

    vec![
        "Enable or disable toggles SKILL.md / SKILL.md.disabled.".to_string(),
        "Redeploy updates from managed; overwrite or promote is required when local drift exists."
            .to_string(),
        "Remove deletes only this deployed Skill, not the Agent skill root.".to_string(),
    ]
}

pub fn is_invalid_toggle(status: &DeploymentStatus) -> bool {
    matches!(
        status.toggle,
        ToggleState::InvalidBothPresent | ToggleState::InvalidBothMissing
    )
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

fn risk_label(model: &GuiModel, status: &DeploymentStatus) -> String {
    model
        .skill_risk_report(&status.record.skill_id)
        .map(crate::gui::state::GuiRiskReport::summary_label)
        .unwrap_or_else(|| "Not scanned".to_string())
}
