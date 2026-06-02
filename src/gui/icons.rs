use super::{AgentAction, ProjectAction, SkillAction};
use crate::gui::state::NavigationView;

pub const FONT_NAME: &str = "font_awesome_7_free_solid";

pub const DASHBOARD: &str = "\u{f624}";
pub const SKILL: &str = "\u{f72b}";
pub const AGENT: &str = "\u{f544}";
pub const PROJECT: &str = "\u{f802}";
pub const PLUGIN: &str = "\u{f1e6}";

pub const SCAN: &str = "\u{f021}";
pub const REFRESH: &str = "\u{f021}";
pub const ENABLE_SKILL: &str = "\u{f144}";
pub const DISABLE_SKILL: &str = "\u{f28b}";
pub const ENABLE_PLUGIN: &str = "\u{f205}";
pub const DISABLE_PLUGIN: &str = "\u{f204}";
pub const READ_ONLY: &str = "\u{f023}";

pub const STATUS_ENABLED: &str = "\u{f058}";
pub const STATUS_DISABLED: &str = "\u{f056}";
pub const STATUS_INVALID: &str = "\u{f071}";
pub const STATUS_UNKNOWN: &str = "\u{f059}";

pub const ADD: &str = "\u{2b}";
pub const EDIT: &str = "\u{f044}";
pub const RESET: &str = "\u{f2ea}";
pub const REMOVE: &str = "\u{f1f8}";
pub const DEPLOY: &str = "\u{f135}";
pub const REDEPLOY: &str = "\u{f2f1}";
pub const IMPORT: &str = "\u{f56f}";
pub const SKIP: &str = "\u{f05e}";
pub const PROMOTE: &str = "\u{f062}";
pub const COPY: &str = "\u{f0c5}";
pub const REVEAL: &str = "\u{f08e}";
pub const COMMAND: &str = "\u{f120}";
pub const ASSET: &str = "\u{f03e}";
pub const APP: &str = "\u{f2d0}";
pub const CONFIG: &str = "\u{f013}";
pub const PACKAGE: &str = "\u{f187}";
pub const BROWSE: &str = "\u{f07c}";
pub const SAVE: &str = "\u{f0c7}";
pub const CANCEL: &str = "\u{f00d}";

pub fn install_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    install_font_definitions(&mut fonts);
    ctx.set_fonts(fonts);
}

pub fn install_font_definitions(fonts: &mut egui::FontDefinitions) {
    fonts.font_data.insert(
        FONT_NAME.to_string(),
        egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/fontawesome/FontAwesome7Free-Solid-900.otf"
        )),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push(FONT_NAME.to_string());
    }
}

pub fn button_label(icon: &str, label: &str) -> String {
    format!("{icon} {label}")
}

pub fn navigation_icon(view: NavigationView) -> &'static str {
    match view {
        NavigationView::Dashboard => DASHBOARD,
        NavigationView::Skills => SKILL,
        NavigationView::Agents => AGENT,
        NavigationView::Projects => PROJECT,
        NavigationView::Plugins => PLUGIN,
    }
}

pub fn skill_action_icon(action: SkillAction) -> &'static str {
    match action {
        SkillAction::ScanAgentSpaces => SCAN,
        SkillAction::Enable => ENABLE_SKILL,
        SkillAction::Disable => DISABLE_SKILL,
    }
}

pub fn agent_action_icon(action: AgentAction) -> &'static str {
    match action {
        AgentAction::EditSelected => EDIT,
        AgentAction::ResetDefault => RESET,
        AgentAction::RemoveCustom => REMOVE,
        AgentAction::AddCustom => ADD,
    }
}

pub fn project_action_icon(action: ProjectAction) -> &'static str {
    match action {
        ProjectAction::Refresh => REFRESH,
        ProjectAction::AdoptSelected | ProjectAction::AdoptAll | ProjectAction::ImportAsNew => {
            IMPORT
        }
        ProjectAction::Skip => SKIP,
        ProjectAction::Deploy => DEPLOY,
        ProjectAction::Enable => ENABLE_SKILL,
        ProjectAction::Disable => DISABLE_SKILL,
        ProjectAction::Redeploy => REDEPLOY,
        ProjectAction::Overwrite => EDIT,
        ProjectAction::Promote => PROMOTE,
        ProjectAction::Remove => REMOVE,
    }
}

pub fn status_icon(value: &str) -> &'static str {
    match value {
        "Enabled" | "Ready" | "Yes" => STATUS_ENABLED,
        "Disabled" | "No" => STATUS_DISABLED,
        "Read-only" => READ_ONLY,
        "Invalid" | "Missing" | "Missing managed source" => STATUS_INVALID,
        value if value.contains("Invalid") || value.contains("Missing") => STATUS_INVALID,
        value if value.contains("Outdated") || value.contains("Drift") => STATUS_INVALID,
        _ => STATUS_UNKNOWN,
    }
}
