use camino::Utf8PathBuf;
use skill_kits::core::status::HealthState;
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
        SkillSource, SkillsRegistry, ToggleState,
    },
};
use skill_kits::gui::state::{
    GuiActionIntent, GuiController, GuiModel, GuiScope, GuiStatusKind, NavigationView,
    ProjectConflict, ProjectDiscoveredSkill, ProjectSummary, DRIFT_REMOVE_CONFIRMATION_MESSAGE,
    GLOBAL_UNINSTALL_CONFIRMATION_MESSAGE,
};
use skill_kits::gui::{
    agent_actions, project_actions, skill_actions, AgentAction, ProjectAction, SkillAction,
    SkillKitsGuiApp,
};
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

fn write_global_codex_skill(temp_dir: &TempDir, name: &str, body: &str) -> Utf8PathBuf {
    let skill_dir =
        Utf8PathBuf::from_path_buf(temp_dir.path().join(".codex/skills").join(name)).unwrap();
    write_skill(&skill_dir, body);
    skill_dir
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

fn discovered_skill(
    project: &camino::Utf8Path,
    agent_id: &str,
    name: &str,
) -> ProjectDiscoveredSkill {
    ProjectDiscoveredSkill {
        agent_id: AgentId::new(agent_id),
        name: name.to_string(),
        path: project.join(".agents/skills").join(name),
        toggle: ToggleState::Enabled,
    }
}

fn section_lines(model: &GuiModel, title: &str) -> Vec<String> {
    model
        .renderable_view()
        .inspector_sections
        .into_iter()
        .find(|section| section.title == title)
        .unwrap_or_else(|| panic!("missing {title} inspector section"))
        .lines
}

#[test]
fn skills_view_renders_agent_space_instances_instead_of_managed_inventory_columns() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![managed_skill(&paths)],
        },
    )
    .unwrap();
    let skill_dir = write_global_codex_skill(&temp_dir, "agent-visible", "# Agent Visible\n");

    let mut model = GuiModel::load_with_home_dir(
        &paths,
        Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap(),
    )
    .unwrap();
    model.navigate(NavigationView::Skills);
    let renderable = model.renderable_view();

    assert_eq!(
        renderable.columns,
        vec!["Skill", "Agent", "Scope", "Status", "Source", "Managed", "Updated"]
    );
    assert_eq!(renderable.main_rows.len(), 1);
    let row = &renderable.main_rows[0];
    assert_eq!(row.cells[0], "Agent Visible");
    assert_eq!(row.cells[1], "Codex");
    assert_eq!(row.cells[2], "Global");
    assert_eq!(row.cells[3], "Enabled");
    assert_eq!(row.cells[4], "Codex global");
    assert_eq!(row.cells[5], "Unmanaged");
    assert!(model.select_render_row(&row.id));
    assert_eq!(
        model.selected_skill_instance().unwrap().skill_dir,
        skill_dir
    );
    assert!(section_lines(&model, "Paths")
        .iter()
        .any(|line| line == &format!("Skill dir {skill_dir}")));
}

#[test]
fn skill_instance_actions_toggle_selected_agent_space_file_only() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    let skill_dir = write_global_codex_skill(&temp_dir, "toggle-me", "# Toggle Me\n");
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(project.join(".agents/skills/toggle-me")).unwrap();
    std::fs::write(
        project.join(".agents/skills/toggle-me/SKILL.md"),
        "# Project copy\n",
    )
    .unwrap();

    let home = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
    let mut model = GuiModel::load_with_home_dir(&paths, home.clone()).unwrap();
    model.navigate(NavigationView::Skills);
    let row_id = model.renderable_view().main_rows[0].id.clone();
    assert!(model.select_render_row(&row_id));
    assert_eq!(
        skill_actions(&model),
        vec![
            SkillAction::InstallLocal,
            SkillAction::AdoptAgentSkills,
            SkillAction::Disable
        ]
    );
    assert_eq!(
        model.request_disable_selected_skill_instance(),
        Some(GuiActionIntent::DisableSkillInstance {
            instance_id: row_id.clone(),
        })
    );

    let controller = GuiController::with_home_dir(paths.clone(), home.clone());
    assert!(model.execute_next_intent(&controller).unwrap().is_some());
    assert!(skill_dir.join("SKILL.md.disabled").exists());
    assert!(project.join(".agents/skills/toggle-me/SKILL.md").exists());
    assert_eq!(model.selected_skill_instance().unwrap().id, row_id);
    assert_eq!(
        model.selected_skill_instance().unwrap().toggle_state,
        ToggleState::Disabled
    );
    assert_eq!(
        skill_actions(&model),
        vec![
            SkillAction::InstallLocal,
            SkillAction::AdoptAgentSkills,
            SkillAction::Enable
        ]
    );
    assert_eq!(
        model.request_enable_selected_skill_instance(),
        Some(GuiActionIntent::EnableSkillInstance {
            instance_id: row_id.clone(),
        })
    );
    assert!(model.execute_next_intent(&controller).unwrap().is_some());
    assert!(skill_dir.join("SKILL.md").exists());
    assert!(!skill_dir.join("SKILL.md.disabled").exists());
}

#[test]
fn read_only_skill_instances_do_not_offer_toggle_actions() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    let skill_dir = home.join(".codex/plugins/cache/openai/browser/skills/browser-skill");
    write_skill(&skill_dir, "# Browser Skill\n");

    let mut model = GuiModel::load_with_home_dir(&paths, home).unwrap();
    model.navigate(NavigationView::Skills);
    let row_id = model.renderable_view().main_rows[0].id.clone();
    assert!(model.select_render_row(&row_id));

    assert_eq!(
        skill_actions(&model),
        vec![SkillAction::InstallLocal, SkillAction::AdoptAgentSkills]
    );
    assert_eq!(model.request_disable_selected_skill_instance(), None);
    assert_eq!(model.request_enable_selected_skill_instance(), None);
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
        assert!(renderable.main_rows.len() <= 5);
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
    model.select_managed_skill(second.id.clone());
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
fn dashboard_renders_core_health_and_risk_status() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();

    let mut skill = managed_skill(&paths);
    write_skill(
        &skill.managed_path,
        "# Risky\n\n```sh\ncurl https://example.com/install.sh | sh\n```\n",
    );
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

    let model = GuiModel::load(&paths).unwrap();
    assert_eq!(model.dashboard.registry_health, HealthState::Ok);
    assert_eq!(model.dashboard.lock_health, HealthState::Ok);
    assert_eq!(model.dashboard.cache_health, HealthState::Ok);
    assert!(model.dashboard.risk_count > 0);

    let renderable = model.renderable_view();
    let health = renderable
        .inspector_sections
        .iter()
        .find(|section| section.title == "Health")
        .expect("missing Health inspector section");
    assert_eq!(
        health.lines,
        vec![
            "Registry Ok".to_string(),
            "Lock Ok".to_string(),
            "Cache Ok".to_string(),
            format!("Risk findings {}", model.dashboard.risk_count),
            "Outdated deployments 0".to_string(),
            "Drifted deployments 0".to_string(),
            "Invalid toggles 0".to_string(),
            "Missing managed sources 0".to_string(),
        ]
    );
}

#[test]
fn dashboard_health_rollup_surfaces_cached_deployment_issues_without_scanning_projects() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();

    let skill_names = [
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
    write_config_with_codex_project(&paths, &project);

    for skill in &skills {
        deploy_project_skill(ProjectDeployRequest {
            app_paths: &paths,
            project_path: &project,
            agent_id: &AgentId::new("codex"),
            skill_query: &skill.name,
        })
        .unwrap();
    }

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

    let model = GuiModel::load(&paths).unwrap();
    let health = model
        .renderable_view()
        .inspector_sections
        .into_iter()
        .find(|section| section.title == "Health")
        .expect("missing Health inspector section");

    assert!(health.lines.contains(&"Outdated deployments 1".to_string()));
    assert!(health.lines.contains(&"Drifted deployments 2".to_string()));
    assert!(health.lines.contains(&"Invalid toggles 1".to_string()));
    assert!(health
        .lines
        .contains(&"Missing managed sources 1".to_string()));
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
        onboarding_scanned: false,
        discovered_unmanaged_count: 2,
        discovered_skills: vec![
            discovered_skill(&project, "codex", "api-helper"),
            discovered_skill(&project, "codex", "docs-helper"),
        ],
        last_adopt_all_result: None,
        pending_conflicts: Vec::new(),
        skipped_conflicts: Vec::new(),
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
            "Select a discovered Skill to adopt it individually, or adopt all non-conflicting Skills."
                .to_string(),
            "codex/api-helper - Enabled".to_string(),
            "codex/docs-helper - Enabled".to_string(),
        ]
    );
    assert_eq!(project_actions(&model), vec![ProjectAction::AdoptAll]);
    let rows = model.renderable_view().main_rows;
    assert!(rows
        .iter()
        .any(|row| row.id == "discovered:codex:api-helper"
            && row.cells[0] == "api-helper"
            && row.cells[2] == "Discovered"
            && row.cells[3] == "Enabled"));
    assert!(model.select_render_row("discovered:codex:api-helper"));
    assert_eq!(
        project_actions(&model),
        vec![ProjectAction::AdoptSelected, ProjectAction::AdoptAll]
    );
    assert_eq!(
        model.request_adopt_selected_discovered_project_skill(),
        Some(GuiActionIntent::ProjectAdoptSelected {
            project_path: project.clone(),
            agent_id: AgentId::new("codex"),
            skill_name: "api-helper".to_string(),
        })
    );
    assert_eq!(
        model.request_adopt_all_discovered_for_selected_project(),
        Some(GuiActionIntent::ProjectAdoptAll {
            project_path: project,
        })
    );
}

#[test]
fn gui_empty_states_are_contextual_and_actionable() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();

    model.navigate(NavigationView::Skills);
    let renderable = model.renderable_view();
    assert!(renderable.main_rows.is_empty());
    assert_eq!(
        renderable.empty_message,
        Some("No Agent Space Skills found. Scan enabled Agent directories.")
    );
    assert_eq!(
        skill_actions(&model),
        vec![SkillAction::InstallLocal, SkillAction::AdoptAgentSkills]
    );
    assert_eq!(
        model.request_adopt_all_agent_skills(),
        Some(GuiActionIntent::AdoptAllAgentSkills)
    );
    assert_eq!(
        section_lines(&model, "Empty"),
        vec![
            "No Agent Space Skills found.".to_string(),
            "Scan enabled Agent directories or install a managed source.".to_string(),
        ]
    );

    model.navigate(NavigationView::Projects);
    let renderable = model.renderable_view();
    assert!(renderable.main_rows.is_empty());
    assert_eq!(
        renderable.empty_message,
        Some("No project deployments in this scope. Refresh a project, adopt existing Skills, or deploy a managed Skill.")
    );
    assert_eq!(
        section_lines(&model, "Empty"),
        vec![
            "No Recent Project is selected.".to_string(),
            "Open a project from the Scope switcher before scanning or deploying.".to_string(),
        ]
    );
}

#[test]
fn gui_adopt_all_agent_skills_imports_from_all_enabled_global_agent_dirs() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = project_path(&temp_dir, "home");
    let codex_global = home.join(".codex/skills");
    let custom_global = home.join("custom-agent/skills");
    let disabled_global = home.join("disabled-agent/skills");
    write_skill(&codex_global.join("codex-one"), "# Codex one\n");
    write_skill(&custom_global.join("custom-one"), "# Custom one\n");
    write_skill(&custom_global.join("conflict"), "# Different source\n");
    write_skill(&disabled_global.join("disabled-one"), "# Disabled one\n");
    std::fs::create_dir_all(custom_global.join("not-a-skill")).unwrap();

    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![
                AgentConfig {
                    id: AgentId::new("codex"),
                    label: "Codex".to_string(),
                    kind: AgentKind::BuiltIn,
                    global_skill_dirs: vec![codex_global.clone()],
                    project_skill_dirs: vec![".agents/skills".into()],
                    enabled: true,
                },
                AgentConfig {
                    id: AgentId::new("custom"),
                    label: "Custom".to_string(),
                    kind: AgentKind::Custom,
                    global_skill_dirs: vec![custom_global.clone()],
                    project_skill_dirs: vec![".custom/skills".into()],
                    enabled: true,
                },
                AgentConfig {
                    id: AgentId::new("disabled"),
                    label: "Disabled".to_string(),
                    kind: AgentKind::Custom,
                    global_skill_dirs: vec![disabled_global.clone()],
                    project_skill_dirs: vec![".disabled/skills".into()],
                    enabled: false,
                },
            ],
            ..Config::default()
        },
    )
    .unwrap();
    let mut existing = managed_skill_with_name(&paths, "conflict");
    write_skill(&existing.managed_path, "# Managed source\n");
    existing.content_hash = hash_skill_dir(&existing.managed_path).unwrap();
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![existing],
        },
    )
    .unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    assert_eq!(
        model.request_adopt_all_agent_skills(),
        Some(GuiActionIntent::AdoptAllAgentSkills)
    );
    let controller = GuiController::with_home_dir(paths.clone(), home);
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    let names: Vec<_> = model
        .skills
        .iter()
        .map(|skill| skill.name.as_str())
        .collect();
    assert_eq!(names, vec!["codex-one", "conflict", "custom-one"]);
    assert!(model.skills.iter().any(|skill| matches!(
        &skill.source,
        SkillSource::GlobalAgentAdopt { agent_id, source_path }
            if agent_id == &AgentId::new("codex") && source_path == &codex_global.join("codex-one")
    )));
    assert!(model.skills.iter().any(|skill| matches!(
        &skill.source,
        SkillSource::GlobalAgentAdopt { agent_id, source_path }
            if agent_id == &AgentId::new("custom") && source_path == &custom_global.join("custom-one")
    )));
    assert!(!model
        .skills
        .iter()
        .any(|skill| skill.name == "disabled-one"));
    assert!(!model.skills.iter().any(|skill| skill.name == "not-a-skill"));
    assert!(codex_global.join("codex-one/SKILL.md").exists());
    assert!(custom_global.join("custom-one/SKILL.md").exists());
    assert_eq!(
        model.last_status().unwrap().message,
        "Adopted Agent Skills into Global Inventory: 2 imported, 1 conflict."
    );
}

#[test]
fn gui_adopt_all_agent_skills_recursively_imports_codex_skill_libraries() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = project_path(&temp_dir, "home");
    write_skill(
        &home.join(".codex/plugins/cache/browser/1/skills/browser-skill"),
        "# Browser skill\n",
    );
    write_skill(
        &home.join(".codex/vendor_imports/skills/skills/.curated/vendor-skill"),
        "# Vendor skill\n",
    );
    write_skill(
        &home.join(".skills-manager/skills/managed-library-skill"),
        "# Managed library skill\n",
    );

    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![AgentConfig {
                id: AgentId::new("codex"),
                label: "Codex".to_string(),
                kind: AgentKind::BuiltIn,
                global_skill_dirs: vec![home.join(".codex/skills")],
                project_skill_dirs: vec![".agents/skills".into()],
                enabled: true,
            }],
            ..Config::default()
        },
    )
    .unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.request_adopt_all_agent_skills().unwrap();
    let controller = GuiController::with_home_dir(paths, home);

    assert!(model.execute_next_intent(&controller).unwrap().is_some());
    assert_eq!(
        model
            .skills
            .iter()
            .map(|skill| skill.name.as_str())
            .collect::<Vec<_>>(),
        vec!["browser-skill", "vendor-skill"]
    );
    assert_eq!(
        model.last_status().unwrap().message,
        "Adopted Agent Skills into Global Inventory: 2 imported, 0 conflicts."
    );
}

#[test]
fn gui_adopt_all_agent_skills_skips_enabled_agents_without_global_dirs() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![AgentConfig {
                id: AgentId::new("custom"),
                label: "Custom".to_string(),
                kind: AgentKind::Custom,
                global_skill_dirs: Vec::new(),
                project_skill_dirs: vec![".custom/skills".into()],
                enabled: true,
            }],
            ..Config::default()
        },
    )
    .unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.request_adopt_all_agent_skills().unwrap();
    let controller = GuiController::with_home_dir(paths, project_path(&temp_dir, "home"));

    assert!(model.execute_next_intent(&controller).unwrap().is_some());
    assert!(model.skills.is_empty());
    assert_eq!(
        model.last_status().unwrap().message,
        "Adopted Agent Skills into Global Inventory: 0 imported, 0 conflicts."
    );
}

#[test]
fn gui_adopt_all_agent_skills_reloads_partial_imports_when_later_agent_fails() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let good_global = project_path(&temp_dir, "codex-global");
    let bad_global = project_path(&temp_dir, "not-a-directory");
    write_skill(&good_global.join("good-one"), "# Good one\n");
    if let Some(parent) = bad_global.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&bad_global, "not a directory").unwrap();

    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![AgentConfig {
                id: AgentId::new("codex"),
                label: "Codex".to_string(),
                kind: AgentKind::BuiltIn,
                global_skill_dirs: vec![good_global.clone(), bad_global],
                project_skill_dirs: vec![".agents/skills".into()],
                enabled: true,
            }],
            ..Config::default()
        },
    )
    .unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.request_adopt_all_agent_skills().unwrap();
    let controller = GuiController::with_home_dir(paths, project_path(&temp_dir, "home"));

    assert!(model.execute_next_intent(&controller).unwrap().is_some());
    assert_eq!(
        model
            .skills
            .iter()
            .map(|skill| skill.name.as_str())
            .collect::<Vec<_>>(),
        vec!["good-one"]
    );
    assert_eq!(model.last_status().unwrap().kind, GuiStatusKind::Error);
    assert_eq!(
        model.last_status().unwrap().message,
        "Adopted Agent Skills into Global Inventory: 1 imported, 0 conflicts, 1 failure."
    );
}

#[test]
fn projects_onboarding_copy_distinguishes_not_scanned_from_no_unmanaged_skills() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config_with_codex_project(&paths, &project);
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Projects);

    assert_eq!(
        section_lines(&model, "Onboarding"),
        vec![
            "Project has not been scanned in this GUI session.".to_string(),
            "Refresh scans this project for existing Agent Skills without adopting automatically."
                .to_string(),
        ]
    );

    model.request_refresh_selected_project().unwrap();
    let controller = GuiController::new(paths);
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert_eq!(
        section_lines(&model, "Onboarding"),
        vec![
            "No unmanaged project Skills were found.".to_string(),
            "Deploy a managed Skill to this project, or add an Agent Skill directory and Refresh."
                .to_string(),
        ]
    );
}

#[test]
fn open_project_draft_records_recent_project_and_selects_project_scope_without_scanning() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "opened-app");
    write_skill(&project.join(".agents/skills/unmanaged"), "# Unmanaged\n");
    let canonical_project =
        Utf8PathBuf::from_path_buf(std::fs::canonicalize(&project).unwrap()).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Projects);
    model.begin_open_project();
    model.update_open_project_path(project.join(".").to_string());

    assert_eq!(
        model.request_save_open_project(),
        Some(GuiActionIntent::OpenProject {
            project_path: canonical_project.clone(),
        })
    );

    let controller = GuiController::new(paths.clone());
    assert_eq!(
        model.execute_next_intent(&controller).unwrap(),
        Some(GuiActionIntent::OpenProject {
            project_path: canonical_project.clone(),
        })
    );

    assert!(
        matches!(model.active_scope, GuiScope::Project(ref path) if path == &canonical_project)
    );
    assert_eq!(model.open_project_draft(), None);
    assert_eq!(
        read_config(&paths).unwrap().recent_projects[0].path,
        canonical_project
    );
    let summary = model
        .project_summaries
        .iter()
        .find(|summary| summary.path == canonical_project)
        .expect("open project should add a recent project summary");
    assert_eq!(summary.name, "opened-app");
    assert!(!summary.onboarding_scanned);
    assert_eq!(summary.discovered_unmanaged_count, 0);
    assert!(read_skills_registry(&paths).unwrap().skills.is_empty());
    assert!(project.join(".agents/skills/unmanaged/SKILL.md").exists());
}

#[test]
fn gui_status_feedback_records_last_success_and_last_error() {
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
    assert_eq!(model.pending_action_status_label(), "Idle");
    model.select_scope(GuiScope::Project(project.clone()));
    model.select_skill(SkillId::new("frontend-design-a1b2c3d4"));
    model
        .request_deploy_selected_skill(AgentId::new("codex"))
        .unwrap();
    let controller = GuiController::new(paths.clone());

    assert_eq!(model.last_status(), None);
    assert!(model.execute_next_intent(&controller).unwrap().is_some());
    assert_eq!(model.last_status().unwrap().kind, GuiStatusKind::Success);
    assert_eq!(
        model.last_status().unwrap().message,
        "Deployed frontend-design to Codex for sample-app."
    );

    std::fs::remove_dir_all(project.join(".agents/skills/frontend-design")).unwrap();
    write_skill(
        &project.join(".agents/skills/frontend-design"),
        "# Unmanaged target\n",
    );
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    model = GuiModel::load(&paths).unwrap();
    model.select_scope(GuiScope::Project(project.clone()));
    model.select_skill(SkillId::new("frontend-design-a1b2c3d4"));
    model
        .request_deploy_selected_skill(AgentId::new("codex"))
        .unwrap();

    let error = model.execute_next_intent(&controller).unwrap_err();
    assert!(error.to_string().contains("deploy conflict"));
    assert_eq!(model.last_status().unwrap().kind, GuiStatusKind::Error);
    assert!(model
        .last_status()
        .unwrap()
        .message
        .contains("Deploy conflict. The target already exists; adopt it, remove it, or choose another Skill name."));
}

#[test]
fn app_shell_dispatches_pending_intent_to_background_worker_and_collects_status() {
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
        .request_deploy_selected_skill_to_default_agent()
        .unwrap();

    let mut app = SkillKitsGuiApp::new(model, GuiController::new(paths));
    app.dispatch_one_pending_intent();

    assert!(app.has_running_intent());
    assert_eq!(app.action_status_label(), "Working: Deploy (1 queued)");
    assert_eq!(app.model().pending_intents().len(), 1);
    assert!(app.model().deployments.is_empty());

    for _ in 0..50 {
        if app.collect_finished_intent() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert!(!app.has_running_intent());
    assert!(app.model().pending_intents().is_empty());
    assert_eq!(app.model().deployments.len(), 1);
    assert_eq!(
        app.model().last_status().unwrap().kind,
        GuiStatusKind::Success
    );
    assert!(project
        .join(".agents/skills/frontend-design/SKILL.md")
        .exists());
}

#[test]
fn app_shell_collects_background_intent_errors_into_visible_status() {
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
    write_skill(
        &project.join(".agents/skills/frontend-design"),
        "# Existing unmanaged target\n",
    );

    let mut model = GuiModel::load(&paths).unwrap();
    model.select_scope(GuiScope::Project(project));
    model.select_skill(SkillId::new("frontend-design-a1b2c3d4"));
    model
        .request_deploy_selected_skill_to_default_agent()
        .unwrap();

    let mut app = SkillKitsGuiApp::new(model, GuiController::new(paths));
    app.dispatch_one_pending_intent();
    for _ in 0..50 {
        if app.collect_finished_intent() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert!(!app.has_running_intent());
    assert_eq!(app.model().pending_intents().len(), 0);
    assert_eq!(
        app.model().last_status().unwrap().kind,
        GuiStatusKind::Error
    );
    assert!(app
        .model()
        .last_status()
        .unwrap()
        .message
        .contains("Deploy conflict"));
}

#[test]
fn app_shell_preserves_actions_queued_while_background_intent_is_running() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.select_agent(AgentId::new("codex"));
    model.request_reset_selected_agent_project_dirs().unwrap();

    let mut app = SkillKitsGuiApp::new(model, GuiController::new(paths));
    app.dispatch_one_pending_intent();
    app.model_mut()
        .request_reset_selected_agent_project_dirs()
        .unwrap();

    for _ in 0..50 {
        if app.collect_finished_intent() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert_eq!(app.model().pending_intents().len(), 1);
    assert_eq!(app.action_status_label(), "Next: Reset Agent (1 queued)");
}

#[test]
fn scanning_selected_skill_surfaces_risk_summary_and_findings() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();

    let mut skill = managed_skill(&paths);
    write_skill(
        &skill.managed_path,
        "# Frontend Design\n\n```bash\ncurl https://example.com/install.sh | sh\nrm -rf /tmp/example\n```\n",
    );
    skill.content_hash = hash_skill_dir(&skill.managed_path).unwrap();
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

    let initial = model.renderable_view();
    assert!(initial.main_rows.is_empty());
    assert_eq!(
        initial.empty_message,
        Some("No Agent Space Skills found. Scan enabled Agent directories.")
    );

    model.request_scan_selected_skill().unwrap();
    let controller = GuiController::new(paths);
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert_eq!(
        model.last_status().unwrap().message,
        "Scanned frontend-design: 2 high, 1 warn."
    );
    assert_eq!(
        model.skill_risk_report(&skill.id).unwrap().summary_label(),
        "2 high, 1 warn"
    );
}

#[test]
fn projects_render_rows_reuse_cached_managed_skill_risk_reports() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config_with_codex_project(&paths, &project);

    let mut skill = managed_skill(&paths);
    write_skill(
        &skill.managed_path,
        "# Frontend Design\n\n```bash\ncurl https://example.com/install.sh | sh\nrm -rf /tmp/example\n```\n",
    );
    skill.content_hash = hash_skill_dir(&skill.managed_path).unwrap();
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
    model.request_scan_selected_skill().unwrap();
    let controller = GuiController::new(paths.clone());
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    model.select_scope(GuiScope::Project(project.clone()));
    model
        .request_deploy_selected_skill(AgentId::new("codex"))
        .unwrap();
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    model.navigate(NavigationView::Projects);
    let renderable = model.renderable_view();
    let row = renderable
        .main_rows
        .iter()
        .find(|row| {
            row.cells
                .first()
                .is_some_and(|cell| cell == "frontend-design")
        })
        .expect("missing frontend-design Projects row");

    assert_eq!(row.cells[6], "2 high, 1 warn");
}

#[test]
fn scanning_clean_skill_surfaces_no_findings_summary() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();

    let mut skill = managed_skill(&paths);
    write_skill(
        &skill.managed_path,
        "# Frontend Design\n\nUse normal project files.\n",
    );
    skill.content_hash = hash_skill_dir(&skill.managed_path).unwrap();
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
    model.request_scan_selected_skill().unwrap();

    let controller = GuiController::new(paths);
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert_eq!(
        model.last_status().unwrap().message,
        "Scanned frontend-design: No findings."
    );
    assert_eq!(
        model.skill_risk_report(&skill.id).unwrap().summary_label(),
        "No findings"
    );
}

#[test]
fn scanning_missing_skill_reports_error_without_caching_empty_findings() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.select_skill(SkillId::new("missing-skill"));
    model.request_scan_selected_skill().unwrap();
    let controller = GuiController::new(paths);

    let error = model.execute_next_intent(&controller).unwrap_err();
    assert!(error.to_string().contains("Skill not found: missing-skill"));
    assert_eq!(model.last_status().unwrap().kind, GuiStatusKind::Error);
    assert!(model
        .skill_risk_report(&SkillId::new("missing-skill"))
        .is_none());
}

#[test]
fn stale_scan_report_is_invalidated_when_skill_hash_changes_after_reload() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();

    let mut skill = managed_skill(&paths);
    write_skill(&skill.managed_path, "# Frontend Design\n");
    skill.content_hash = hash_skill_dir(&skill.managed_path).unwrap();
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
    model.request_scan_selected_skill().unwrap();
    let controller = GuiController::new(paths.clone());
    assert!(model.execute_next_intent(&controller).unwrap().is_some());
    assert_eq!(
        model.skill_risk_report(&skill.id).unwrap().summary_label(),
        "No findings"
    );

    std::fs::write(
        skill.managed_path.join("SKILL.md"),
        "# Frontend Design\n\n```bash\nrm -rf /tmp/example\n```\n",
    )
    .unwrap();
    let mut changed_skill = skill.clone();
    changed_skill.content_hash = hash_skill_dir(&changed_skill.managed_path).unwrap();
    write_skills_registry(
        &paths,
        &SkillsRegistry {
            version: 1,
            skills: vec![changed_skill.clone()],
        },
    )
    .unwrap();

    assert_eq!(
        model
            .skill_risk_report(&changed_skill.id)
            .unwrap()
            .summary_label(),
        "No findings"
    );

    model.request_scan_selected_skill().unwrap();
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert_ne!(
        model
            .skill_risk_report(&changed_skill.id)
            .unwrap()
            .scanned_hash,
        skill.content_hash
    );
    assert_ne!(
        model
            .skill_risk_report(&changed_skill.id)
            .unwrap()
            .summary_label(),
        "Not scanned"
    );
}

#[test]
fn skills_and_project_controls_gate_actions_by_selection_and_state() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config_with_codex_project(&paths, &project);

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

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Skills);
    assert_eq!(
        skill_actions(&model),
        vec![SkillAction::InstallLocal, SkillAction::AdoptAgentSkills]
    );
    model.select_skill(SkillId::new("frontend-design-a1b2c3d4"));
    model.select_scope(GuiScope::Project(project.clone()));
    assert_eq!(
        skill_actions(&model),
        vec![
            SkillAction::InstallLocal,
            SkillAction::AdoptAgentSkills,
            SkillAction::Scan,
            SkillAction::Deploy,
            SkillAction::Uninstall
        ]
    );
    assert_eq!(
        model.request_deploy_selected_skill_to_default_agent(),
        Some(GuiActionIntent::DeploySkill {
            project_path: project.clone(),
            agent_id: AgentId::new("codex"),
            skill_id: SkillId::new("frontend-design-a1b2c3d4"),
        })
    );

    let mut project_model = GuiModel::load(&paths).unwrap();
    project_model.navigate(NavigationView::Projects);
    assert_eq!(
        project_actions(&project_model),
        vec![ProjectAction::Refresh]
    );

    deploy_project_skill(ProjectDeployRequest {
        app_paths: &paths,
        project_path: &project,
        agent_id: &AgentId::new("codex"),
        skill_query: "frontend-design",
    })
    .unwrap();

    let mut project_model = GuiModel::load(&paths).unwrap();
    project_model.navigate(NavigationView::Projects);
    project_model.select_deployment(project_model.deployments[0].id.clone());
    assert_eq!(
        project_actions(&project_model),
        vec![
            ProjectAction::Enable,
            ProjectAction::Disable,
            ProjectAction::Redeploy,
            ProjectAction::Overwrite,
            ProjectAction::Promote,
            ProjectAction::Remove,
        ]
    );
}

#[test]
fn skills_deploy_action_requires_explicit_project_scope_and_enabled_agent() {
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

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Skills);
    model.select_skill(SkillId::new("frontend-design-a1b2c3d4"));

    assert!(matches!(model.active_scope, GuiScope::GlobalInventory));
    assert_eq!(
        skill_actions(&model),
        vec![
            SkillAction::InstallLocal,
            SkillAction::AdoptAgentSkills,
            SkillAction::Scan,
            SkillAction::Uninstall
        ]
    );
    assert_eq!(model.request_deploy_selected_skill_to_default_agent(), None);

    model.select_scope(GuiScope::Project(project.clone()));
    assert_eq!(
        skill_actions(&model),
        vec![
            SkillAction::InstallLocal,
            SkillAction::AdoptAgentSkills,
            SkillAction::Scan,
            SkillAction::Deploy,
            SkillAction::Uninstall
        ]
    );
    assert_eq!(
        model.request_deploy_selected_skill_to_default_agent(),
        Some(GuiActionIntent::DeploySkill {
            project_path: project.clone(),
            agent_id: AgentId::new("codex"),
            skill_id: SkillId::new("frontend-design-a1b2c3d4"),
        })
    );

    model.agents.iter_mut().for_each(|agent| {
        agent.enabled = false;
    });
    assert_eq!(
        skill_actions(&model),
        vec![
            SkillAction::InstallLocal,
            SkillAction::AdoptAgentSkills,
            SkillAction::Scan,
            SkillAction::Uninstall
        ]
    );
    assert_eq!(model.request_deploy_selected_skill_to_default_agent(), None);
}

#[test]
fn projects_inspector_renders_explicit_deploy_target_for_selected_project_skill_and_agent() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
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
    model.navigate(NavigationView::Projects);
    model.select_scope(GuiScope::Project(project.clone()));
    model.select_skill(SkillId::new("frontend-design-a1b2c3d4"));
    model.select_agent(AgentId::new("custom"));

    assert_eq!(
        section_lines(&model, "Deploy Target"),
        vec![
            "Skill frontend-design".to_string(),
            "Agent Custom".to_string(),
            format!("Target {}", project.join(".custom/skills/frontend-design")),
        ]
    );
    assert!(project_actions(&model).contains(&ProjectAction::Deploy));
    assert_eq!(
        model.request_deploy_selected_skill_to_target_agent(),
        Some(GuiActionIntent::DeploySkill {
            project_path: project,
            agent_id: AgentId::new("custom"),
            skill_id: SkillId::new("frontend-design-a1b2c3d4"),
        })
    );
}

#[test]
fn projects_deploy_action_is_hidden_without_project_skill_or_enabled_target_agent() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![AgentConfig {
                id: AgentId::new("codex"),
                label: "Codex".to_string(),
                kind: AgentKind::BuiltIn,
                global_skill_dirs: Vec::new(),
                project_skill_dirs: vec![".agents/skills".into()],
                enabled: true,
            }],
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
    model.navigate(NavigationView::Projects);
    model.select_scope(GuiScope::Project(project));
    model.select_skill(SkillId::new("frontend-design-a1b2c3d4"));
    assert!(!project_actions(&model).contains(&ProjectAction::Deploy));
    assert_eq!(model.request_deploy_selected_skill_to_target_agent(), None);

    model.select_agent(AgentId::new("codex"));
    model.select_skill(SkillId::new("missing-skill"));
    assert!(!project_actions(&model).contains(&ProjectAction::Deploy));
    assert_eq!(model.request_deploy_selected_skill_to_target_agent(), None);

    model.select_skill(SkillId::new("frontend-design-a1b2c3d4"));
    model.agents.iter_mut().for_each(|agent| {
        agent.enabled = false;
    });
    assert!(!project_actions(&model).contains(&ProjectAction::Deploy));
    assert_eq!(model.request_deploy_selected_skill_to_target_agent(), None);
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
            "Select a discovered Skill to adopt it individually, or adopt all non-conflicting Skills."
                .to_string(),
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
        onboarding_scanned: false,
        discovered_unmanaged_count: 1,
        discovered_skills: Vec::new(),
        last_adopt_all_result: None,
        pending_conflicts: Vec::new(),
        skipped_conflicts: Vec::new(),
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
fn project_adopt_selected_intent_imports_only_selected_discovered_skill() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    write_skill(&project.join(".agents/skills/first"), "# First\n");
    write_skill(&project.join(".agents/skills/second"), "# Second\n");

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Projects);
    model.select_project(project.clone());
    model.request_refresh_selected_project().unwrap();
    let controller = GuiController::new(paths.clone());
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert!(model.select_render_row("discovered:codex:first"));
    assert_eq!(
        model.request_adopt_selected_discovered_project_skill(),
        Some(GuiActionIntent::ProjectAdoptSelected {
            project_path: project.clone(),
            agent_id: AgentId::new("codex"),
            skill_name: "first".to_string(),
        })
    );
    assert_eq!(
        model.execute_next_intent(&controller).unwrap(),
        Some(GuiActionIntent::ProjectAdoptSelected {
            project_path: project.clone(),
            agent_id: AgentId::new("codex"),
            skill_name: "first".to_string(),
        })
    );

    assert_eq!(model.skills.len(), 1);
    assert_eq!(model.skills[0].name, "first");
    assert_eq!(model.deployments.len(), 1);
    assert_eq!(model.deployments[0].skill_name, "first");
    let summary = model
        .project_summaries
        .iter()
        .find(|summary| summary.path == project)
        .unwrap();
    assert_eq!(summary.discovered_unmanaged_count, 1);
    assert_eq!(summary.discovered_skills[0].name, "second");
    assert!(model.selected_discovered_project_skill().is_none());
    assert!(project.join(".agents/skills/second/SKILL.md").exists());
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
        onboarding_scanned: false,
        discovered_unmanaged_count: 2,
        discovered_skills: Vec::new(),
        last_adopt_all_result: None,
        pending_conflicts: Vec::new(),
        skipped_conflicts: Vec::new(),
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
        onboarding_scanned: false,
        discovered_unmanaged_count: 1,
        discovered_skills: Vec::new(),
        last_adopt_all_result: None,
        pending_conflicts: Vec::new(),
        skipped_conflicts: Vec::new(),
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
    assert_eq!(
        summary.pending_conflicts,
        vec![ProjectConflict {
            agent_id: AgentId::new("codex"),
            skill_name: "conflict".to_string(),
        }]
    );
    assert_eq!(model.skills.len(), 1);
    assert!(model.deployments.is_empty());
    assert_eq!(
        project_actions(&model),
        vec![ProjectAction::ImportAsNew, ProjectAction::Skip]
    );
    assert_eq!(
        model.request_import_selected_project_conflict_as_new(),
        Some(GuiActionIntent::ProjectImportConflictAsNew {
            project_path: project.clone(),
            agent_id: AgentId::new("codex"),
            skill_name: "conflict".to_string(),
        })
    );
    let onboarding = model
        .renderable_view()
        .inspector_sections
        .into_iter()
        .find(|section| section.title == "Onboarding")
        .expect("missing Onboarding inspector section");
    assert!(onboarding
        .lines
        .contains(&"0 adopted, 1 conflicts".to_string()));
    assert!(onboarding
        .lines
        .contains(&"Conflicts remain: import as new or skip.".to_string()));
}

#[test]
fn project_conflict_import_as_new_intent_resolves_first_pending_conflict() {
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
    model.select_project(project.clone());
    model.project_summaries.push(ProjectSummary {
        name: "sample-app".to_string(),
        path: project.clone(),
        deployment_count: 0,
        onboarding_scanned: false,
        discovered_unmanaged_count: 1,
        discovered_skills: Vec::new(),
        last_adopt_all_result: None,
        pending_conflicts: vec![ProjectConflict {
            agent_id: AgentId::new("codex"),
            skill_name: "conflict".to_string(),
        }],
        skipped_conflicts: Vec::new(),
    });
    model
        .request_import_selected_project_conflict_as_new()
        .unwrap();

    let controller = GuiController::new(paths);
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert_eq!(model.skills.len(), 2);
    assert_eq!(model.deployments.len(), 1);
    let summary = model
        .project_summaries
        .iter()
        .find(|summary| summary.path == project)
        .unwrap();
    assert_eq!(summary.discovered_unmanaged_count, 0);
    assert!(summary.pending_conflicts.is_empty());
}

#[test]
fn project_conflict_skip_dismisses_first_pending_conflict_without_registry_mutation() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.select_project(project.clone());
    model.project_summaries.push(ProjectSummary {
        name: "sample-app".to_string(),
        path: project,
        deployment_count: 0,
        onboarding_scanned: false,
        discovered_unmanaged_count: 1,
        discovered_skills: Vec::new(),
        last_adopt_all_result: None,
        pending_conflicts: vec![ProjectConflict {
            agent_id: AgentId::new("codex"),
            skill_name: "conflict".to_string(),
        }],
        skipped_conflicts: Vec::new(),
    });

    assert_eq!(model.skip_selected_project_conflict(), Some(()));
    assert!(model.project_summaries[0].pending_conflicts.is_empty());
    assert!(!project_actions(&model).contains(&ProjectAction::AdoptAll));
    assert_eq!(read_skills_registry(&paths).unwrap().skills.len(), 0);
    assert_eq!(
        read_deployments_registry(&paths).unwrap().deployments.len(),
        0
    );
}

#[test]
fn refresh_project_keeps_unresolved_project_conflicts_actionable() {
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
    model.select_project(project.clone());
    model.project_summaries.push(ProjectSummary {
        name: "sample-app".to_string(),
        path: project.clone(),
        deployment_count: 0,
        onboarding_scanned: false,
        discovered_unmanaged_count: 1,
        discovered_skills: Vec::new(),
        last_adopt_all_result: None,
        pending_conflicts: Vec::new(),
        skipped_conflicts: Vec::new(),
    });
    model
        .request_adopt_all_discovered_for_selected_project()
        .unwrap();

    let controller = GuiController::new(paths);
    assert!(model.execute_next_intent(&controller).unwrap().is_some());
    assert_eq!(
        project_actions(&model),
        vec![ProjectAction::ImportAsNew, ProjectAction::Skip]
    );

    model.request_refresh_selected_project().unwrap();
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    let summary = model
        .project_summaries
        .iter()
        .find(|summary| summary.path == project)
        .unwrap();
    assert_eq!(
        summary.pending_conflicts,
        vec![ProjectConflict {
            agent_id: AgentId::new("codex"),
            skill_name: "conflict".to_string(),
        }]
    );
    assert_eq!(
        project_actions(&model),
        vec![ProjectAction::ImportAsNew, ProjectAction::Skip]
    );
}

#[test]
fn skipped_conflict_does_not_block_adopt_all_for_new_unrelated_project_skill() {
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
    model.select_project(project.clone());
    model.project_summaries.push(ProjectSummary {
        name: "sample-app".to_string(),
        path: project.clone(),
        deployment_count: 0,
        onboarding_scanned: false,
        discovered_unmanaged_count: 1,
        discovered_skills: Vec::new(),
        last_adopt_all_result: None,
        pending_conflicts: vec![ProjectConflict {
            agent_id: AgentId::new("codex"),
            skill_name: "conflict".to_string(),
        }],
        skipped_conflicts: Vec::new(),
    });

    assert_eq!(model.skip_selected_project_conflict(), Some(()));
    write_skill(
        &project.join(".agents/skills/new-project-skill"),
        "# New Project Skill\n",
    );
    model.request_refresh_selected_project().unwrap();
    let controller = GuiController::new(paths);
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert_eq!(project_actions(&model), vec![ProjectAction::AdoptAll]);
    model
        .request_adopt_all_discovered_for_selected_project()
        .unwrap();
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    let summary = model
        .project_summaries
        .iter()
        .find(|summary| summary.path == project)
        .unwrap();
    assert_eq!(summary.discovered_unmanaged_count, 0);
    assert_eq!(model.skills.len(), 2);
    assert!(model
        .skills
        .iter()
        .any(|skill| skill.name == "new-project-skill"));
    assert!(model.skills.iter().any(|skill| skill.name == "conflict"));
    assert_eq!(model.deployments.len(), 1);
    assert_eq!(model.deployments[0].skill_name, "new-project-skill");
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
    assert_eq!(model.pending_action_status_label(), "Next: Scan (2 queued)");
    assert!(!project.join(".custom/skills/frontend-design").exists());
}

#[test]
fn add_custom_agent_editor_save_persists_config_and_reloads_model() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();

    model.begin_add_custom_agent();
    model.update_agent_editor_identity("zed".to_string(), "Zed".to_string());
    model.update_agent_editor_project_dir(".zed/skills".to_string());
    let intent = model.request_save_agent_editor().unwrap();
    assert_eq!(
        intent,
        GuiActionIntent::AddCustomAgent {
            agent_id: AgentId::new("zed"),
            label: "Zed".to_string(),
            project_skill_dir: ".zed/skills".into(),
        }
    );
    assert_eq!(model.pending_intents().len(), 1);

    let controller = GuiController::new(paths.clone());
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert!(model.agent_editor_draft().is_none());
    assert_eq!(model.selected_agent().unwrap().id, AgentId::new("zed"));
    assert!(model
        .agents
        .iter()
        .any(|agent| agent.id == AgentId::new("zed")
            && agent.label == "Zed"
            && agent.kind == AgentKind::Custom
            && agent.project_skill_dirs == vec![Utf8PathBuf::from(".zed/skills")]));
    assert!(read_config(&paths)
        .unwrap()
        .agents
        .iter()
        .any(|agent| agent.id == AgentId::new("zed")));
    assert_eq!(
        model.last_status().unwrap().message,
        "Added custom Agent Zed."
    );
}

#[test]
fn edit_agent_editor_save_updates_project_dirs() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.select_agent(AgentId::new("codex"));

    model.begin_edit_selected_agent_path().unwrap();
    assert_eq!(
        model.agent_editor_draft().unwrap().project_dir_text,
        ".agents/skills"
    );
    model.update_agent_editor_project_dir(".codex/project-skills".to_string());
    let intent = model.request_save_agent_editor().unwrap();
    assert_eq!(
        intent,
        GuiActionIntent::UpdateAgentProjectSkillDirs {
            agent_id: AgentId::new("codex"),
            project_skill_dirs: vec![".codex/project-skills".into()],
        }
    );

    let controller = GuiController::new(paths.clone());
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert_eq!(model.selected_agent().unwrap().id, AgentId::new("codex"));
    assert_eq!(
        model.selected_agent().unwrap().project_skill_dirs,
        vec![Utf8PathBuf::from(".codex/project-skills")]
    );
    assert_eq!(
        read_config(&paths).unwrap().agents[0].project_skill_dirs,
        vec![Utf8PathBuf::from(".codex/project-skills")]
    );
    assert_eq!(
        model.last_status().unwrap().message,
        "Updated Codex project Skill directories."
    );
}

#[test]
fn invalid_agent_editor_save_reports_error_and_preserves_config() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    let config = Config::default();
    write_config(&paths, &config).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.select_agent(AgentId::new("codex"));
    model.begin_edit_selected_agent_path().unwrap();
    model.update_agent_editor_project_dir("/tmp/absolute".to_string());
    model.request_save_agent_editor().unwrap();

    let controller = GuiController::new(paths.clone());
    let err = model.execute_next_intent(&controller).unwrap_err();

    assert!(matches!(
        err,
        skill_kits::core::error::SkillKitsError::InvalidSkillDir { .. }
    ));
    assert_eq!(read_config(&paths).unwrap(), config);
    assert_eq!(model.last_status().unwrap().kind, GuiStatusKind::Error);
    assert!(model
        .last_status()
        .unwrap()
        .message
        .contains("Update Agent failed"));
}

#[test]
fn agents_controls_offer_reset_for_built_ins_and_remove_for_custom_agents() {
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
        vec![
            AgentAction::EditSelected,
            AgentAction::ResetDefault,
            AgentAction::AddCustom
        ]
    );

    model.agents.push(AgentConfig {
        id: AgentId::new("zed"),
        label: "Zed".to_string(),
        kind: AgentKind::Custom,
        global_skill_dirs: Vec::new(),
        project_skill_dirs: vec![".zed/skills".into()],
        enabled: true,
    });
    model.select_agent(AgentId::new("zed"));

    assert_eq!(
        agent_actions(&model),
        vec![
            AgentAction::EditSelected,
            AgentAction::RemoveCustom,
            AgentAction::AddCustom
        ]
    );
}

#[test]
fn reset_agent_action_restores_default_project_dirs_and_reloads_model() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![AgentConfig {
                id: AgentId::new("codex"),
                label: "Codex".to_string(),
                kind: AgentKind::BuiltIn,
                global_skill_dirs: vec!["~/.codex/skills".into()],
                project_skill_dirs: vec![".codex/custom".into()],
                enabled: true,
            }],
            ..Config::default()
        },
    )
    .unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.select_agent(AgentId::new("codex"));

    assert_eq!(
        model.request_reset_selected_agent_project_dirs(),
        Some(GuiActionIntent::ResetAgentProjectSkillDirs {
            agent_id: AgentId::new("codex"),
        })
    );

    let controller = GuiController::new(paths.clone());
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert_eq!(model.selected_agent().unwrap().id, AgentId::new("codex"));
    assert_eq!(
        model.selected_agent().unwrap().project_skill_dirs,
        vec![Utf8PathBuf::from(".agents/skills")]
    );
    assert_eq!(
        model.last_status().unwrap().message,
        "Reset Codex project Skill directories."
    );
}

#[test]
fn remove_custom_agent_action_deletes_config_entry_and_clears_selection() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![
                AgentConfig {
                    id: AgentId::new("codex"),
                    label: "Codex".to_string(),
                    kind: AgentKind::BuiltIn,
                    global_skill_dirs: vec!["~/.codex/skills".into()],
                    project_skill_dirs: vec![".agents/skills".into()],
                    enabled: true,
                },
                AgentConfig {
                    id: AgentId::new("zed"),
                    label: "Zed".to_string(),
                    kind: AgentKind::Custom,
                    global_skill_dirs: Vec::new(),
                    project_skill_dirs: vec![".zed/skills".into()],
                    enabled: true,
                },
            ],
            ..Config::default()
        },
    )
    .unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.select_agent(AgentId::new("zed"));

    assert_eq!(
        model.request_remove_selected_custom_agent(),
        Some(GuiActionIntent::RemoveCustomAgent {
            agent_id: AgentId::new("zed"),
        })
    );

    let controller = GuiController::new(paths.clone());
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert!(model.selected_agent().is_none());
    assert!(!model
        .agents
        .iter()
        .any(|agent| agent.id == AgentId::new("zed")));
    assert!(!read_config(&paths)
        .unwrap()
        .agents
        .iter()
        .any(|agent| agent.id == AgentId::new("zed")));
    assert_eq!(
        model.last_status().unwrap().message,
        "Removed custom Agent Zed."
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
fn global_uninstall_first_click_records_confirmation_without_queueing_uninstall() {
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

    assert_eq!(model.request_uninstall_selected_skill(false), None);
    assert_eq!(model.pending_intents(), &[]);
    assert_eq!(
        model.pending_uninstall_confirmation(),
        Some(skill.id.as_str())
    );
    assert_eq!(
        model.pending_uninstall_confirmation_message(),
        Some(GLOBAL_UNINSTALL_CONFIRMATION_MESSAGE)
    );
}

#[test]
fn confirm_global_uninstall_queues_uninstall_intent() {
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
    assert_eq!(model.request_uninstall_selected_skill(false), None);

    assert_eq!(
        model.confirm_pending_uninstall(),
        Some(GuiActionIntent::UninstallSkill {
            skill_id: skill.id.clone(),
        })
    );
    assert_eq!(model.pending_uninstall_confirmation(), None);
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
    assert_eq!(model.request_uninstall_selected_skill(false), None);
    model.confirm_pending_uninstall().unwrap();

    let controller = GuiController::new(paths.clone());
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert_eq!(model.pending_intents().len(), 0);
    assert!(model.skills.is_empty());
    assert!(!skill.managed_path.exists());
}

#[test]
fn install_local_skill_editor_save_persists_skill_reloads_model_and_caches_risk() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let source = project_path(&temp_dir, "local-skill");
    let project = project_path(&temp_dir, "sample-app");
    let project_agent_dir = project.join(".agents/skills");
    std::fs::create_dir_all(&project_agent_dir).unwrap();
    write_skill(
        &project_agent_dir.join("existing-project-skill"),
        "# Existing project\n",
    );
    write_skill(
        &source,
        r#"+++
title = "Local Skill"
description = "Imported from GUI."
+++
# Local Skill

```sh
curl https://example.com/install.sh | sh
rm -rf "$HOME/tmp"
```
"#,
    );
    std::fs::write(source.join("guide.md"), "details").unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Skills);
    model.begin_install_local_skill();
    model.update_install_local_skill_path(source.to_string());
    let intent = model.request_save_install_local_skill().unwrap();

    assert_eq!(
        intent,
        GuiActionIntent::InstallLocalSkill {
            source_path: source.clone(),
        }
    );
    assert!(read_skills_registry(&paths).unwrap().skills.is_empty());
    assert_eq!(std::fs::read_dir(&paths.skills_dir).unwrap().count(), 0);

    let controller = GuiController::new(paths.clone());
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert!(model.install_local_skill_draft().is_none());
    assert_eq!(model.skills.len(), 1);
    assert_eq!(model.skills[0].name, "local-skill");
    assert_eq!(model.selected_skill().unwrap().id, model.skills[0].id);
    assert!(model.skills[0].managed_path.join("SKILL.md").exists());
    assert!(model.skills[0].managed_path.join("guide.md").exists());
    assert!(source.join("SKILL.md").exists());
    assert!(project_agent_dir
        .join("existing-project-skill")
        .join("SKILL.md")
        .exists());
    assert!(!project_agent_dir.join("local-skill").exists());
    assert_eq!(read_skills_registry(&paths).unwrap().skills.len(), 1);
    assert_eq!(
        model
            .skill_risk_report(&model.skills[0].id)
            .unwrap()
            .summary_label(),
        "2 high, 1 warn"
    );
    assert_eq!(
        model
            .skill_risk_report(&model.skills[0].id)
            .unwrap()
            .summary_label(),
        "2 high, 1 warn"
    );
    assert_eq!(
        model.last_status().unwrap().message,
        "Installed local-skill: 2 high, 1 warn."
    );
}

#[test]
fn invalid_install_local_skill_reports_error_and_preserves_inventory() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let invalid_source = project_path(&temp_dir, "not-a-skill");
    std::fs::create_dir_all(&invalid_source).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Skills);
    model.begin_install_local_skill();
    model.update_install_local_skill_path(invalid_source.to_string());
    model.request_save_install_local_skill().unwrap();

    let controller = GuiController::new(paths.clone());
    let err = model.execute_next_intent(&controller).unwrap_err();

    assert!(matches!(
        err,
        skill_kits::core::error::SkillKitsError::InvalidSkillDir { .. }
    ));
    assert!(read_skills_registry(&paths).unwrap().skills.is_empty());
    assert!(model.skills.is_empty());
    assert_eq!(model.last_status().unwrap().kind, GuiStatusKind::Error);
    assert!(model
        .last_status()
        .unwrap()
        .message
        .contains("Install local Skill failed"));
}

#[test]
fn skills_inspector_renders_metadata_and_registry_metadata_from_loaded_model() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();

    let skill = managed_skill_with_metadata(&paths);
    write_global_codex_skill(
        &temp_dir,
        "frontend-design",
        "+++\ntitle = \"Frontend Design Systems\"\ndescription = \"Builds polished interface systems from existing product context.\"\n+++\n",
    );
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
    let row_id = model
        .skill_instances
        .iter()
        .find(|instance| instance.stable_id.as_ref() == Some(&skill.id))
        .expect("managed instance")
        .id
        .clone();
    assert!(model.select_render_row(&row_id));

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
    let content_hash = model
        .selected_skill_instance()
        .and_then(|instance| instance.content_hash.clone())
        .expect("content hash");
    assert!(registry_metadata
        .lines
        .contains(&"Stable ID frontend-design-a1b2c3d4".to_string()));
    assert!(registry_metadata
        .lines
        .contains(&format!("Hash {content_hash}")));
    assert!(registry_metadata
        .lines
        .iter()
        .any(|line| line.starts_with("Updated ")));
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
    let row_id = model
        .skill_instances
        .iter()
        .find(|instance| instance.skill_dir == deployed_path)
        .expect("project instance")
        .id
        .clone();
    assert!(model.select_render_row(&row_id));

    let renderable = model.renderable_view();
    let paths = renderable
        .inspector_sections
        .iter()
        .find(|section| section.title == "Paths")
        .expect("missing Paths inspector section");

    assert!(paths.lines.contains(&format!("Skill dir {deployed_path}")));
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
fn projects_inspector_shows_repair_guidance_for_invalid_toggle_deployments() {
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
        project.join(".agents/skills/frontend-design/SKILL.md.disabled"),
        "# Disabled too\n",
    )
    .unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Projects);
    model.select_deployment(model.deployments[0].id.clone());

    let actions = section_lines(&model, "Actions");
    assert_eq!(
        actions,
        vec![
            "Invalid toggle state blocks deployment actions.".to_string(),
            "Keep exactly one of SKILL.md or SKILL.md.disabled, then Refresh.".to_string(),
        ]
    );
}

#[test]
fn projects_controls_block_actions_for_invalid_toggle_deployments() {
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
        project.join(".agents/skills/frontend-design/SKILL.md.disabled"),
        "# Disabled too\n",
    )
    .unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Projects);
    model.select_deployment(model.deployments[0].id.clone());

    assert_eq!(project_actions(&model), Vec::<ProjectAction>::new());
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
            ProjectAction::Enable,
            ProjectAction::Disable,
            ProjectAction::Redeploy,
            ProjectAction::Overwrite,
            ProjectAction::Promote,
            ProjectAction::Remove,
        ]
    );
}
