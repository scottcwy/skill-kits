use camino::Utf8PathBuf;
use skill_kits::core::{
    agents::{AgentConfig, AgentKind},
    config::{read_config, write_config, Config, RecentProject},
    hash::hash_skill_dir,
    ids::{AgentId, SkillId},
    paths::{ensure_app_dirs, AppPaths},
    project::{deploy_project_skill, ProjectDeployRequest},
    registry::{
        read_deployments_registry, read_skills_registry, write_deployments_registry,
        write_skills_registry, DeploymentRecord, DeploymentsRegistry, ManagedSkill, SkillMetadata,
        SkillSource, SkillsRegistry,
    },
};
use skill_kits::gui::state::{
    GuiActionIntent, GuiController, GuiModel, GuiScope, NavigationView, ProjectSummary,
    DRIFT_REMOVE_CONFIRMATION_MESSAGE,
};
use skill_kits::gui::{agent_actions, project_actions, AgentAction, ProjectAction};
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

fn managed_skill_with_metadata(paths: &AppPaths) -> ManagedSkill {
    let mut skill = managed_skill(paths);
    skill.content_hash = "metadata-hash".to_string();
    skill.updated_at = "2026-05-31T12:34:56Z".to_string();
    skill.metadata = Some(SkillMetadata {
        title: Some("Frontend Design Systems".to_string()),
        description: Some(
            "Builds polished interface systems from existing product context.".to_string(),
        ),
        frontmatter: toml::value::Table::new(),
    });
    skill
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

fn write_config_with_codex_project(paths: &AppPaths, project: &camino::Utf8Path) {
    write_config(
        paths,
        &Config {
            recent_projects: vec![RecentProject {
                name: "sample-app".to_string(),
                path: project.to_path_buf(),
                last_opened_at: "2026-05-31T00:00:00Z".to_string(),
            }],
            ..Config::default()
        },
    )
    .unwrap();
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
fn selecting_render_rows_updates_view_selection_without_filesystem_mutation() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();

    let frontend = managed_skill(&paths);
    write_skill(&frontend.managed_path, "# Frontend Design\n");
    let second = managed_skill_with_name(&paths, "reviewer");
    write_skill(&second.managed_path, "# Reviewer\n");
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![frontend, second.clone()],
        },
    )
    .unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    write_config_with_codex_project(&paths, &project);
    deploy_project_skill(ProjectDeployRequest {
        app_paths: &paths,
        project_path: &project,
        agent_id: &AgentId::new("codex"),
        skill_query: "frontend-design",
    })
    .unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Skills);
    assert!(model.select_render_row(second.id.as_str()));
    assert_eq!(
        model.request_scan_selected_skill(),
        Some(GuiActionIntent::ScanSkill {
            skill_id: second.id.clone(),
        })
    );

    model.navigate(NavigationView::Agents);
    assert!(model.select_render_row("codex"));
    assert_eq!(model.selected_agent().unwrap().id, AgentId::new("codex"));
    assert!(!model.select_render_row("custom-agents"));

    model.navigate(NavigationView::Projects);
    let deployment_id = model.deployments[0].id.clone();
    assert!(model.select_render_row(&deployment_id));
    assert_eq!(model.selected_deployment().unwrap().id, deployment_id);
    assert_eq!(
        model.request_disable_selected_deployment(),
        Some(GuiActionIntent::DisableDeployment {
            project_path: project,
            agent_id: AgentId::new("codex"),
            skill_name: "frontend-design".to_string(),
        })
    );
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
fn projects_onboarding_renders_adopt_all_for_discovered_unmanaged_summary() {
    let project = Utf8PathBuf::from("/tmp/sample-app");
    let mut model = GuiModel::default();
    model.navigate(NavigationView::Projects);
    model.select_project(project.clone());
    model.project_summaries.push(ProjectSummary {
        name: "sample-app".to_string(),
        path: project.clone(),
        deployment_count: 0,
        discovered_unmanaged_count: 2,
        last_adopt_all_result: None,
    });

    let renderable = model.renderable_view();
    let onboarding = renderable
        .inspector_sections
        .iter()
        .find(|section| section.title == "Onboarding")
        .expect("missing Onboarding inspector section");

    assert_eq!(
        onboarding.lines,
        vec![
            "2 discovered project Skill(s) are available to adopt.".to_string(),
            "Adopt all emits a GUI intent; no Skill is adopted automatically.".to_string(),
        ]
    );
    assert_eq!(project_actions(&model), vec![ProjectAction::AdoptAll]);
    assert_eq!(
        model.request_adopt_all_discovered_for_selected_project(),
        Some(GuiActionIntent::ProjectAdoptAll {
            project_path: project,
        })
    );
}

#[test]
fn selecting_project_clears_stale_deployment_selection_for_onboarding() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    let onboarding_project = project_path(&temp_dir, "onboarding-app");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&onboarding_project).unwrap();
    ensure_app_dirs(&paths).unwrap();

    let skill = managed_skill(&paths);
    write_skill(&skill.managed_path, "# Frontend Design\n");
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![skill],
        },
    )
    .unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    write_config(
        &paths,
        &Config {
            recent_projects: vec![
                RecentProject {
                    name: "sample-app".to_string(),
                    path: project.clone(),
                    last_opened_at: "2026-05-31T00:00:00Z".to_string(),
                },
                RecentProject {
                    name: "onboarding-app".to_string(),
                    path: onboarding_project.clone(),
                    last_opened_at: "2026-05-31T00:00:00Z".to_string(),
                },
            ],
            ..Config::default()
        },
    )
    .unwrap();
    deploy_project_skill(ProjectDeployRequest {
        app_paths: &paths,
        project_path: &project,
        agent_id: &AgentId::new("codex"),
        skill_query: "frontend-design",
    })
    .unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Projects);
    let deployment_id = model.deployments[0].id.clone();
    model.select_deployment(deployment_id);
    model
        .project_summaries
        .iter_mut()
        .find(|summary| summary.path == onboarding_project)
        .unwrap()
        .discovered_unmanaged_count = 2;

    model.select_scope(GuiScope::Project(onboarding_project.clone()));

    assert_eq!(model.selected_deployment_status(), None);
    assert_eq!(project_actions(&model), vec![ProjectAction::AdoptAll]);
    let renderable = model.renderable_view();
    let onboarding = renderable
        .inspector_sections
        .iter()
        .find(|section| section.title == "Onboarding")
        .expect("missing Onboarding inspector section");
    assert_eq!(
        onboarding.lines,
        vec![
            "2 discovered project Skill(s) are available to adopt.".to_string(),
            "Adopt all emits a GUI intent; no Skill is adopted automatically.".to_string(),
        ]
    );
}

#[test]
fn refresh_project_intent_runs_onboarding_scan_and_updates_discovered_count() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    write_skill(&project.join(".agents/skills/unmanaged"), "# Unmanaged\n");

    let mut model = GuiModel::load(&paths).unwrap();
    model.select_project(project.clone());
    model.request_refresh_selected_project().unwrap();

    let controller = GuiController::new(paths.clone());
    assert_eq!(
        model.execute_next_intent(&controller).unwrap(),
        Some(GuiActionIntent::RefreshProject {
            project_path: project.clone(),
        })
    );

    let summary = model
        .project_summaries
        .iter()
        .find(|summary| summary.path == project)
        .expect("refresh should add a project summary");
    assert_eq!(summary.discovered_unmanaged_count, 1);
    assert_eq!(project_actions(&model), vec![ProjectAction::AdoptAll]);
    assert!(read_config(&paths)
        .unwrap()
        .recent_projects
        .iter()
        .any(|recent| recent.path == project));
    assert!(read_skills_registry(&paths).unwrap().skills.is_empty());
    assert!(read_deployments_registry(&paths)
        .unwrap()
        .deployments
        .is_empty());
    assert!(project.join(".agents/skills/unmanaged/SKILL.md").exists());
}

#[test]
fn project_adopt_all_intent_executes_for_discovered_project_skills() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    write_skill(&project.join(".agents/skills/unmanaged"), "# Unmanaged\n");

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Projects);
    model.select_project(project.clone());
    model.project_summaries.push(ProjectSummary {
        name: "sample-app".to_string(),
        path: project.clone(),
        deployment_count: 0,
        discovered_unmanaged_count: 1,
        last_adopt_all_result: None,
    });
    model
        .request_adopt_all_discovered_for_selected_project()
        .unwrap();

    let controller = GuiController::new(paths.clone());
    assert_eq!(
        model.execute_next_intent(&controller).unwrap(),
        Some(GuiActionIntent::ProjectAdoptAll {
            project_path: project.clone(),
        })
    );

    assert_eq!(model.skills.len(), 1);
    assert_eq!(model.skills[0].name, "unmanaged");
    assert_eq!(model.deployments.len(), 1);
    assert_eq!(model.deployments[0].skill_name, "unmanaged");
    assert_eq!(
        model
            .project_summaries
            .iter()
            .find(|summary| summary.path == project)
            .unwrap()
            .discovered_unmanaged_count,
        0
    );
    let onboarding = model
        .renderable_view()
        .inspector_sections
        .into_iter()
        .find(|section| section.title == "Onboarding")
        .expect("missing Onboarding inspector section");
    assert!(onboarding
        .lines
        .contains(&"1 adopted, 0 conflicts".to_string()));
    assert!(project.join(".agents/skills/unmanaged/SKILL.md").exists());
}

#[test]
fn project_adopt_all_intent_runs_for_multiple_enabled_agents() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![
                AgentConfig {
                    id: AgentId::new("codex"),
                    label: "Codex".to_string(),
                    kind: AgentKind::BuiltIn,
                    global_skill_dirs: Vec::new(),
                    project_skill_dirs: vec![".agents/skills".into()],
                    enabled: true,
                },
                AgentConfig {
                    id: AgentId::new("custom"),
                    label: "Custom".to_string(),
                    kind: AgentKind::Custom,
                    global_skill_dirs: Vec::new(),
                    project_skill_dirs: vec![".custom/skills".into()],
                    enabled: true,
                },
            ],
            ..Config::default()
        },
    )
    .unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    write_skill(&project.join(".agents/skills/codex-skill"), "# Codex\n");
    write_skill(&project.join(".custom/skills/custom-skill"), "# Custom\n");

    let mut model = GuiModel::load(&paths).unwrap();
    model.select_project(project.clone());
    model.project_summaries.push(ProjectSummary {
        name: "sample-app".to_string(),
        path: project.clone(),
        deployment_count: 0,
        discovered_unmanaged_count: 2,
        last_adopt_all_result: None,
    });
    model
        .request_adopt_all_discovered_for_selected_project()
        .unwrap();

    let controller = GuiController::new(paths);
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert_eq!(model.skills.len(), 2);
    assert!(model.skills.iter().any(|skill| skill.name == "codex-skill"));
    assert!(model
        .skills
        .iter()
        .any(|skill| skill.name == "custom-skill"));
    assert_eq!(model.deployments.len(), 2);
    assert!(model
        .deployments
        .iter()
        .any(|deployment| deployment.agent_id == AgentId::new("codex")));
    assert!(model
        .deployments
        .iter()
        .any(|deployment| deployment.agent_id == AgentId::new("custom")));
    let summary = model
        .project_summaries
        .iter()
        .find(|summary| summary.path == project)
        .unwrap();
    assert_eq!(summary.discovered_unmanaged_count, 0);
    assert_eq!(summary.last_adopt_all_result.as_ref().unwrap().imported, 2);
}

#[test]
fn project_adopt_all_keeps_conflicting_project_skills_discovered() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    let mut managed = managed_skill_with_name(&paths, "conflict");
    write_skill(&managed.managed_path, "# Managed Conflict\n");
    managed.content_hash = hash_skill_dir(&managed.managed_path).unwrap();
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![managed],
        },
    )
    .unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    write_skill(
        &project.join(".agents/skills/conflict"),
        "# Project Conflict\n",
    );

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Projects);
    model.select_project(project.clone());
    model.project_summaries.push(ProjectSummary {
        name: "sample-app".to_string(),
        path: project.clone(),
        deployment_count: 0,
        discovered_unmanaged_count: 1,
        last_adopt_all_result: None,
    });
    model
        .request_adopt_all_discovered_for_selected_project()
        .unwrap();

    let controller = GuiController::new(paths);
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    let summary = model
        .project_summaries
        .iter()
        .find(|summary| summary.path == project)
        .unwrap();
    assert_eq!(summary.discovered_unmanaged_count, 1);
    assert_eq!(summary.last_adopt_all_result.as_ref().unwrap().imported, 0);
    assert_eq!(summary.last_adopt_all_result.as_ref().unwrap().conflicts, 1);
    assert_eq!(model.skills.len(), 1);
    assert!(model.deployments.is_empty());
    assert_eq!(project_actions(&model), vec![ProjectAction::AdoptAll]);
    let onboarding = model
        .renderable_view()
        .inspector_sections
        .into_iter()
        .find(|section| section.title == "Onboarding")
        .expect("missing Onboarding inspector section");
    assert!(onboarding
        .lines
        .contains(&"0 adopted, 1 conflicts".to_string()));
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
fn agent_actions_emit_intents_without_direct_filesystem_mutation() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
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
            ..Config::default()
        },
    )
    .unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let config_before = std::fs::read_to_string(&paths.config_file).unwrap();
    let mut model = GuiModel::load(&paths).unwrap();
    model.select_agent(AgentId::new("custom"));

    let edit_intent = model.request_edit_selected_agent().unwrap();
    assert_eq!(
        edit_intent,
        GuiActionIntent::EditAgent {
            agent_id: AgentId::new("custom"),
        }
    );
    let add_intent = model.request_add_custom_agent().unwrap();
    assert_eq!(add_intent, GuiActionIntent::AddCustomAgent);

    let controller = GuiController::new(paths.clone());
    controller.execute(&edit_intent).unwrap();
    controller.execute(&add_intent).unwrap();
    assert_eq!(
        std::fs::read_to_string(&paths.config_file).unwrap(),
        config_before
    );
}

#[test]
fn agents_controls_offer_edit_selected_and_add_custom_actions() {
    let mut model = GuiModel::default();
    assert_eq!(agent_actions(&model), vec![AgentAction::AddCustom]);

    model.agents.push(AgentConfig {
        id: AgentId::new("codex"),
        label: "Codex".to_string(),
        kind: AgentKind::BuiltIn,
        global_skill_dirs: Vec::new(),
        project_skill_dirs: vec![".agents/skills".into()],
        enabled: true,
    });
    model.select_agent(AgentId::new("codex"));

    assert_eq!(
        agent_actions(&model),
        vec![AgentAction::EditSelected, AgentAction::AddCustom]
    );
}

#[test]
fn redeploy_actions_emit_selected_deployment_intents_without_direct_filesystem_mutation() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();

    let skill = managed_skill(&paths);
    write_skill(&skill.managed_path, "# Frontend Design\n");
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![skill],
        },
    )
    .unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    write_config_with_codex_project(&paths, &project);

    deploy_project_skill(ProjectDeployRequest {
        app_paths: &paths,
        project_path: &project,
        agent_id: &AgentId::new("codex"),
        skill_query: "frontend-design",
    })
    .unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.select_deployment(model.deployments[0].id.clone());

    assert_eq!(
        model.request_redeploy_selected_deployment(),
        Some(GuiActionIntent::RedeployDeployment {
            project_path: project.clone(),
            agent_id: AgentId::new("codex"),
            skill_name: "frontend-design".to_string(),
            overwrite: false,
            promote: false,
        })
    );
    assert_eq!(
        model.request_overwrite_selected_deployment(),
        Some(GuiActionIntent::RedeployDeployment {
            project_path: project.clone(),
            agent_id: AgentId::new("codex"),
            skill_name: "frontend-design".to_string(),
            overwrite: true,
            promote: false,
        })
    );
    assert_eq!(
        model.request_promote_selected_deployment(),
        Some(GuiActionIntent::RedeployDeployment {
            project_path: project,
            agent_id: AgentId::new("codex"),
            skill_name: "frontend-design".to_string(),
            overwrite: false,
            promote: true,
        })
    );
    assert_eq!(model.pending_intents().len(), 3);
}

#[test]
fn drifted_remove_first_click_records_confirmation_without_queueing_remove() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();

    let skill = managed_skill(&paths);
    write_skill(&skill.managed_path, "# Frontend Design\n");
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![skill],
        },
    )
    .unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    write_config_with_codex_project(&paths, &project);

    deploy_project_skill(ProjectDeployRequest {
        app_paths: &paths,
        project_path: &project,
        agent_id: &AgentId::new("codex"),
        skill_query: "frontend-design",
    })
    .unwrap();
    std::fs::write(
        project.join(".agents/skills/frontend-design/local.txt"),
        "project edit\n",
    )
    .unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    let deployment_id = model.deployments[0].id.clone();
    model.select_deployment(deployment_id.clone());

    assert_eq!(model.request_remove_selected_deployment(false), None);
    assert_eq!(model.pending_intents(), &[]);
    assert_eq!(
        model.pending_remove_confirmation(),
        Some(deployment_id.as_str())
    );
    assert_eq!(
        model.pending_remove_confirmation_message(),
        Some(DRIFT_REMOVE_CONFIRMATION_MESSAGE)
    );
}

#[test]
fn confirm_drifted_remove_queues_force_remove_intent() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();

    let skill = managed_skill(&paths);
    write_skill(&skill.managed_path, "# Frontend Design\n");
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![skill],
        },
    )
    .unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    write_config_with_codex_project(&paths, &project);

    deploy_project_skill(ProjectDeployRequest {
        app_paths: &paths,
        project_path: &project,
        agent_id: &AgentId::new("codex"),
        skill_query: "frontend-design",
    })
    .unwrap();
    std::fs::write(
        project.join(".agents/skills/frontend-design/local.txt"),
        "project edit\n",
    )
    .unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    let deployment_id = model.deployments[0].id.clone();
    model.select_deployment(deployment_id.clone());
    assert_eq!(model.request_remove_selected_deployment(false), None);

    assert_eq!(
        model.confirm_pending_remove(),
        Some(GuiActionIntent::RemoveDeployment {
            project_path: project,
            agent_id: AgentId::new("codex"),
            skill_name: "frontend-design".to_string(),
            force: true,
        })
    );
    assert_eq!(model.pending_remove_confirmation(), None);
    assert_eq!(model.pending_intents().len(), 1);
}

#[test]
fn controller_executes_project_action_intents_and_reloads_model_state() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config_with_codex_project(&paths, &project);

    let mut skill = managed_skill(&paths);
    write_skill(&skill.managed_path, "# Frontend Design\n");
    skill.content_hash = hash_skill_dir(&skill.managed_path).unwrap();
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![skill],
        },
    )
    .unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.select_scope(GuiScope::Project(project.clone()));
    model.select_skill(SkillId::new("frontend-design-a1b2c3d4"));
    model
        .request_deploy_selected_skill(AgentId::new("codex"))
        .unwrap();

    let controller = GuiController::new(paths.clone());
    assert!(model.execute_next_intent(&controller).unwrap().is_some());
    assert_eq!(model.pending_intents().len(), 0);
    assert_eq!(model.deployments.len(), 1);
    assert!(project
        .join(".agents/skills/frontend-design/SKILL.md")
        .exists());

    model.select_deployment(model.deployments[0].id.clone());
    model.request_disable_selected_deployment().unwrap();
    assert!(model.execute_next_intent(&controller).unwrap().is_some());
    assert!(project
        .join(".agents/skills/frontend-design/SKILL.md.disabled")
        .exists());
    assert_eq!(
        model.selected_deployment_status().unwrap().toggle,
        skill_kits::core::registry::ToggleState::Disabled
    );

    model.request_enable_selected_deployment().unwrap();
    assert!(model.execute_next_intent(&controller).unwrap().is_some());
    assert!(project
        .join(".agents/skills/frontend-design/SKILL.md")
        .exists());

    model.request_remove_selected_deployment(false).unwrap();
    assert!(model.execute_next_intent(&controller).unwrap().is_some());
    assert_eq!(model.deployments.len(), 0);
    assert!(!project.join(".agents/skills/frontend-design").exists());
}

#[test]
fn controller_executes_redeploy_intent_and_reloads_model_state() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config_with_codex_project(&paths, &project);

    let mut skill = managed_skill(&paths);
    write_skill(&skill.managed_path, "# Frontend Design\n");
    skill.content_hash = hash_skill_dir(&skill.managed_path).unwrap();
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![skill],
        },
    )
    .unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    deploy_project_skill(ProjectDeployRequest {
        app_paths: &paths,
        project_path: &project,
        agent_id: &AgentId::new("codex"),
        skill_query: "frontend-design",
    })
    .unwrap();
    std::fs::write(
        paths.skills_dir.join("frontend-design-a1b2c3d4/SKILL.md"),
        "# Frontend Design\n\nUpdated upstream\n",
    )
    .unwrap();
    let mut updated_skill = managed_skill(&paths);
    updated_skill.content_hash = hash_skill_dir(&updated_skill.managed_path).unwrap();
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![updated_skill],
        },
    )
    .unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.select_deployment(model.deployments[0].id.clone());
    model.request_redeploy_selected_deployment().unwrap();

    let controller = GuiController::new(paths);
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert_eq!(model.pending_intents().len(), 0);
    assert_eq!(model.deployments.len(), 1);
    assert!(
        std::fs::read_to_string(project.join(".agents/skills/frontend-design/SKILL.md"))
            .unwrap()
            .contains("Updated upstream")
    );
    assert!(!model.selected_deployment_status().unwrap().outdated);
}

#[test]
fn controller_executes_uninstall_intent_and_reloads_global_inventory() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();

    let skill = managed_skill(&paths);
    write_skill(&skill.managed_path, "# Frontend Design\n");
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![skill.clone()],
        },
    )
    .unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.select_skill(skill.id.clone());
    model.request_uninstall_selected_skill().unwrap();

    let controller = GuiController::new(paths.clone());
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert_eq!(model.pending_intents().len(), 0);
    assert!(model.skills.is_empty());
    assert!(!skill.managed_path.exists());
}

#[test]
fn skills_inspector_renders_metadata_and_registry_metadata_from_loaded_model() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();

    let skill = managed_skill_with_metadata(&paths);
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![skill.clone()],
        },
    )
    .unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Skills);
    model.select_skill(skill.id.clone());

    let renderable = model.renderable_view();
    let metadata = renderable
        .inspector_sections
        .iter()
        .find(|section| section.title == "Metadata")
        .expect("missing Metadata inspector section");
    assert_eq!(
        metadata.lines,
        vec![
            "Title Frontend Design Systems".to_string(),
            "Description Builds polished interface systems from existing product context."
                .to_string(),
        ]
    );

    let registry_metadata = renderable
        .inspector_sections
        .iter()
        .find(|section| section.title == "Registry Metadata")
        .expect("missing Registry Metadata inspector section");
    assert_eq!(
        registry_metadata.lines,
        vec![
            "ID frontend-design-a1b2c3d4".to_string(),
            "Hash metadata-hash".to_string(),
            "Updated 2026-05-31T12:34:56Z".to_string(),
        ]
    );
}

#[test]
fn skills_inspector_renders_project_deployments_from_loaded_model() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config_with_codex_project(&paths, &project);

    let mut skill = managed_skill(&paths);
    write_skill(&skill.managed_path, "# Frontend Design\n");
    skill.content_hash = hash_skill_dir(&skill.managed_path).unwrap();
    let deployed_path = project.join(".agents/skills/frontend-design");
    write_skill(&deployed_path, "# Frontend Design\n");
    let mut record = deployment(&project);
    record.baseline_hash = hash_skill_dir(&deployed_path).unwrap();
    record.deployed_from_hash = skill.content_hash.clone();
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![skill.clone()],
        },
    )
    .unwrap();
    write_deployments_registry(
        &paths,
        &DeploymentsRegistry {
            version: 1,
            deployments: vec![record],
        },
    )
    .unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Skills);
    model.select_skill(skill.id.clone());

    let renderable = model.renderable_view();
    let deployments = renderable
        .inspector_sections
        .iter()
        .find(|section| section.title == "Project Deployments")
        .expect("missing Project Deployments inspector section");

    assert_eq!(
        deployments.lines,
        vec![format!(
            "sample-app | codex | Enabled | {}",
            project.join(".agents/skills/frontend-design")
        )]
    );
}

#[test]
fn gui_view_modules_do_not_use_std_fs_directly() {
    let view_files = [
        "src/gui/dashboard.rs",
        "src/gui/skills.rs",
        "src/gui/agents.rs",
        "src/gui/projects.rs",
    ];
    for file in view_files {
        let contents = std::fs::read_to_string(file).unwrap();
        assert!(
            !contents.contains("std::fs") && !contents.contains("use std::fs"),
            "{file} should not use std::fs directly"
        );
    }
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

#[test]
fn projects_inspector_limits_actions_for_missing_managed_source_deployments() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();

    let skill = managed_skill(&paths);
    write_skill(&skill.managed_path, "# Frontend Design\n");
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![skill],
        },
    )
    .unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    write_config_with_codex_project(&paths, &project);

    deploy_project_skill(ProjectDeployRequest {
        app_paths: &paths,
        project_path: &project,
        agent_id: &AgentId::new("codex"),
        skill_query: "frontend-design",
    })
    .unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Projects);
    let deployment_id = model.deployments[0].id.clone();
    model.select_deployment(deployment_id);

    let renderable = model.renderable_view();
    let actions = renderable
        .inspector_sections
        .iter()
        .find(|section| section.title == "Actions")
        .expect("missing Actions inspector section");

    assert_eq!(
        actions.lines,
        vec!["Available actions: Promote to managed, Remove from project.".to_string()]
    );
}

#[test]
fn projects_controls_limit_actions_for_missing_managed_source_deployments() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();

    let skill = managed_skill(&paths);
    write_skill(&skill.managed_path, "# Frontend Design\n");
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![skill],
        },
    )
    .unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    write_config_with_codex_project(&paths, &project);

    deploy_project_skill(ProjectDeployRequest {
        app_paths: &paths,
        project_path: &project,
        agent_id: &AgentId::new("codex"),
        skill_query: "frontend-design",
    })
    .unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Projects);
    model.select_deployment(model.deployments[0].id.clone());

    assert_eq!(
        project_actions(&model),
        vec![ProjectAction::Promote, ProjectAction::Remove]
    );
}

#[test]
fn projects_controls_keep_normal_deployment_actions() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();

    let skill = managed_skill(&paths);
    write_skill(&skill.managed_path, "# Frontend Design\n");
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![skill],
        },
    )
    .unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    write_config_with_codex_project(&paths, &project);

    deploy_project_skill(ProjectDeployRequest {
        app_paths: &paths,
        project_path: &project,
        agent_id: &AgentId::new("codex"),
        skill_query: "frontend-design",
    })
    .unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Projects);
    model.select_deployment(model.deployments[0].id.clone());

    assert_eq!(
        project_actions(&model),
        vec![
            ProjectAction::Deploy,
            ProjectAction::Enable,
            ProjectAction::Disable,
            ProjectAction::Redeploy,
            ProjectAction::Overwrite,
            ProjectAction::Promote,
            ProjectAction::Remove,
        ]
    );
}
