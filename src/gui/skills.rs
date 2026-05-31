use crate::gui::state::{
    skill_source_label, GuiModel, InspectorSection, RenderRow, RenderableView,
};

pub fn view_name() -> &'static str {
    "Skills"
}

pub fn renderable(model: &GuiModel) -> RenderableView {
    let main_rows = model
        .skills
        .iter()
        .map(|skill| {
            let deployment_count = model
                .deployments
                .iter()
                .filter(|deployment| deployment.skill_id == skill.id)
                .count();
            RenderRow {
                id: skill.id.to_string(),
                cells: vec![
                    skill.name.clone(),
                    skill_source_label(&skill.source),
                    "Not scanned".to_string(),
                    deployment_count.to_string(),
                    skill.updated_at.clone(),
                ],
            }
        })
        .collect();

    RenderableView {
        view: model.active_view,
        title: view_name().to_string(),
        columns: vec![
            "Skill".to_string(),
            "Source".to_string(),
            "Risk".to_string(),
            "Project deployments".to_string(),
            "Updated".to_string(),
        ],
        main_rows,
        inspector_sections: inspector_sections(model),
    }
}

fn inspector_sections(model: &GuiModel) -> Vec<InspectorSection> {
    let Some(skill) = model.selected_skill().or_else(|| model.skills.first()) else {
        return vec![InspectorSection {
            title: "Empty".to_string(),
            lines: vec!["Install a local Skill or adopt existing Agent Skills.".to_string()],
        }];
    };
    let source = match &skill.source {
        crate::core::SkillSource::Local { source_path }
        | crate::core::SkillSource::GlobalAgentAdopt { source_path, .. }
        | crate::core::SkillSource::ProjectAdopt { source_path, .. } => source_path.to_string(),
        crate::core::SkillSource::PromotedFromProject { project_path, .. } => {
            project_path.to_string()
        }
    };

    vec![
        InspectorSection {
            title: "Summary".to_string(),
            lines: vec![skill.name.clone(), format!("ID {}", skill.id)],
        },
        InspectorSection {
            title: "Paths".to_string(),
            lines: vec![
                format!("Managed {}", skill.managed_path),
                format!("Source {source}"),
            ],
        },
        InspectorSection {
            title: "Actions".to_string(),
            lines: vec![
                "Scan emits an intent for core execution.".to_string(),
                "Uninstall removes this Skill from Global Inventory. Project copies are not deleted."
                    .to_string(),
            ],
        },
    ]
}
