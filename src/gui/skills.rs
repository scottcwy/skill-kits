use crate::core::{
    agent_space::{SkillInstance, SkillInstanceSourceKind},
    registry::{DeploymentStatus, ToggleState},
};
use crate::gui::state::{
    skill_instance_scope_label, skill_instance_source_label, skill_instance_status_label, GuiModel,
    InspectorSection, RenderRow, RenderableView,
};

pub fn view_name() -> &'static str {
    "Skills"
}

pub fn renderable(model: &GuiModel) -> RenderableView {
    let main_rows = model
        .skill_instances
        .iter()
        .filter(|instance| matches_agent_filter(model, instance))
        .filter(|instance| matches_scope_filter(model, instance))
        .filter(|instance| matches_status_filter(model, instance))
        .map(|instance| RenderRow {
            id: instance.id.clone(),
            cells: vec![
                instance.name.clone(),
                agent_label(model, instance),
                skill_instance_scope_label(&instance.scope),
                skill_instance_status_label(instance).to_string(),
                skill_instance_source_label(model, instance),
                if instance.managed {
                    "Managed".to_string()
                } else {
                    "Unmanaged".to_string()
                },
                instance
                    .updated_at
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
            ],
        })
        .collect();

    RenderableView {
        view: model.active_view,
        title: view_name().to_string(),
        columns: vec![
            "Skill".to_string(),
            "Agent".to_string(),
            "Scope".to_string(),
            "Status".to_string(),
            "Source".to_string(),
            "Managed".to_string(),
            "Updated".to_string(),
        ],
        main_rows,
        inspector_sections: inspector_sections(model),
        empty_message: model
            .skill_instances
            .is_empty()
            .then_some("No Agent Space Skills found. Scan enabled Agent directories."),
    }
}

fn inspector_sections(model: &GuiModel) -> Vec<InspectorSection> {
    let Some(instance) = model
        .selected_skill_instance()
        .or_else(|| model.skill_instances.first())
    else {
        return vec![InspectorSection {
            title: "Empty".to_string(),
            lines: vec![
                "No Agent Space Skills found.".to_string(),
                "Scan enabled Agent directories or install a local managed source.".to_string(),
            ],
        }];
    };

    let mut sections = vec![
        InspectorSection {
            title: "Summary".to_string(),
            lines: vec![
                instance.name.clone(),
                format!("Instance ID {}", instance.id),
                format!("Agent {}", agent_label(model, instance)),
                format!("Scope {}", skill_instance_scope_label(&instance.scope)),
                format!("Status {}", skill_instance_status_label(instance)),
                format!("Source {}", skill_instance_source_label(model, instance)),
                format!("Managed {}", if instance.managed { "Yes" } else { "No" }),
                format!("Writable {}", if instance.writable { "Yes" } else { "No" }),
            ],
        },
        InspectorSection {
            title: "Paths".to_string(),
            lines: vec![
                format!("Skill dir {}", instance.skill_dir),
                format!("Enabled {}", instance.enabled_path),
                format!("Disabled {}", instance.disabled_path),
            ],
        },
        InspectorSection {
            title: "Metadata".to_string(),
            lines: metadata_lines(instance),
        },
        InspectorSection {
            title: "Registry Metadata".to_string(),
            lines: registry_metadata_lines(instance),
        },
        InspectorSection {
            title: "State".to_string(),
            lines: state_lines(instance),
        },
        InspectorSection {
            title: "Risk Findings".to_string(),
            lines: risk_lines(model, instance),
        },
        InspectorSection {
            title: "Project Deployments".to_string(),
            lines: project_deployment_lines(model, instance),
        },
        InspectorSection {
            title: "Actions".to_string(),
            lines: vec![
                "Scan Agent Spaces refreshes the Agent-visible Skill read model.".to_string(),
                "Install local copies a local Skill into Managed Inventory.".to_string(),
                "Import managed copy copies this Agent Space Skill into Managed Inventory."
                    .to_string(),
                "Deploy to Project copies a managed Skill into a project Agent Space.".to_string(),
            ],
        },
    ];

    sections.retain(|section| !section.lines.is_empty());
    sections
}

fn metadata_lines(instance: &SkillInstance) -> Vec<String> {
    let Some(metadata) = &instance.metadata else {
        return Vec::new();
    };

    let mut lines = Vec::new();
    if let Some(title) = &metadata.title {
        lines.push(format!("Title {title}"));
    }
    if let Some(description) = &metadata.description {
        lines.push(format!("Description {description}"));
    }
    lines
}

fn registry_metadata_lines(instance: &SkillInstance) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(stable_id) = &instance.stable_id {
        lines.push(format!("Stable ID {stable_id}"));
    }
    if let Some(content_hash) = &instance.content_hash {
        lines.push(format!("Hash {content_hash}"));
    }
    if let Some(updated_at) = &instance.updated_at {
        lines.push(format!("Updated {updated_at}"));
    }
    lines
}

fn state_lines(instance: &SkillInstance) -> Vec<String> {
    let mut lines = Vec::new();
    match instance.toggle_state {
        ToggleState::InvalidBothPresent => {
            lines.push("Invalid: both SKILL.md and SKILL.md.disabled are present.".to_string());
        }
        ToggleState::InvalidBothMissing => {
            lines.push("Missing: neither SKILL.md nor SKILL.md.disabled is present.".to_string());
        }
        ToggleState::Enabled | ToggleState::Disabled => {
            if !instance.writable
                && matches!(
                    instance.source_kind,
                    SkillInstanceSourceKind::PluginCache | SkillInstanceSourceKind::Vendor
                )
            {
                lines.push(
                    "Read-only: plugin/cache/vendor sources cannot be toggled here.".to_string(),
                );
            } else if !instance.writable {
                lines.push("Read-only: this Skill directory is not writable.".to_string());
            }
        }
    }
    lines
}

fn risk_lines(model: &GuiModel, instance: &SkillInstance) -> Vec<String> {
    let Some(stable_id) = &instance.stable_id else {
        return Vec::new();
    };
    let Some(report) = model.skill_risk_report(stable_id) else {
        return Vec::new();
    };

    let mut lines = vec![format!("{}.", report.summary_label())];
    lines.extend(report.findings.iter().map(|finding| {
        format!(
            "{} line {} - {}",
            finding.rule_id,
            finding
                .line
                .map(|line| line.to_string())
                .unwrap_or_else(|| "-".to_string()),
            finding.message
        )
    }));
    lines
}

fn project_deployment_lines(model: &GuiModel, instance: &SkillInstance) -> Vec<String> {
    let Some(stable_id) = &instance.stable_id else {
        return Vec::new();
    };
    model
        .deployment_statuses
        .iter()
        .filter(|status| status.record.skill_id == *stable_id)
        .map(|status| {
            format!(
                "{} | {} | {} | {}",
                status.record.project_name,
                status.record.agent_id,
                deployment_status_label(status),
                status.record.deployment_path
            )
        })
        .collect()
}

fn deployment_status_label(status: &DeploymentStatus) -> String {
    if status.missing_managed_source {
        return "Missing managed source".to_string();
    }
    if status.outdated {
        return "Outdated".to_string();
    }
    if status.drift {
        return "Drift".to_string();
    }

    match status.toggle {
        ToggleState::Enabled => "Enabled".to_string(),
        ToggleState::Disabled => "Disabled".to_string(),
        ToggleState::InvalidBothPresent | ToggleState::InvalidBothMissing => "Invalid".to_string(),
    }
}

fn agent_label(model: &GuiModel, instance: &SkillInstance) -> String {
    model
        .agents
        .iter()
        .find(|agent| agent.id == instance.agent_id)
        .map(|agent| agent.label.clone())
        .unwrap_or_else(|| instance.agent_id.to_string())
}

fn matches_agent_filter(model: &GuiModel, instance: &SkillInstance) -> bool {
    model
        .skill_agent_filter()
        .map_or(true, |agent_id| instance.agent_id == *agent_id)
}

fn matches_scope_filter(model: &GuiModel, instance: &SkillInstance) -> bool {
    model.skill_scope_filter().map_or(true, |scope| {
        skill_instance_scope_label(&instance.scope) == scope
    })
}

fn matches_status_filter(model: &GuiModel, instance: &SkillInstance) -> bool {
    model.skill_status_filter().map_or(true, |status| {
        skill_instance_status_label(instance) == status
    })
}
