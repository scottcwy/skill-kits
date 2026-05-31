use camino::Utf8PathBuf;
use skill_kits::core::{
    agents::{AgentConfig, AgentKind},
    config::{write_config, Config, RecentProject},
    hash::hash_skill_dir,
    ids::{AgentId, SkillId},
    paths::{ensure_app_dirs, AppPaths},
    project::{deploy_project_skill, ProjectDeployRequest},
    registry::{
        write_deployments_registry, write_skills_registry, DeploymentRecord, DeploymentsRegistry,
        ManagedSkill, SkillSource, SkillsRegistry,
    },
};
use skill_kits::gui::state::{GuiActionIntent, GuiModel, GuiScope, NavigationView};
use tempfile::TempDir;

fn test_paths(temp_dir: &TempDir) -> AppPaths {
    AppPaths::from_data_root(
        Utf8PathBuf::from_path_buf(temp_dir.path().join(".skill-kits")).unwrap(),
    )
}

fn project_path(temp_dir: &TempDir, name: &str) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(temp_dir.path().join(name)).unwrap()
}

fn managed_skill(paths: &AppPaths) -> ManagedSkill {
    ManagedSkill {
        id: SkillId::new("frontend-design-a1b2c3d4"),
        name: "frontend-design".to_string(),
        source: SkillSource::Local {
            source_path: "/tmp/source/frontend-design".into(),
        },
        managed_path: paths.skills_dir.join("frontend-design-a1b2c3d4"),
        content_hash: "managed-hash".to_string(),
        metadata: None,
        created_at: "2026-05-31T00:00:00Z".to_string(),
        updated_at: "2026-05-31T00:00:00Z".to_string(),
    }
}

fn managed_skill_with_name(paths: &AppPaths, name: &str) -> ManagedSkill {
    let id = SkillId::new(format!("{name}-a1b2c3d4"));
    ManagedSkill {
        id,
        name: name.to_string(),
        source: SkillSource::Local {
            source_path: Utf8PathBuf::from(format!("/tmp/source/{name}")),
        },
        managed_path: paths.skills_dir.join(format!("{name}-a1b2c3d4")),
        content_hash: String::new(),
        metadata: None,
        created_at: "2026-05-31T00:00:00Z".to_string(),
        updated_at: "2026-05-31T00:00:00Z".to_string(),
    }
}

fn deployment(project: &camino::Utf8Path) -> DeploymentRecord {
    DeploymentRecord {
        id: "codex-frontend-design-a1b2c3d4-project".to_string(),
        skill_id: SkillId::new("frontend-design-a1b2c3d4"),
        agent_id: AgentId::new("codex"),
        project_name: "sample-app".to_string(),
        project_path: project.to_path_buf(),
        deployment_path: project.join(".agents/skills/frontend-design"),
        skill_name: "frontend-design".to_string(),
        baseline_hash: "baseline".to_string(),
        deployed_from_hash: "managed-hash".to_string(),
        created_at: "2026-05-31T00:00:00Z".to_string(),
        updated_at: "2026-05-31T00:00:00Z".to_string(),
    }
}

fn write_skill(path: &camino::Utf8Path, body: &str) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::write(path.join("SKILL.md"), body).unwrap();
}

#[test]
fn each_navigation_view_loads_from_app_paths_model() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();

    let skill = managed_skill(&paths);
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![skill],
        },
    )
    .unwrap();
    write_deployments_registry(
        &paths,
        &DeploymentsRegistry {
            version: 1,
            deployments: vec![deployment(&project)],
        },
    )
    .unwrap();
    write_config(
        &paths,
        &Config {
            recent_projects: vec![RecentProject {
                name: "sample-app".to_string(),
                path: project.clone(),
                last_opened_at: "2026-05-31T00:00:00Z".to_string(),
            }],
            ..Config::default()
        },
    )
    .unwrap();

    let mut model = GuiModel::load(&paths).unwrap();

    for view in NavigationView::ORDER {
        model.navigate(view);
        let renderable = model.renderable_view();
        assert_eq!(renderable.view, view);
        assert!(!renderable.title.is_empty());
        assert!(renderable.main_rows.len() <= 4);
        assert!(!renderable.inspector_sections.is_empty());
    }
}

#[test]
fn startup_loads_registry_and_recent_project_summaries_without_recursive_project_scan() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    let unmanaged_skill = project.join(".agents/skills/unmanaged/SKILL.md");
    std::fs::create_dir_all(unmanaged_skill.parent().unwrap()).unwrap();
    std::fs::write(&unmanaged_skill, "# Unmanaged\n").unwrap();
    ensure_app_dirs(&paths).unwrap();

    write_config(
        &paths,
        &Config {
            recent_projects: vec![RecentProject {
                name: "sample-app".to_string(),
                path: project.clone(),
                last_opened_at: "2026-05-31T00:00:00Z".to_string(),
            }],
            ..Config::default()
        },
    )
    .unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let model = GuiModel::load(&paths).unwrap();

    assert_eq!(model.dashboard.managed_skill_count, 0);
    assert_eq!(model.recent_projects.len(), 1);
    assert!(model
        .project_summaries
        .iter()
        .all(|summary| summary.deployment_count == 0));
    assert!(!model
        .project_summaries
        .iter()
        .any(|summary| summary.discovered_unmanaged_count > 0));
}

#[test]
fn actions_emit_intents_without_direct_filesystem_mutation() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();

    let custom_agent = AgentConfig {
        id: AgentId::new("custom"),
        label: "Custom".to_string(),
        kind: AgentKind::Custom,
        global_skill_dirs: Vec::new(),
        project_skill_dirs: vec![".custom/skills".into()],
        enabled: true,
    };
    write_config(
        &paths,
        &Config {
            agents: vec![custom_agent],
            recent_projects: vec![RecentProject {
                name: "sample-app".to_string(),
                path: project.clone(),
                last_opened_at: "2026-05-31T00:00:00Z".to_string(),
            }],
            ..Config::default()
        },
    )
    .unwrap();
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![managed_skill(&paths)],
        },
    )
    .unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.select_scope(GuiScope::Project(project.clone()));
    model.select_skill(SkillId::new("frontend-design-a1b2c3d4"));

    assert_eq!(
        model.request_scan_selected_skill(),
        Some(GuiActionIntent::ScanSkill {
            skill_id: SkillId::new("frontend-design-a1b2c3d4")
        })
    );
    assert_eq!(
        model.request_deploy_selected_skill(AgentId::new("custom")),
        Some(GuiActionIntent::DeploySkill {
            project_path: project.clone(),
            agent_id: AgentId::new("custom"),
            skill_id: SkillId::new("frontend-design-a1b2c3d4"),
        })
    );
    assert_eq!(model.pending_intents().len(), 2);
    assert!(!project.join(".custom/skills/frontend-design").exists());
}

#[test]
fn projects_render_rows_show_cached_core_deployment_statuses() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();

    let skill_names = [
        "enabled-skill",
        "disabled-skill",
        "outdated-skill",
        "drifted-skill",
        "missing-source-skill",
        "invalid-toggle-skill",
    ];
    let skills = skill_names
        .iter()
        .map(|name| {
            let mut skill = managed_skill_with_name(&paths, name);
            write_skill(&skill.managed_path, &format!("# {name}\n"));
            skill.content_hash = hash_skill_dir(&skill.managed_path).unwrap();
            skill
        })
        .collect::<Vec<_>>();
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: skills.clone(),
        },
    )
    .unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    write_config(
        &paths,
        &Config {
            recent_projects: vec![RecentProject {
                name: "sample-app".to_string(),
                path: project.clone(),
                last_opened_at: "2026-05-31T00:00:00Z".to_string(),
            }],
            ..Config::default()
        },
    )
    .unwrap();

    for skill in &skills {
        deploy_project_skill(ProjectDeployRequest {
            app_paths: &paths,
            project_path: &project,
            agent_id: &AgentId::new("codex"),
            skill_query: &skill.name,
        })
        .unwrap();
    }

    let disabled_dir = project.join(".agents/skills/disabled-skill");
    std::fs::rename(
        disabled_dir.join("SKILL.md"),
        disabled_dir.join("SKILL.md.disabled"),
    )
    .unwrap();
    std::fs::write(
        paths.skills_dir.join("outdated-skill-a1b2c3d4/SKILL.md"),
        "# Outdated changed\n",
    )
    .unwrap();
    let mut updated_skills = skills.clone();
    let outdated = updated_skills
        .iter_mut()
        .find(|skill| skill.name == "outdated-skill")
        .unwrap();
    outdated.content_hash = hash_skill_dir(&outdated.managed_path).unwrap();
    std::fs::write(
        project.join(".agents/skills/drifted-skill/local.txt"),
        "project edit\n",
    )
    .unwrap();
    updated_skills.retain(|skill| skill.name != "missing-source-skill");
    let invalid_dir = project.join(".agents/skills/invalid-toggle-skill");
    std::fs::write(invalid_dir.join("SKILL.md.disabled"), "# Disabled too\n").unwrap();
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: updated_skills,
        },
    )
    .unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Projects);
    model.select_scope(GuiScope::Project(project));
    let renderable = model.renderable_view();

    let row_cells = |skill_name: &str| -> Vec<String> {
        renderable
            .main_rows
            .iter()
            .find(|row| row.cells.first().is_some_and(|cell| cell == skill_name))
            .unwrap_or_else(|| panic!("missing Projects row for {skill_name}"))
            .cells
            .clone()
    };

    assert_eq!(row_cells("enabled-skill")[2], "Enabled");
    assert_eq!(row_cells("enabled-skill")[3], "No");
    assert_eq!(row_cells("enabled-skill")[4], "No");
    assert_eq!(row_cells("enabled-skill")[5], "No");
    assert_eq!(row_cells("disabled-skill")[2], "Disabled");
    assert_eq!(row_cells("outdated-skill")[3], "Outdated");
    assert_eq!(row_cells("drifted-skill")[4], "Drift");
    assert_eq!(
        row_cells("missing-source-skill")[5],
        "Missing managed source"
    );
    assert_eq!(row_cells("invalid-toggle-skill")[2], "Invalid");
}
