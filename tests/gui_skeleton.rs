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
    agent_actions, inspector_line_presentation, native_options, path_validation_message,
    plugin_actions, project_actions, skill_actions, workbench_cell_style,
    workbench_row_accepts_keyboard_key, AgentAction, InspectorLineKind, InspectorLinePresentation,
    PathFieldKind, PluginAction, ProjectAction, SkillAction, SkillKitsGuiApp, WorkbenchCellStyle,
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

fn write_file(path: &camino::Utf8Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn write_plugin_package(
    home: &camino::Utf8Path,
    provider: &str,
    name: &str,
    version: &str,
    display_name: &str,
) -> Utf8PathBuf {
    let package = home.join(format!(".codex/plugins/cache/{provider}/{name}/{version}"));
    write_file(
        &package.join(".codex-plugin/plugin.json"),
        &format!(r#"{{"name":"{name}","display_name":"{display_name}","version":"{version}"}}"#),
    );
    package
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
        vec!["Skill", "Agent", "Scope", "Status", "Source", "Updated"]
    );
    assert_eq!(renderable.main_rows.len(), 1);
    let row = &renderable.main_rows[0];
    assert_eq!(row.cells[0], "Agent Visible");
    assert_eq!(row.cells[1], "Codex");
    assert_eq!(row.cells[2], "Global");
    assert_eq!(row.cells[3], "Enabled");
    assert_eq!(row.cells[4], "Codex global");
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
        vec![SkillAction::ScanAgentSpaces, SkillAction::Disable]
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
        vec![SkillAction::ScanAgentSpaces, SkillAction::Enable]
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
fn plugin_cache_skill_instances_are_hidden_from_native_skill_view() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![AgentConfig {
                id: AgentId::new("codex"),
                label: "Codex".to_string(),
                kind: AgentKind::BuiltIn,
                global_skill_dirs: vec!["~/.codex/plugins/cache".into()],
                project_skill_dirs: vec![".agents/skills".into()],
                enabled: true,
            }],
            ..Config::default()
        },
    )
    .unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    let skill_dir = home.join(".codex/plugins/cache/openai/browser/1.0.0/skills/browser-skill");
    write_skill(&skill_dir, "# Browser Skill\n");

    let mut model = GuiModel::load_with_home_dir(&paths, home).unwrap();
    model.navigate(NavigationView::Skills);
    assert!(model.renderable_view().main_rows.is_empty());
    assert_eq!(skill_actions(&model), vec![SkillAction::ScanAgentSpaces]);
}

#[test]
fn skills_view_default_excludes_plugin_provided_skills_from_native_table() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    write_global_codex_skill(&temp_dir, "native-skill", "# Native Skill\n");
    write_skill(
        &home.join(".codex/plugins/cache/openai/browser/skills/browser-skill"),
        "# Browser Skill\n",
    );
    write_skill(
        &home.join(".codex/vendor_imports/skills/skills/.curated/vendor-skill"),
        "# Vendor Skill\n",
    );
    write_skill(
        &home.join(".skills-manager/skills/manager-only-skill"),
        "# Manager Only Skill\n",
    );

    let mut model = GuiModel::load_with_home_dir(&paths, home).unwrap();
    model.navigate(NavigationView::Skills);
    let renderable = model.renderable_view();

    assert_eq!(
        renderable
            .main_rows
            .iter()
            .map(|row| row.cells[0].as_str())
            .collect::<Vec<_>>(),
        vec!["Native Skill"]
    );
    assert_eq!(model.dashboard.agent_space_instance_count, 1);
    assert_eq!(model.dashboard.read_only_count, 0);
}

#[test]
fn skills_view_filters_by_agent_and_scope_without_changing_selection_identity() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
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
                    global_skill_dirs: vec!["~/.codex/skills".into()],
                    project_skill_dirs: vec![".agents/skills".into()],
                    enabled: true,
                },
                AgentConfig {
                    id: AgentId::new("custom"),
                    label: "Custom".to_string(),
                    kind: AgentKind::Custom,
                    global_skill_dirs: vec![home.join("custom/skills")],
                    project_skill_dirs: vec![".custom/skills".into()],
                    enabled: true,
                },
            ],
            recent_projects: vec![RecentProject {
                name: "sample-app".to_string(),
                path: project.clone(),
                last_opened_at: "2026-06-01T00:00:00Z".to_string(),
            }],
            ..Config::default()
        },
    )
    .unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    write_global_codex_skill(&temp_dir, "codex-global", "# Codex Global\n");
    write_skill(
        &home.join("custom/skills/custom-global"),
        "# Custom Global\n",
    );
    write_skill(
        &project.join(".agents/skills/codex-project"),
        "# Codex Project\n",
    );
    write_skill(
        &project.join(".custom/skills/custom-project"),
        "# Custom Project\n",
    );

    let mut model = GuiModel::load_with_home_dir(&paths, home).unwrap();
    model.navigate(NavigationView::Skills);
    let custom_row_id = model
        .renderable_view()
        .main_rows
        .iter()
        .find(|row| row.cells[0] == "Custom Global")
        .unwrap()
        .id
        .clone();
    assert!(model.select_render_row(&custom_row_id));

    model.set_skill_agent_filter(Some(AgentId::new("codex")));
    let renderable = model.renderable_view();
    assert_eq!(
        renderable
            .main_rows
            .iter()
            .map(|row| row.cells[0].as_str())
            .collect::<Vec<_>>(),
        vec!["Codex Global", "Codex Project"]
    );
    assert_eq!(
        model.selected_skill_instance().unwrap().id,
        custom_row_id,
        "filtering must not rewrite selected instance identity"
    );

    model.set_skill_scope_filter(Some("Global".to_string()));
    let renderable = model.renderable_view();
    assert_eq!(
        renderable
            .main_rows
            .iter()
            .map(|row| row.cells[0].as_str())
            .collect::<Vec<_>>(),
        vec!["Codex Global"]
    );
    assert_eq!(model.skill_agent_filter(), Some(&AgentId::new("codex")));
    assert_eq!(model.skill_scope_filter(), Some("Global"));
}

#[test]
fn skills_view_filters_by_status_and_exposes_filter_options() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    write_global_codex_skill(&temp_dir, "enabled-skill", "# Enabled\n");
    let disabled_dir = home.join(".codex/skills/disabled-skill");
    std::fs::create_dir_all(&disabled_dir).unwrap();
    std::fs::write(disabled_dir.join("SKILL.md.disabled"), "# Disabled\n").unwrap();
    let invalid_dir = home.join(".codex/skills/invalid-skill");
    write_skill(&invalid_dir, "# Enabled side\n");
    std::fs::write(invalid_dir.join("SKILL.md.disabled"), "# Disabled side\n").unwrap();
    let mut model = GuiModel::load_with_home_dir(&paths, home).unwrap();
    model.navigate(NavigationView::Skills);

    assert_eq!(
        model.skill_status_filter_options(),
        vec!["Enabled", "Disabled", "Invalid"]
    );

    model.set_skill_status_filter(Some("Read-only".to_string()));
    let renderable = model.renderable_view();
    assert_eq!(model.skill_status_filter(), Some("Read-only"));
    assert_eq!(
        renderable
            .main_rows
            .iter()
            .map(|row| row.cells[0].as_str())
            .collect::<Vec<_>>(),
        Vec::<&str>::new()
    );

    model.set_skill_status_filter(Some("Invalid".to_string()));
    let renderable = model.renderable_view();
    assert_eq!(
        renderable
            .main_rows
            .iter()
            .map(|row| row.cells[0].as_str())
            .collect::<Vec<_>>(),
        vec!["invalid-skill"]
    );
}

#[test]
fn selected_agent_space_instance_can_be_imported_as_managed_copy() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    let skill_dir = write_global_codex_skill(&temp_dir, "agent-visible", "# Agent Visible\n");

    let home = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
    let mut model = GuiModel::load_with_home_dir(&paths, home.clone()).unwrap();
    model.navigate(NavigationView::Skills);
    let row_id = model.renderable_view().main_rows[0].id.clone();
    assert!(model.select_render_row(&row_id));
    assert_eq!(
        skill_actions(&model),
        vec![SkillAction::ScanAgentSpaces, SkillAction::Disable]
    );
    assert_eq!(
        model.request_import_selected_skill_instance_as_managed_copy(),
        Some(GuiActionIntent::ImportManagedCopy {
            instance_id: row_id.clone(),
        })
    );

    let controller = GuiController::with_home_dir(paths.clone(), home);
    assert!(model.execute_next_intent(&controller).unwrap().is_some());
    assert_eq!(model.skills.len(), 1);
    assert_eq!(model.skills[0].name, "agent-visible");
    assert!(model.skills[0].managed_path.join("SKILL.md").exists());
    assert!(matches!(
        &model.skills[0].source,
        SkillSource::GlobalAgentAdopt { agent_id, source_path }
            if agent_id == &AgentId::new("codex") && source_path == &skill_dir
    ));
    assert!(skill_dir.join("SKILL.md").exists());
    assert_eq!(model.selected_skill_instance().unwrap().id, row_id);
    assert_eq!(
        model.last_status().unwrap().message,
        "Imported Agent Visible into Managed Inventory."
    );
}

#[test]
fn selected_project_skill_instance_import_records_project_source_and_deployment_link() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config_with_codex_project(&paths, &project);
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    let skill_dir = project.join(".agents/skills/project-only");
    write_skill(&skill_dir, "# Project Only\n");

    let home = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
    let mut model = GuiModel::load_with_home_dir(&paths, home).unwrap();
    model.navigate(NavigationView::Skills);
    let row_id = model
        .skill_instances
        .iter()
        .find(|instance| instance.skill_dir == skill_dir)
        .expect("project instance")
        .id
        .clone();
    assert!(model.select_render_row(&row_id));
    assert_eq!(
        skill_actions(&model),
        vec![SkillAction::ScanAgentSpaces, SkillAction::Disable]
    );
    assert_eq!(
        model.request_import_selected_skill_instance_as_managed_copy(),
        Some(GuiActionIntent::ImportManagedCopy {
            instance_id: row_id.clone(),
        })
    );

    let controller = GuiController::new(paths.clone());
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert_eq!(model.selected_skill_instance().unwrap().id, row_id);
    assert_eq!(model.skills.len(), 1);
    assert!(matches!(
        &model.skills[0].source,
        SkillSource::ProjectAdopt {
            agent_id,
            project_path,
            source_path,
        } if agent_id == &AgentId::new("codex")
            && project_path == &project
            && source_path == &skill_dir
    ));
    let deployments = read_deployments_registry(&paths).unwrap().deployments;
    assert_eq!(deployments.len(), 1);
    assert_eq!(deployments[0].deployment_path, skill_dir);
    assert_eq!(deployments[0].skill_id, model.skills[0].id);
}

#[test]
fn skills_inspector_includes_risk_findings_and_project_deployment_links_when_available() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    std::fs::create_dir_all(&project).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config_with_codex_project(&paths, &project);

    let mut skill = managed_skill(&paths);
    let source_body =
        "# Frontend Design\n\n```bash\ncurl https://example.com/install.sh | sh\n```\n";
    write_skill(&skill.managed_path, source_body);
    skill.content_hash = hash_skill_dir(&skill.managed_path).unwrap();
    let agent_space_skill = write_global_codex_skill(&temp_dir, "frontend-design", source_body);
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
    model.select_managed_skill(skill.id.clone());
    model.request_scan_selected_skill().unwrap();
    let controller = GuiController::new(paths);
    assert!(model.execute_next_intent(&controller).unwrap().is_some());
    model.navigate(NavigationView::Skills);
    let row_id = model
        .skill_instances
        .iter()
        .find(|instance| instance.skill_dir == agent_space_skill)
        .unwrap()
        .id
        .clone();
    assert!(model.select_render_row(&row_id));

    let sections = model.renderable_view().inspector_sections;
    assert!(sections
        .iter()
        .all(|section| section.title != "Risk Findings"));
    assert!(sections
        .iter()
        .all(|section| section.title != "Project Deployments"));
}

#[test]
fn skills_inspector_names_invalid_state_explicitly_and_hides_plugin_cache_rows() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config(
        &paths,
        &Config {
            agents: vec![AgentConfig {
                id: AgentId::new("codex"),
                label: "Codex".to_string(),
                kind: AgentKind::BuiltIn,
                global_skill_dirs: vec!["~/.codex/skills".into(), "~/.codex/plugins/cache".into()],
                project_skill_dirs: vec![".agents/skills".into()],
                enabled: true,
            }],
            ..Config::default()
        },
    )
    .unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let invalid = home.join(".codex/skills/invalid-skill");
    write_skill(&invalid, "# Enabled side\n");
    std::fs::write(invalid.join("SKILL.md.disabled"), "# Disabled side\n").unwrap();
    let read_only = home.join(".codex/plugins/cache/openai/browser/1.0.0/skills/browser-skill");
    write_skill(&read_only, "# Browser Skill\n");

    let mut model = GuiModel::load_with_home_dir(&paths, home).unwrap();
    model.navigate(NavigationView::Skills);
    let invalid_row_id = model
        .renderable_view()
        .main_rows
        .iter()
        .find(|row| row.cells[0] == "invalid-skill")
        .unwrap()
        .id
        .clone();
    assert!(model.select_render_row(&invalid_row_id));
    assert!(section_lines(&model, "State")
        .contains(&"Invalid: both SKILL.md and SKILL.md.disabled are present.".to_string()));

    assert!(model
        .renderable_view()
        .main_rows
        .iter()
        .all(|row| row.cells[0] != "Browser Skill"));
}

#[test]
fn disabling_skill_instance_requires_inline_confirmation_copy() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    let skill_dir = write_global_codex_skill(&temp_dir, "confirm-disable", "# Confirm\n");

    let home = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
    let mut model = GuiModel::load_with_home_dir(&paths, home).unwrap();
    model.navigate(NavigationView::Skills);
    let row_id = model.renderable_view().main_rows[0].id.clone();
    assert!(model.select_render_row(&row_id));

    assert_eq!(
        model.request_disable_selected_skill_instance_with_confirmation(false),
        None
    );
    assert_eq!(model.pending_intents(), &[]);
    assert_eq!(
        model.pending_disable_skill_instance_confirmation_message(),
        Some(skill_kits::gui::state::SKILL_INSTANCE_DISABLE_CONFIRMATION_MESSAGE)
    );
    assert!(skill_dir.join("SKILL.md").exists());

    assert_eq!(
        model.confirm_pending_disable_skill_instance(),
        Some(GuiActionIntent::DisableSkillInstance {
            instance_id: row_id.clone()
        })
    );
    assert_eq!(model.pending_intents().len(), 1);
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
fn navigation_titles_match_frozen_agent_space_shape() {
    assert_eq!(NavigationView::Dashboard.title(), "Dashboard");
    assert_eq!(NavigationView::Skills.title(), "Skill");
    assert_eq!(NavigationView::Agents.title(), "Agent");
    assert_eq!(NavigationView::Projects.title(), "Project");
    assert_eq!(NavigationView::Plugins.title(), "Plugins");

    let mut model = GuiModel::default();
    for (view, title) in [
        (NavigationView::Dashboard, "Dashboard"),
        (NavigationView::Skills, "Skill"),
        (NavigationView::Agents, "Agent"),
        (NavigationView::Projects, "Project"),
        (NavigationView::Plugins, "Plugins"),
    ] {
        model.navigate(view);
        assert_eq!(model.renderable_view().title, title);
    }
}

#[test]
fn plugins_view_lists_packages_and_read_only_capabilities() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    let package = write_plugin_package(&home, "openai-curated", "github", "1.0.0", "GitHub Tools");
    write_file(
        &package.join("skills/github/SKILL.md"),
        "+++\ntitle = \"GitHub Skill\"\ndescription = \"Works with GitHub.\"\n+++\n",
    );
    write_file(&package.join("commands/review.md"), "# Review\n");

    let mut model = GuiModel::load_with_home_dir(&paths, home).unwrap();
    model.navigate(NavigationView::Plugins);
    let renderable = model.renderable_view();

    assert_eq!(
        renderable.columns,
        vec![
            "Plugin",
            "Provider",
            "Agent",
            "Status",
            "Capabilities",
            "Path",
            "Updated"
        ]
    );
    assert_eq!(
        renderable
            .main_rows
            .iter()
            .map(|row| row.cells[0].as_str())
            .collect::<Vec<_>>(),
        vec!["GitHub Tools", "GitHub Skill", "review"]
    );

    let package_row_id = renderable.main_rows[0].id.clone();
    assert!(model.select_render_row(&package_row_id));
    assert_eq!(
        plugin_actions(&model),
        vec![PluginAction::ScanPlugins, PluginAction::Disable]
    );
    let summary = section_lines(&model, "Summary");
    assert!(summary.contains(&"Plugin key github@openai-curated".to_string()));
    let config = section_lines(&model, "Config");
    assert!(config.iter().any(|line| line.starts_with("Config path ")));
    assert!(config.contains(
        &"This plugin is managed through Codex plugin configuration. Skill-kits does not modify plugin package contents."
            .to_string()
    ));

    let capability_row_id = model
        .renderable_view()
        .main_rows
        .iter()
        .find(|row| row.cells[0] == "GitHub Skill")
        .unwrap()
        .id
        .clone();
    assert!(model.select_render_row(&capability_row_id));
    assert_eq!(plugin_actions(&model), vec![PluginAction::ScanPlugins]);
    assert!(section_lines(&model, "State").contains(
        &"This Skill is bundled by a Codex plugin. It is not a native Agent Space Skill and cannot be enabled or disabled by renaming SKILL.md."
            .to_string()
    ));
}

#[test]
fn plugins_view_offers_enable_for_disabled_package() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let home = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    write_plugin_package(&home, "openai-bundled", "browser", "1.0.0", "Browser");
    write_file(
        &home.join(".codex/config.toml"),
        "[plugins.\"browser@openai-bundled\"]\nenabled = false\n",
    );

    let mut model = GuiModel::load_with_home_dir(&paths, home).unwrap();
    model.navigate(NavigationView::Plugins);
    let row_id = model.renderable_view().main_rows[0].id.clone();
    assert!(model.select_render_row(&row_id));

    assert_eq!(
        plugin_actions(&model),
        vec![PluginAction::ScanPlugins, PluginAction::Enable]
    );
    assert_eq!(
        model.request_enable_selected_plugin(),
        Some(GuiActionIntent::EnablePlugin {
            plugin_id: row_id.clone()
        })
    );
}

#[test]
fn native_gui_options_integrate_dark_macos_title_area() {
    let options = native_options();
    assert_eq!(options.viewport.inner_size, Some(egui::vec2(1180.0, 760.0)));
    assert_eq!(
        options.viewport.min_inner_size,
        Some(egui::vec2(960.0, 620.0))
    );
    assert_eq!(options.viewport.fullsize_content_view, Some(true));
    assert_eq!(options.viewport.titlebar_shown, Some(false));
    assert_eq!(options.viewport.title_shown, Some(false));
    assert_eq!(options.viewport.titlebar_buttons_shown, Some(true));
    assert_ne!(options.viewport.decorations, Some(false));
}

#[test]
fn workbench_cell_styles_mark_statuses_and_paths_for_dense_rows() {
    assert_eq!(
        workbench_cell_style("Status"),
        WorkbenchCellStyle::StatusBadge
    );
    assert_eq!(
        workbench_cell_style("Toggle"),
        WorkbenchCellStyle::StatusBadge
    );
    assert_eq!(
        workbench_cell_style("Validation"),
        WorkbenchCellStyle::StatusBadge
    );
    assert_eq!(workbench_cell_style("Path"), WorkbenchCellStyle::Mono);
    assert_eq!(
        workbench_cell_style("Project skill directories"),
        WorkbenchCellStyle::Mono
    );
    assert_eq!(workbench_cell_style("Skill"), WorkbenchCellStyle::Text);
}

#[test]
fn path_validation_contracts_cover_open_project_and_agent_editor_drafts() {
    let temp_dir = TempDir::new().unwrap();
    let project = project_path(&temp_dir, "opened-app");
    std::fs::create_dir_all(&project).unwrap();

    assert_eq!(
        path_validation_message("", PathFieldKind::ExistingDirectory),
        Some("Choose a folder.")
    );
    assert_eq!(
        path_validation_message(project.as_str(), PathFieldKind::ExistingDirectory),
        None
    );
    assert_eq!(
        path_validation_message("/tmp/absolute", PathFieldKind::AgentProjectDir),
        Some("Use a project-relative directory.")
    );
    assert_eq!(
        path_validation_message(".codex/skills", PathFieldKind::AgentProjectDir),
        None
    );
}

#[test]
fn inspector_line_presentation_supports_key_values_paths_and_badges() {
    assert_eq!(
        inspector_line_presentation("Skill dir /tmp/project/.agents/skills/example"),
        InspectorLinePresentation {
            label: "Skill dir".to_string(),
            value: "/tmp/project/.agents/skills/example".to_string(),
            kind: InspectorLineKind::Path,
        }
    );
    assert_eq!(
        inspector_line_presentation("Instance ID codex-example-global"),
        InspectorLinePresentation {
            label: "Instance ID".to_string(),
            value: "codex-example-global".to_string(),
            kind: InspectorLineKind::Mono,
        }
    );
    assert_eq!(
        inspector_line_presentation("Status Enabled"),
        InspectorLinePresentation {
            label: "Status".to_string(),
            value: "Enabled".to_string(),
            kind: InspectorLineKind::StatusBadge,
        }
    );
    assert_eq!(
        inspector_line_presentation("Scan Agent Spaces refreshes the Skill Instance Index."),
        InspectorLinePresentation {
            label: String::new(),
            value: "Scan Agent Spaces refreshes the Skill Instance Index.".to_string(),
            kind: InspectorLineKind::Text,
        }
    );
}

#[test]
fn workbench_rows_activate_with_keyboard_enter_or_space() {
    assert!(workbench_row_accepts_keyboard_key(egui::Key::Enter));
    assert!(workbench_row_accepts_keyboard_key(egui::Key::Space));
    assert!(!workbench_row_accepts_keyboard_key(egui::Key::Escape));
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

    assert_eq!(model.dashboard.agent_space_instance_count, 1);
    assert_eq!(model.dashboard.project_agent_space_instance_count, 1);
    assert_eq!(model.recent_projects.len(), 1);
    assert!(model
        .project_summaries
        .iter()
        .all(|summary| summary.native_skill_count == 1));
    assert!(!model
        .project_summaries
        .iter()
        .any(|summary| summary.discovered_unmanaged_count > 0));
}

#[test]
fn dashboard_renders_native_agent_space_health_status() {
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
    assert_eq!(model.dashboard.registry_health, HealthState::Warning);
    assert_eq!(model.dashboard.lock_health, HealthState::Ok);
    assert_eq!(model.dashboard.cache_health, HealthState::Ok);
    assert_eq!(model.dashboard.agent_space_instance_count, 0);

    let renderable = model.renderable_view();
    let scope = renderable
        .inspector_sections
        .iter()
        .find(|section| section.title == "Scope")
        .expect("missing Scope inspector section");
    assert_eq!(
        scope.lines,
        vec![
            "Agent Space instances 0".to_string(),
            "Project Agent Space instances 0".to_string(),
        ]
    );
    let health = renderable
        .inspector_sections
        .iter()
        .find(|section| section.title == "Health")
        .expect("missing Health inspector section");
    assert_eq!(
        health.lines,
        vec![
            "Registry Warning".to_string(),
            "Lock Ok".to_string(),
            "Cache Ok".to_string(),
            "Invalid Agent Space toggles 0".to_string(),
            "Read-only Agent Space instances 0".to_string(),
        ]
    );
}

#[test]
fn dashboard_health_rollup_ignores_legacy_deployment_drift_for_native_counts() {
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

    assert_eq!(model.dashboard.project_agent_space_instance_count, 4);
    assert!(health
        .lines
        .contains(&"Invalid Agent Space toggles 1".to_string()));
    assert!(!health
        .lines
        .iter()
        .any(|line| line.contains("deployments") || line.contains("managed sources")));
}

#[test]
fn projects_view_renders_native_project_skill_instances_and_actions() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    ensure_app_dirs(&paths).unwrap();
    write_config_with_codex_project(&paths, &project);
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();
    let skill_dir = project.join(".agents/skills/project-helper");
    write_skill(&skill_dir, "# Project Helper\n");

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Projects);
    model.select_scope(GuiScope::Project(project.clone()));

    let renderable = model.renderable_view();
    assert_eq!(
        renderable.columns,
        vec!["Skill", "Agent", "Status", "Source", "Writable", "Path"]
    );
    let row = renderable
        .main_rows
        .iter()
        .find(|row| row.cells[0] == "Project Helper")
        .expect("native project row");
    assert_eq!(row.cells[1], "Codex");
    assert_eq!(row.cells[2], "Enabled");
    assert_eq!(row.cells[3], "Project");
    assert_eq!(row.cells[4], "Yes");
    assert_eq!(row.cells[5], skill_dir.to_string());
    assert_eq!(project_actions(&model), vec![ProjectAction::Refresh]);

    let row_id = row.id.clone();
    assert!(model.select_render_row(&row_id));
    assert_eq!(
        section_lines(&model, "Project Skill"),
        vec![
            "Project Helper".to_string(),
            "Agent Codex".to_string(),
            "Status Enabled".to_string(),
            "Source Project".to_string(),
            "Writable Yes".to_string(),
        ]
    );
    assert_eq!(
        project_actions(&model),
        vec![ProjectAction::Refresh, ProjectAction::Disable]
    );
    assert_eq!(
        model.request_disable_selected_skill_instance(),
        Some(GuiActionIntent::DisableSkillInstance {
            instance_id: row_id,
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
    assert_eq!(skill_actions(&model), vec![SkillAction::ScanAgentSpaces]);
    assert_eq!(
        model.request_scan_agent_spaces(),
        Some(GuiActionIntent::ScanAgentSpaces)
    );
    assert_eq!(
        section_lines(&model, "Empty"),
        vec![
            "No Agent Space Skills found.".to_string(),
            "Scan enabled Agent directories.".to_string(),
        ]
    );

    model.navigate(NavigationView::Projects);
    let renderable = model.renderable_view();
    assert!(renderable.main_rows.is_empty());
    assert_eq!(
        renderable.empty_message,
        Some("No project Agent Space Skills in this scope. Scan Agent Spaces or open a project.")
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
fn scan_agent_spaces_refreshes_instances_without_importing_managed_copies() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let home = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
    let mut model = GuiModel::load_with_home_dir(&paths, home.clone()).unwrap();
    model.navigate(NavigationView::Skills);
    assert_eq!(skill_actions(&model), vec![SkillAction::ScanAgentSpaces]);

    write_global_codex_skill(&temp_dir, "late-agent-skill", "# Late Agent Skill\n");
    assert_eq!(
        model.request_scan_agent_spaces(),
        Some(GuiActionIntent::ScanAgentSpaces)
    );
    let controller = GuiController::with_home_dir(paths.clone(), home);
    assert!(model.execute_next_intent(&controller).unwrap().is_some());

    assert_eq!(model.skill_instances.len(), 1);
    assert_eq!(model.skill_instances[0].name, "Late Agent Skill");
    assert!(model.skills.is_empty());
    assert!(read_skills_registry(&paths).unwrap().skills.is_empty());
    assert!(paths.skill_instance_index_file.exists());
    assert_eq!(
        model.last_status().unwrap().message,
        "Scanned Agent Spaces: 1 instance."
    );
}

#[test]
fn import_all_managed_copies_is_explicitly_named_when_used() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    ensure_app_dirs(&paths).unwrap();
    write_config(&paths, &Config::default()).unwrap();
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();

    assert_eq!(
        model.request_import_all_agent_skills_as_managed_copies(),
        Some(GuiActionIntent::ImportAllManagedCopies)
    );
    assert_eq!(
        model.pending_action_status_label(),
        "Next: Import all managed copies (1 queued)"
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
        model.request_import_all_agent_skills_as_managed_copies(),
        Some(GuiActionIntent::ImportAllManagedCopies)
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
        "Imported Agent Skills into Managed Inventory: 2 imported, 1 conflict."
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
                global_skill_dirs: vec![
                    home.join(".codex/skills"),
                    home.join(".codex/plugins/cache"),
                    home.join(".codex/vendor_imports"),
                ],
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
    model
        .request_import_all_agent_skills_as_managed_copies()
        .unwrap();
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
        "Imported Agent Skills into Managed Inventory: 2 imported, 0 conflicts."
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
    model
        .request_import_all_agent_skills_as_managed_copies()
        .unwrap();
    let controller = GuiController::with_home_dir(paths, project_path(&temp_dir, "home"));

    assert!(model.execute_next_intent(&controller).unwrap().is_some());
    assert!(model.skills.is_empty());
    assert_eq!(
        model.last_status().unwrap().message,
        "Imported Agent Skills into Managed Inventory: 0 imported, 0 conflicts."
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
    model
        .request_import_all_agent_skills_as_managed_copies()
        .unwrap();
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
        "Imported Agent Skills into Managed Inventory: 1 imported, 0 conflicts, 1 failure."
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
fn skills_and_project_controls_gate_native_actions_by_selection_and_state() {
    let temp_dir = TempDir::new().unwrap();
    let paths = test_paths(&temp_dir);
    let project = project_path(&temp_dir, "sample-app");
    ensure_app_dirs(&paths).unwrap();
    write_config_with_codex_project(&paths, &project);
    write_skill(
        &project.join(".agents/skills/frontend-design"),
        "# Frontend Design\n",
    );
    write_skills_registry(&paths, &SkillsRegistry::default()).unwrap();
    write_deployments_registry(&paths, &DeploymentsRegistry::default()).unwrap();

    let mut model = GuiModel::load(&paths).unwrap();
    model.navigate(NavigationView::Skills);
    assert_eq!(skill_actions(&model), vec![SkillAction::ScanAgentSpaces]);
    let mut project_model = GuiModel::load(&paths).unwrap();
    project_model.navigate(NavigationView::Projects);
    project_model.select_scope(GuiScope::Project(project.clone()));
    assert_eq!(
        project_actions(&project_model),
        vec![ProjectAction::Refresh]
    );
    let row_id = project_model.renderable_view().main_rows[0].id.clone();
    assert!(project_model.select_render_row(&row_id));
    assert_eq!(
        project_actions(&project_model),
        vec![ProjectAction::Refresh, ProjectAction::Disable]
    );
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
        native_skill_count: 0,
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
        native_skill_count: 0,
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
fn skills_inspector_renders_metadata_and_native_paths_from_loaded_model() {
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
        .find(|instance| instance.skill_dir.file_name() == Some("frontend-design"))
        .expect("native instance")
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

    let paths_section = renderable
        .inspector_sections
        .iter()
        .find(|section| section.title == "Paths")
        .expect("missing Paths inspector section");
    let content_hash = model
        .selected_skill_instance()
        .and_then(|instance| instance.content_hash.clone())
        .expect("content hash");
    assert!(!content_hash.is_empty());
    assert!(paths_section
        .lines
        .iter()
        .any(|line| line.ends_with("/frontend-design")));
    assert!(renderable
        .inspector_sections
        .iter()
        .all(|section| section.title != "Registry Metadata"));
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
