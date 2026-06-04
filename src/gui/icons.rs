use super::{AgentAction, ProjectAction, SkillAction};
use crate::gui::state::NavigationView;

pub const FONT_NAME: &str = "font_awesome_7_free_solid";
pub const UI_FONT_NAME: &str = "geist_regular";
pub const MONO_FONT_NAME: &str = "geist_mono_regular";
pub const CJK_FONT_NAME: &str = "skill_kits_cjk_fallback";
pub const CJK_FONT_Y_OFFSET_FACTOR: f32 = 0.30;

pub const DASHBOARD: &str = "\u{f624}";
pub const SKILL: &str = "\u{f72b}";
pub const AGENT: &str = "\u{f544}";
pub const PROJECT: &str = BROWSE;
pub const PLUGIN: &str = "\u{f1e6}";

pub const SCAN: &str = "\u{f021}";
pub const REFRESH: &str = "\u{f021}";
pub const BACK: &str = "\u{f060}";
pub const FORWARD: &str = "\u{f061}";
pub const CHEVRON_RIGHT: &str = "\u{f054}";
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
        UI_FONT_NAME.to_string(),
        egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/geist/Geist-Regular.ttf"
        )),
    );
    fonts.font_data.insert(
        MONO_FONT_NAME.to_string(),
        egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/geist/GeistMono-Regular.ttf"
        )),
    );
    if let Some(font_data) = load_cjk_fallback_font() {
        fonts
            .font_data
            .insert(CJK_FONT_NAME.to_string(), font_data);
    }
    fonts.font_data.insert(
        FONT_NAME.to_string(),
        egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/fontawesome/FontAwesome7Free-Solid-900.otf"
        )),
    );
    for (family, primary_font) in [
        (egui::FontFamily::Proportional, UI_FONT_NAME),
        (egui::FontFamily::Monospace, MONO_FONT_NAME),
    ] {
        let family_fonts = fonts.families.entry(family).or_default();
        if !family_fonts.iter().any(|name| name == primary_font) {
            family_fonts.insert(0, primary_font.to_string());
        }
        if fonts.font_data.contains_key(CJK_FONT_NAME)
            && !family_fonts.iter().any(|name| name == CJK_FONT_NAME)
        {
            family_fonts.push(CJK_FONT_NAME.to_string());
        }
        family_fonts.push(FONT_NAME.to_string());
    }
}

fn load_cjk_fallback_font() -> Option<egui::FontData> {
    for path in CJK_FONT_CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            return Some(egui::FontData::from_owned(bytes).tweak(egui::FontTweak {
                scale: 1.0,
                y_offset_factor: CJK_FONT_Y_OFFSET_FACTOR,
                y_offset: 0.0,
                baseline_offset_factor: 0.0,
            }));
        }
    }
    None
}

const CJK_FONT_CANDIDATES: &[&str] = &[
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/STHeiti Light.ttc",
    "/System/Library/Fonts/STHeiti Medium.ttc",
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/noto/NotoSansCJKsc-Regular.otf",
    "C:\\Windows\\Fonts\\msyh.ttc",
    "C:\\Windows\\Fonts\\simsun.ttc",
];

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
