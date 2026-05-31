use skill_kits::core::{
    agents::{AgentConfig, AgentKind},
    config::Config,
    ids::{AgentId, ProjectId, SkillId},
    registry::{DeploymentRecord, DeploymentsRegistry, ManagedSkill, SkillsRegistry},
    scan::{RiskFinding, RiskSeverity},
    DeploymentStatus, SkillKitsError, SkillSource, ToggleState,
};

#[test]
fn public_core_contract_is_available_for_parallel_agents() {
    let skill_id = SkillId::new("frontend-design-a1b2c3d4");
    let agent_id = AgentId::new("codex");
    let project_id = ProjectId::new("my-app");

    assert_eq!(skill_id.as_str(), "frontend-design-a1b2c3d4");
    assert_eq!(agent_id.as_str(), "codex");
    assert_eq!(project_id.as_str(), "my-app");

    let config = Config::default();
    assert!(config
        .agents
        .iter()
        .any(|agent| agent.id.as_str() == "codex"));
    assert_eq!(SkillsRegistry::default().version, 1);
    assert_eq!(DeploymentsRegistry::default().version, 1);

    let custom = AgentConfig {
        id: AgentId::new("custom"),
        label: "Custom".to_string(),
        kind: AgentKind::Custom,
        global_skill_dirs: vec![],
        project_skill_dirs: vec![".custom/skills".into()],
        enabled: true,
    };
    assert_eq!(custom.kind, AgentKind::Custom);

    let source = SkillSource::Local {
        source_path: "/tmp/source".into(),
    };
    let skill = ManagedSkill {
        id: skill_id.clone(),
        name: "frontend-design".to_string(),
        source,
        managed_path: "/tmp/managed".into(),
        content_hash: "hash".to_string(),
        metadata: None,
        created_at: "2026-05-31T00:00:00Z".to_string(),
        updated_at: "2026-05-31T00:00:00Z".to_string(),
    };
    assert_eq!(skill.id, skill_id);

    let record = DeploymentRecord {
        id: "deployment".to_string(),
        skill_id: skill.id.clone(),
        agent_id,
        project_name: "my-app".to_string(),
        project_path: "/tmp/project".into(),
        deployment_path: "/tmp/project/.agents/skills/frontend-design".into(),
        skill_name: "frontend-design".to_string(),
        baseline_hash: "base".to_string(),
        deployed_from_hash: "hash".to_string(),
        created_at: "2026-05-31T00:00:00Z".to_string(),
        updated_at: "2026-05-31T00:00:00Z".to_string(),
    };
    let status = DeploymentStatus {
        record,
        toggle: ToggleState::Enabled,
        current_hash: Some("base".to_string()),
        drift: false,
        outdated: false,
        missing_managed_source: false,
    };
    assert_eq!(status.toggle, ToggleState::Enabled);

    let finding = RiskFinding {
        severity: RiskSeverity::High,
        rule_id: "remote-shell-pipe".to_string(),
        path: "SKILL.md".into(),
        line: Some(1),
        message: "network pipe to shell".to_string(),
    };
    assert_eq!(finding.severity, RiskSeverity::High);

    let error = SkillKitsError::RegistryBusy;
    assert_eq!(error.exit_code(), 4);
}
