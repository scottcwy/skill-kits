use camino::{Utf8Path, Utf8PathBuf};
use skill_kits::core::{
    adopt::{global_agent_adopt, AdoptReport, GlobalAgentAdoptRequest},
    hash::hash_skill_dir,
    ids::{AgentId, SkillId},
    install::{uninstall_skill, UninstallSkillRequest},
    paths::AppPaths,
    registry::{
        read_deployments_registry, read_skills_registry, write_deployments_registry,
        write_skills_registry, DeploymentRecord, DeploymentsRegistry, ManagedSkill, SkillSource,
    },
};
use tempfile::TempDir;

fn utf8(path: impl AsRef<std::path::Path>) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(path.as_ref().to_path_buf()).unwrap()
}

fn write_file(path: &Utf8Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

fn write_skill(root: &Utf8Path, body: &str) {
    write_file(&root.join("SKILL.md"), body);
}

fn write_disabled_skill(root: &Utf8Path, body: &str) {
    write_file(&root.join("SKILL.md.disabled"), body);
}

fn test_paths(temp_dir: &TempDir) -> AppPaths {
    AppPaths::from_data_root(utf8(temp_dir.path()).join(".skill-kits"))
}

fn seed_managed_skill(paths: &AppPaths, id: &str, name: &str, body: &str) -> ManagedSkill {
    let managed_path = paths.skills_dir.join(id);
    write_skill(&managed_path, body);
    let content_hash = hash_skill_dir(&managed_path).unwrap();
    let skill = ManagedSkill {
        id: SkillId::new(id),
        name: name.to_string(),
        source: SkillSource::Local {
            source_path: managed_path.clone(),
        },
        managed_path,
        content_hash,
        metadata: None,
        created_at: "2026-05-31T00:00:00Z".to_string(),
        updated_at: "2026-05-31T00:00:00Z".to_string(),
    };
    let mut registry = read_skills_registry(paths).unwrap();
    registry.skills.push(skill.clone());
    write_skills_registry(paths, &registry).unwrap();
    skill
}

#[test]
fn global_agent_adopt_imports_valid_immediate_children_and_never_writes_source() {
    let temp_dir = TempDir::new().unwrap();
    let root = utf8(temp_dir.path());
    let paths = test_paths(&temp_dir);
    let global_root = root.join(".codex/skills");
    write_skill(&global_root.join("fresh"), "# Fresh\n");
    write_disabled_skill(&global_root.join("disabled"), "# Disabled\n");
    std::fs::create_dir_all(global_root.join("empty")).unwrap();
    write_skill(
        &global_root.join("nested").join("child"),
        "# Nested child\n",
    );

    let report = global_agent_adopt(GlobalAgentAdoptRequest {
        app_paths: &paths,
        agent_id: &AgentId::new("codex"),
        home_dir: &root,
    })
    .unwrap();

    assert_eq!(
        report,
        AdoptReport {
            imported: 2,
            conflicts: 0
        }
    );
    let registry = read_skills_registry(&paths).unwrap();
    let names: Vec<_> = registry
        .skills
        .iter()
        .map(|skill| skill.name.as_str())
        .collect();
    assert_eq!(names, vec!["disabled", "fresh"]);
    assert!(registry
        .skills
        .iter()
        .all(|skill| matches!(skill.source, SkillSource::GlobalAgentAdopt { .. })));
    assert!(!global_root.join("fresh/SKILL.md.disabled").exists());
    assert!(global_root.join("disabled/SKILL.md.disabled").exists());
    assert!(!registry.skills.iter().any(|skill| skill.name == "nested"));
}

#[test]
fn global_agent_adopt_skips_same_name_same_hash_and_reports_same_name_conflicts() {
    let temp_dir = TempDir::new().unwrap();
    let root = utf8(temp_dir.path());
    let paths = test_paths(&temp_dir);
    let global_root = root.join(".codex/skills");
    let existing = seed_managed_skill(&paths, "existing-a1b2c3d4", "existing", "# Same\n");
    write_skill(&global_root.join("existing"), "# Same\n");
    write_skill(&global_root.join("conflict"), "# Different source\n");
    seed_managed_skill(
        &paths,
        "conflict-a1b2c3d4",
        "conflict",
        "# Managed source\n",
    );

    let report = global_agent_adopt(GlobalAgentAdoptRequest {
        app_paths: &paths,
        agent_id: &AgentId::new("codex"),
        home_dir: &root,
    })
    .unwrap();

    assert_eq!(
        report,
        AdoptReport {
            imported: 0,
            conflicts: 1
        }
    );
    let registry = read_skills_registry(&paths).unwrap();
    assert_eq!(registry.skills.len(), 2);
    assert!(registry
        .skills
        .iter()
        .any(|skill| skill.id == existing.id && skill.name == "existing"));
}

#[test]
fn uninstall_removes_only_managed_skill_and_registry_entry() {
    let temp_dir = TempDir::new().unwrap();
    let root = utf8(temp_dir.path());
    let paths = test_paths(&temp_dir);
    let skill = seed_managed_skill(
        &paths,
        "frontend-design-a1b2c3d4",
        "frontend-design",
        "# Skill\n",
    );
    let project_copy = root.join("project/.agents/skills/frontend-design");
    write_skill(&project_copy, "# Project copy\n");
    let deployment = DeploymentRecord {
        id: "deployment-1".to_string(),
        skill_id: skill.id.clone(),
        agent_id: AgentId::new("codex"),
        project_name: "project".to_string(),
        project_path: root.join("project"),
        deployment_path: project_copy.clone(),
        skill_name: skill.name.clone(),
        baseline_hash: "baseline".to_string(),
        deployed_from_hash: skill.content_hash.clone(),
        created_at: "2026-05-31T00:00:00Z".to_string(),
        updated_at: "2026-05-31T00:00:00Z".to_string(),
    };
    write_deployments_registry(
        &paths,
        &DeploymentsRegistry {
            version: 1,
            deployments: vec![deployment.clone()],
        },
    )
    .unwrap();

    let result = uninstall_skill(UninstallSkillRequest {
        app_paths: &paths,
        query: "frontend-design",
    })
    .unwrap();

    assert_eq!(result.skill_id, skill.id);
    assert!(!skill.managed_path.exists());
    assert!(read_skills_registry(&paths).unwrap().skills.is_empty());
    assert_eq!(
        read_deployments_registry(&paths).unwrap().deployments,
        vec![deployment]
    );
    assert!(project_copy.join("SKILL.md").exists());
}
