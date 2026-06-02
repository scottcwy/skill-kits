pub mod agents;
pub mod dashboard;
pub mod plugins;
pub mod projects;
pub mod skills;
pub mod state;

use crate::core::paths::AppPaths;
use crate::core::ToggleState;
use camino::Utf8Path;
use eframe::egui;
use state::{
    AgentEditorMode, GuiController, GuiModel, GuiScope, GuiStatusKind, NavigationView, RenderRow,
    RenderableView, UiColors, DRIFT_REMOVE_CONFIRMATION_MESSAGE,
    SKILL_INSTANCE_DISABLE_CONFIRMATION_MESSAGE,
};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::thread;

const MAIN_ROW_HEIGHT: f32 = 34.0;
const INSPECTOR_CONTROLS_HEIGHT: f32 = 184.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillAction {
    ScanAgentSpaces,
    Enable,
    Disable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PluginAction {
    ScanPlugins,
    Enable,
    Disable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectAction {
    Refresh,
    AdoptSelected,
    AdoptAll,
    ImportAsNew,
    Skip,
    Deploy,
    Enable,
    Disable,
    Redeploy,
    Overwrite,
    Promote,
    Remove,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentAction {
    EditSelected,
    ResetDefault,
    RemoveCustom,
    AddCustom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkbenchCellStyle {
    Text,
    Mono,
    StatusBadge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathFieldKind {
    ExistingDirectory,
    AgentProjectDir,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectorLinePresentation {
    pub label: String,
    pub value: String,
    pub kind: InspectorLineKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectorLineKind {
    Text,
    Mono,
    Path,
    StatusBadge,
}

impl AgentAction {
    const EMPTY: [Self; 1] = [Self::AddCustom];

    fn label(self) -> &'static str {
        match self {
            Self::EditSelected => "Edit path",
            Self::ResetDefault => "Reset default",
            Self::RemoveCustom => "Remove custom",
            Self::AddCustom => "Add custom",
        }
    }
}

impl ProjectAction {
    const REFRESH: [Self; 1] = [Self::Refresh];

    fn label(self) -> &'static str {
        match self {
            Self::Refresh => "Refresh",
            Self::AdoptSelected => "Adopt selected",
            Self::AdoptAll => "Adopt all",
            Self::ImportAsNew => "Import as new",
            Self::Skip => "Skip",
            Self::Deploy => "Deploy",
            Self::Enable => "Enable",
            Self::Disable => "Disable",
            Self::Redeploy => "Redeploy",
            Self::Overwrite => "Overwrite",
            Self::Promote => "Promote",
            Self::Remove => "Remove",
        }
    }
}

impl SkillAction {
    fn label(self) -> &'static str {
        match self {
            Self::ScanAgentSpaces => "Scan Agent Spaces",
            Self::Enable => "Enable",
            Self::Disable => "Disable",
        }
    }
}

impl PluginAction {
    fn label(self) -> &'static str {
        match self {
            Self::ScanPlugins => "Scan Plugins",
            Self::Enable => "Enable Plugin",
            Self::Disable => "Disable Plugin",
        }
    }
}

pub fn skill_actions(model: &GuiModel) -> Vec<SkillAction> {
    let mut actions = vec![SkillAction::ScanAgentSpaces];
    if model.selected_skill_instance().is_none() {
        return actions;
    }

    if let Some(instance) = model.selected_skill_instance() {
        if instance.writable {
            match instance.toggle_state {
                ToggleState::Enabled => actions.push(SkillAction::Disable),
                ToggleState::Disabled => actions.push(SkillAction::Enable),
                ToggleState::InvalidBothPresent | ToggleState::InvalidBothMissing => {}
            }
        }
    }
    actions
}

pub fn agent_actions(model: &GuiModel) -> Vec<AgentAction> {
    let Some(agent) = model.selected_agent() else {
        return AgentAction::EMPTY.to_vec();
    };

    let mut actions = vec![AgentAction::EditSelected];
    match agent.kind {
        crate::core::agents::AgentKind::BuiltIn => actions.push(AgentAction::ResetDefault),
        crate::core::agents::AgentKind::Custom => actions.push(AgentAction::RemoveCustom),
    }
    actions.push(AgentAction::AddCustom);
    actions
}

pub fn project_actions(model: &GuiModel) -> Vec<ProjectAction> {
    let mut actions = ProjectAction::REFRESH.to_vec();
    if let Some(instance) = model.selected_skill_instance() {
        if instance.writable {
            match instance.toggle_state {
                ToggleState::Enabled => actions.push(ProjectAction::Disable),
                ToggleState::Disabled => actions.push(ProjectAction::Enable),
                ToggleState::InvalidBothPresent | ToggleState::InvalidBothMissing => {}
            }
        }
    }
    actions
}

pub fn plugin_actions(model: &GuiModel) -> Vec<PluginAction> {
    let mut actions = vec![PluginAction::ScanPlugins];
    if model.selected_plugin_capability().is_some() {
        return actions;
    }
    let Some(plugin) = model.selected_plugin() else {
        return actions;
    };
    if plugin.can_toggle {
        match plugin.status {
            crate::core::plugins::PluginStatus::Enabled => actions.push(PluginAction::Disable),
            crate::core::plugins::PluginStatus::Disabled => actions.push(PluginAction::Enable),
            crate::core::plugins::PluginStatus::Unknown
            | crate::core::plugins::PluginStatus::Invalid => {}
        }
    }
    actions
}

pub fn workbench_cell_style(column: &str) -> WorkbenchCellStyle {
    match column {
        "Status" | "Toggle" | "Validation" | "Enabled" | "Writable" | "Managed" | "Source"
        | "Outdated" | "Drift" | "Risk" | "Read-only" => WorkbenchCellStyle::StatusBadge,
        "Path"
        | "Project skill directories"
        | "Skill ID"
        | "Instance ID"
        | "Plugin ID"
        | "Plugin Key"
        | "Capability ID"
        | "Hash"
        | "Command" => WorkbenchCellStyle::Mono,
        _ => WorkbenchCellStyle::Text,
    }
}

pub fn workbench_row_accepts_keyboard_key(key: egui::Key) -> bool {
    matches!(key, egui::Key::Enter | egui::Key::Space)
}

pub fn path_validation_message(value: &str, kind: PathFieldKind) -> Option<&'static str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("Choose a folder.");
    }

    match kind {
        PathFieldKind::ExistingDirectory => {
            if Utf8Path::new(trimmed).is_dir() {
                None
            } else {
                Some("Folder does not exist.")
            }
        }
        PathFieldKind::AgentProjectDir => {
            if Utf8Path::new(trimmed).is_absolute() {
                Some("Use a project-relative directory.")
            } else {
                None
            }
        }
    }
}

pub fn inspector_line_presentation(line: &str) -> InspectorLinePresentation {
    const LABELS: &[&str] = &[
        "Missing managed source",
        "Project dir",
        "Project path",
        "Project Agent Space instances",
        "Agent Space instances",
        "Instance ID",
        "Agent id",
        "Skill dir",
        "Enabled",
        "Disabled",
        "Content hash",
        "Scanned hash",
        "Registry",
        "Status",
        "Source",
        "Writable",
        "Managed",
        "Agent",
        "Scope",
        "Kind",
        "Title",
        "Description",
        "Plugin id",
        "Plugin key",
        "Capability id",
        "Config path",
        "Package path",
        "Manifest path",
        "Lock",
        "Cache",
    ];

    for label in LABELS {
        if let Some(value) = line.strip_prefix(&format!("{label} ")) {
            return InspectorLinePresentation {
                label: (*label).to_string(),
                value: value.to_string(),
                kind: inspector_line_kind(label, value),
            };
        }
    }

    InspectorLinePresentation {
        label: String::new(),
        value: line.to_string(),
        kind: InspectorLineKind::Text,
    }
}

fn inspector_line_kind(label: &str, value: &str) -> InspectorLineKind {
    match label {
        "Skill dir" | "Enabled" | "Disabled" | "Project dir" | "Project path" | "Config path"
        | "Package path" | "Manifest path" => InspectorLineKind::Path,
        "Instance ID" | "Agent id" | "Content hash" | "Scanned hash" | "Plugin id"
        | "Plugin key" | "Capability id" => InspectorLineKind::Mono,
        "Status"
        | "Writable"
        | "Managed"
        | "Registry"
        | "Lock"
        | "Cache"
        | "Missing managed source" => InspectorLineKind::StatusBadge,
        _ if value.starts_with('/') || value.starts_with("~/") => InspectorLineKind::Path,
        _ => InspectorLineKind::Text,
    }
}

pub struct SkillKitsGuiApp {
    model: GuiModel,
    controller: GuiController,
    colors: UiColors,
    running_intent: Option<RunningIntent>,
}

struct RunningIntent {
    label: &'static str,
    intent: state::GuiActionIntent,
    receiver: Receiver<GuiModel>,
}

impl SkillKitsGuiApp {
    pub fn from_paths(paths: &AppPaths) -> crate::core::Result<Self> {
        Ok(Self::new(
            GuiModel::load(paths)?,
            GuiController::new(paths.clone()),
        ))
    }

    pub fn new(model: GuiModel, controller: GuiController) -> Self {
        Self {
            model,
            controller,
            colors: UiColors::dark(),
            running_intent: None,
        }
    }

    pub fn model(&self) -> &GuiModel {
        &self.model
    }

    pub fn model_mut(&mut self) -> &mut GuiModel {
        &mut self.model
    }

    pub fn has_running_intent(&self) -> bool {
        self.running_intent.is_some()
    }

    pub fn action_status_label(&self) -> String {
        if let Some(running) = &self.running_intent {
            return format!(
                "Working: {} ({} queued)",
                running.label,
                self.model.pending_intents().len()
            );
        }
        self.model.pending_action_status_label()
    }
}

pub fn run_native(paths: AppPaths) -> anyhow::Result<()> {
    eframe::run_native(
        "Skill-kits",
        native_options(),
        Box::new(move |_cc| Ok(Box::new(SkillKitsGuiApp::from_paths(&paths)?))),
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))
}

pub fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([960.0, 620.0])
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false)
            .with_titlebar_buttons_shown(true),
        ..Default::default()
    }
}

impl eframe::App for SkillKitsGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_dark_theme(ctx, self.colors);
        if self.collect_finished_intent() {
            ctx.request_repaint();
        }
        self.dispatch_one_pending_intent();
        if self.has_running_intent() {
            ctx.request_repaint();
        }

        egui::TopBottomPanel::top("top_bar")
            .frame(egui::Frame::none().fill(self.colors.surface_1))
            .exact_height(42.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(egui::RichText::new("Skill-kits").strong());
                    ui.separator();
                    ui.label(scope_label(&self.model.active_scope));
                    if ui.button("Refresh").clicked() {
                        let _ = self.model.request_refresh_selected_project();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(self.action_status_label());
                    });
                });
                if let Some(status) = self.model.last_status() {
                    let color = match status.kind {
                        GuiStatusKind::Success => self.colors.success,
                        GuiStatusKind::Error => self.colors.danger,
                    };
                    ui.separator();
                    ui.label(egui::RichText::new(&status.message).color(color));
                }
            });

        egui::SidePanel::left("sidebar")
            .frame(egui::Frame::none().fill(self.colors.surface_1))
            .resizable(false)
            .exact_width(204.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                for view in NavigationView::ORDER {
                    if ui
                        .selectable_label(self.model.active_view == view, view.title())
                        .clicked()
                    {
                        self.model.navigate(view);
                    }
                }
                ui.add_space(12.0);
                ui.separator();
                ui.label(egui::RichText::new("Scope").color(self.colors.ink_subtle));
                if ui
                    .selectable_label(
                        matches!(self.model.active_scope, GuiScope::GlobalInventory),
                        "Managed Inventory",
                    )
                    .clicked()
                {
                    self.model.select_scope(GuiScope::GlobalInventory);
                }
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Recent Projects").color(self.colors.ink_subtle));
                let projects = self.model.recent_projects.clone();
                egui::ScrollArea::vertical()
                    .id_salt("sidebar_recent_projects")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for project in projects {
                            let selected = matches!(
                                &self.model.active_scope,
                                GuiScope::Project(path) if path == &project.path
                            );
                            let response = ui
                                .selectable_label(selected, &project.name)
                                .on_hover_text(project.path.to_string());
                            if response.clicked() {
                                self.model.select_scope(GuiScope::Project(project.path));
                            }
                        }
                    });
            });

        egui::SidePanel::right("inspector")
            .frame(egui::Frame::none().fill(self.colors.surface_1))
            .resizable(false)
            .exact_width(344.0)
            .show(ctx, |ui| {
                let renderable = self.model.renderable_view();
                let controls_height = INSPECTOR_CONTROLS_HEIGHT
                    .min(ui.available_height() * 0.45)
                    .max(112.0);
                let inspector_height = (ui.available_height() - controls_height).max(96.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), inspector_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("inspector_scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                render_inspector(ui, &renderable, self.colors);
                            });
                    },
                );
                render_action_controls(ui, &mut self.model, self.colors);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(self.colors.canvas))
            .show(ctx, |ui| {
                let renderable = self.model.renderable_view();
                render_main(ui, &mut self.model, &renderable, self.colors);
            });
    }
}

impl SkillKitsGuiApp {
    pub fn execute_one_pending_intent(&mut self) {
        let _ = self.model.execute_next_intent(&self.controller);
    }

    pub fn dispatch_one_pending_intent(&mut self) {
        if self.running_intent.is_some() || self.model.pending_intents().is_empty() {
            return;
        }
        let label = self.model.next_action_label().unwrap_or("Action");
        let intent = self.model.pending_intents()[0].clone();
        let mut model = self.model.clone();
        let controller = self.controller.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = model.execute_next_intent(&controller);
            let _ = sender.send(model);
        });
        self.running_intent = Some(RunningIntent {
            label,
            intent,
            receiver,
        });
    }

    pub fn collect_finished_intent(&mut self) -> bool {
        let Some(running) = &self.running_intent else {
            return false;
        };
        let model = match running.receiver.try_recv() {
            Ok(model) => model,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.running_intent = None;
                return true;
            }
        };
        let running = self.running_intent.take().expect("running intent exists");
        let queued_during_run = self.model.take_pending_intents();
        self.model = model;
        self.model
            .append_pending_intents(remove_running_intent(queued_during_run, &running.intent));
        true
    }
}

fn remove_running_intent(
    mut intents: Vec<state::GuiActionIntent>,
    running: &state::GuiActionIntent,
) -> Vec<state::GuiActionIntent> {
    if let Some(index) = intents.iter().position(|intent| intent == running) {
        intents.remove(index);
    }
    intents
}

fn apply_dark_theme(ctx: &egui::Context, colors: UiColors) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = colors.canvas;
    visuals.window_fill = colors.surface_1;
    visuals.faint_bg_color = colors.surface_2;
    visuals.extreme_bg_color = colors.canvas;
    visuals.selection.bg_fill = colors.surface_3;
    visuals.selection.stroke = egui::Stroke::new(1.0, colors.focus);
    visuals.widgets.inactive.bg_fill = colors.surface_1;
    visuals.widgets.hovered.bg_fill = colors.surface_2;
    visuals.widgets.active.bg_fill = colors.surface_3;
    visuals.widgets.noninteractive.bg_fill = colors.surface_1;
    visuals.widgets.inactive.fg_stroke.color = colors.ink_muted;
    visuals.widgets.hovered.fg_stroke.color = colors.ink;
    visuals.widgets.active.fg_stroke.color = colors.ink;
    visuals.hyperlink_color = colors.ink_muted;
    ctx.set_visuals(visuals);
}

fn render_main(
    ui: &mut egui::Ui,
    model: &mut GuiModel,
    renderable: &RenderableView,
    colors: UiColors,
) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.heading(egui::RichText::new(&renderable.title).size(20.0));
    });
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);
    if matches!(renderable.view, NavigationView::Skills) {
        render_skill_filters(ui, model, colors);
        ui.add_space(8.0);
    }

    if renderable.main_rows.is_empty() {
        ui.add_space(20.0);
        ui.label(
            egui::RichText::new(renderable.empty_message.unwrap_or("No rows"))
                .color(colors.ink_subtle),
        );
    } else {
        render_workbench_table(ui, model, renderable, colors);
    }
}

fn render_workbench_table(
    ui: &mut egui::Ui,
    model: &mut GuiModel,
    renderable: &RenderableView,
    colors: UiColors,
) {
    egui::ScrollArea::horizontal()
        .id_salt(format!("main_table_horizontal_{:?}", renderable.view))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let widths = table_column_widths(&renderable.columns);
            let table_width: f32 = widths.iter().sum::<f32>() + 12.0;
            render_table_header(ui, &renderable.columns, &widths, colors);
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt(format!("main_table_vertical_{:?}", renderable.view))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for row in &renderable.main_rows {
                        render_table_row(ui, model, renderable, row, &widths, table_width, colors);
                    }
                });
        });
}

fn render_table_header(ui: &mut egui::Ui, columns: &[String], widths: &[f32], colors: UiColors) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        for (column, width) in columns.iter().zip(widths.iter()) {
            ui.add_sized(
                [*width, 22.0],
                egui::Label::new(
                    egui::RichText::new(column)
                        .color(colors.ink_subtle)
                        .strong(),
                )
                .truncate(),
            )
            .on_hover_text(column);
        }
    });
}

fn render_table_row(
    ui: &mut egui::Ui,
    model: &mut GuiModel,
    renderable: &RenderableView,
    row: &RenderRow,
    widths: &[f32],
    table_width: f32,
    colors: UiColors,
) {
    let selected = is_render_row_selected(model, renderable.view, &row.id);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(table_width, MAIN_ROW_HEIGHT),
        egui::Sense::click(),
    );
    let fill = if selected {
        colors.surface_3
    } else if response.hovered() || response.has_focus() {
        colors.surface_2
    } else {
        egui::Color32::TRANSPARENT
    };
    if fill != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::Rounding::same(5.0), fill);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            egui::Rounding::same(5.0),
            egui::Stroke::new(1.0, colors.focus),
        );
    }

    let mut row_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(8.0, 0.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    for ((cell, column), width) in row
        .cells
        .iter()
        .zip(renderable.columns.iter())
        .zip(widths.iter())
    {
        render_table_cell(&mut row_ui, cell, column, *width, colors);
    }

    let keyboard_activated = response.has_focus()
        && ui.input(|input| {
            [egui::Key::Enter, egui::Key::Space]
                .into_iter()
                .any(|key| workbench_row_accepts_keyboard_key(key) && input.key_pressed(key))
        });
    if response.clicked() || keyboard_activated {
        response.request_focus();
        model.select_render_row(&row.id);
    }
    response.on_hover_text("Select row");
}

fn render_table_cell(ui: &mut egui::Ui, cell: &str, column: &str, width: f32, colors: UiColors) {
    match workbench_cell_style(column) {
        WorkbenchCellStyle::StatusBadge => {
            ui.allocate_ui_with_layout(
                egui::vec2(width, MAIN_ROW_HEIGHT),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    render_status_badge(ui, cell, colors).on_hover_text(cell);
                },
            );
        }
        WorkbenchCellStyle::Mono => {
            let response = ui.add_sized(
                [width, MAIN_ROW_HEIGHT],
                egui::Label::new(
                    egui::RichText::new(cell)
                        .monospace()
                        .size(12.0)
                        .color(colors.ink_subtle),
                )
                .truncate(),
            );
            response.on_hover_text(cell);
        }
        WorkbenchCellStyle::Text => {
            let response = ui.add_sized(
                [width, MAIN_ROW_HEIGHT],
                egui::Label::new(egui::RichText::new(cell).color(colors.ink_muted)).truncate(),
            );
            response.on_hover_text(cell);
        }
    }
}

fn render_status_badge(ui: &mut egui::Ui, value: &str, colors: UiColors) -> egui::Response {
    let text_color = status_color(value, colors);
    egui::Frame::none()
        .fill(colors.surface_2)
        .rounding(egui::Rounding::same(3.0))
        .inner_margin(egui::Margin::symmetric(7.0, 2.0))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let dot = egui::RichText::new("•").color(text_color);
                ui.label(dot);
                ui.label(
                    egui::RichText::new(value)
                        .color(colors.ink_muted)
                        .size(12.0),
                );
            });
        })
        .response
}

fn status_color(value: &str, colors: UiColors) -> egui::Color32 {
    match value {
        "Enabled" | "Ready" | "Yes" => colors.success,
        "Invalid" | "Missing" | "No" => colors.danger,
        "Disabled" | "Read-only" | "Not configured" => colors.info,
        value if value.contains("Invalid") || value.contains("Missing") => colors.danger,
        value if value.contains("Outdated") || value.contains("Drift") => colors.warning,
        _ => colors.info,
    }
}

fn table_column_widths(columns: &[String]) -> Vec<f32> {
    columns
        .iter()
        .map(|column| match column.as_str() {
            "Skill" | "Agent" => 156.0,
            "Project skill directories" | "Path" => 280.0,
            "Scope" | "Source" => 144.0,
            "Status" | "Toggle" | "Enabled" | "Writable" | "Managed" | "Validation" => 112.0,
            "Updated" | "Outdated" | "Drift" | "Risk" => 120.0,
            _ => 132.0,
        })
        .collect()
}

fn is_render_row_selected(model: &GuiModel, view: NavigationView, row_id: &str) -> bool {
    match view {
        NavigationView::Dashboard => false,
        NavigationView::Skills => model
            .selected_skill_instance()
            .is_some_and(|instance| instance.id == row_id),
        NavigationView::Agents => model
            .selected_agent()
            .is_some_and(|agent| agent.id.as_str() == row_id),
        NavigationView::Projects => {
            model
                .selected_skill_instance()
                .is_some_and(|instance| instance.id == row_id)
                || model
                    .selected_deployment()
                    .is_some_and(|deployment| deployment.id == row_id)
        }
        NavigationView::Plugins => {
            model
                .selected_plugin()
                .is_some_and(|plugin| plugin.id == row_id)
                || model
                    .selected_plugin_capability()
                    .is_some_and(|capability| capability.id == row_id)
        }
    }
}

fn render_skill_filters(ui: &mut egui::Ui, model: &mut GuiModel, colors: UiColors) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Agent").color(colors.ink_subtle));
        let agent_text = model
            .skill_agent_filter()
            .and_then(|selected| {
                model
                    .skill_agent_filter_options()
                    .into_iter()
                    .find(|(agent_id, _)| agent_id == selected)
                    .map(|(_, label)| label)
            })
            .unwrap_or_else(|| "All".to_string());
        egui::ComboBox::from_id_salt("skill_agent_filter")
            .selected_text(agent_text)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(model.skill_agent_filter().is_none(), "All")
                    .clicked()
                {
                    model.set_skill_agent_filter(None);
                }
                for (agent_id, label) in model.skill_agent_filter_options() {
                    if ui
                        .selectable_label(
                            model.skill_agent_filter() == Some(&agent_id),
                            label.as_str(),
                        )
                        .clicked()
                    {
                        model.set_skill_agent_filter(Some(agent_id));
                    }
                }
            });

        ui.label(egui::RichText::new("Scope").color(colors.ink_subtle));
        let scope_text = model.skill_scope_filter().unwrap_or("All").to_string();
        egui::ComboBox::from_id_salt("skill_scope_filter")
            .selected_text(scope_text)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(model.skill_scope_filter().is_none(), "All")
                    .clicked()
                {
                    model.set_skill_scope_filter(None);
                }
                for scope in model.skill_scope_filter_options() {
                    if ui
                        .selectable_label(
                            model.skill_scope_filter() == Some(scope.as_str()),
                            &scope,
                        )
                        .clicked()
                    {
                        model.set_skill_scope_filter(Some(scope));
                    }
                }
            });

        ui.label(egui::RichText::new("Status").color(colors.ink_subtle));
        let status_text = model.skill_status_filter().unwrap_or("All").to_string();
        egui::ComboBox::from_id_salt("skill_status_filter")
            .selected_text(status_text)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(model.skill_status_filter().is_none(), "All")
                    .clicked()
                {
                    model.set_skill_status_filter(None);
                }
                for status in model.skill_status_filter_options() {
                    if ui
                        .selectable_label(model.skill_status_filter() == Some(status), status)
                        .clicked()
                    {
                        model.set_skill_status_filter(Some(status.to_string()));
                    }
                }
            });
    });
}

fn render_inspector(ui: &mut egui::Ui, renderable: &RenderableView, colors: UiColors) {
    ui.add_space(10.0);
    ui.label(egui::RichText::new("Inspector").size(15.0).strong());
    ui.add_space(8.0);
    for section in &renderable.inspector_sections {
        ui.separator();
        ui.add_space(8.0);
        ui.label(egui::RichText::new(&section.title).strong());
        ui.add_space(4.0);
        for line in &section.lines {
            render_inspector_line(ui, line, colors);
        }
        ui.add_space(8.0);
    }
}

fn render_inspector_line(ui: &mut egui::Ui, line: &str, colors: UiColors) {
    let presentation = inspector_line_presentation(line);
    match presentation.kind {
        InspectorLineKind::Path => render_inspector_path_line(ui, &presentation, colors),
        InspectorLineKind::Mono => render_inspector_key_value(
            ui,
            &presentation,
            egui::RichText::new(&presentation.value)
                .monospace()
                .color(colors.ink_subtle)
                .size(12.0),
            colors,
        ),
        InspectorLineKind::StatusBadge => {
            if presentation.label.is_empty() {
                render_status_badge(ui, &presentation.value, colors);
            } else {
                ui.horizontal(|ui| {
                    render_inspector_key(ui, &presentation.label, colors);
                    render_status_badge(ui, &presentation.value, colors);
                });
            }
        }
        InspectorLineKind::Text => {
            if presentation.label.is_empty() {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&presentation.value)
                            .color(colors.ink_muted)
                            .size(13.0),
                    )
                    .wrap(),
                );
            } else {
                render_inspector_key_value(
                    ui,
                    &presentation,
                    egui::RichText::new(&presentation.value)
                        .color(colors.ink_muted)
                        .size(13.0),
                    colors,
                );
            }
        }
    }
}

fn render_inspector_path_line(
    ui: &mut egui::Ui,
    presentation: &InspectorLinePresentation,
    colors: UiColors,
) {
    ui.horizontal(|ui| {
        render_inspector_key(ui, &presentation.label, colors);
        let response = ui.add(
            egui::Label::new(
                egui::RichText::new(&presentation.value)
                    .monospace()
                    .color(colors.ink_subtle)
                    .size(12.0),
            )
            .wrap(),
        );
        response.on_hover_text(&presentation.value);
        if ui.small_button("Copy").on_hover_text("Copy path").clicked() {
            ui.output_mut(|output| output.copied_text = presentation.value.clone());
        }
        let can_reveal = path_exists(&presentation.value);
        if ui
            .add_enabled(can_reveal, egui::Button::new("Reveal").small())
            .on_hover_text("Reveal in Finder")
            .clicked()
        {
            reveal_path(&presentation.value);
        }
    });
}

fn render_inspector_key_value(
    ui: &mut egui::Ui,
    presentation: &InspectorLinePresentation,
    value: egui::RichText,
    colors: UiColors,
) {
    ui.horizontal_wrapped(|ui| {
        render_inspector_key(ui, &presentation.label, colors);
        ui.label(value);
    });
}

fn render_inspector_key(ui: &mut egui::Ui, label: &str, colors: UiColors) {
    if label.is_empty() {
        return;
    }
    ui.add_sized(
        [92.0, 18.0],
        egui::Label::new(
            egui::RichText::new(label)
                .color(colors.ink_subtle)
                .size(12.0),
        )
        .truncate(),
    )
    .on_hover_text(label);
}

fn render_action_controls(ui: &mut egui::Ui, model: &mut GuiModel, colors: UiColors) {
    ui.separator();
    ui.add_space(8.0);
    ui.label(egui::RichText::new("Controls").strong());
    ui.add_space(4.0);

    match model.active_view {
        NavigationView::Skills => {
            render_skill_controls(ui, model, colors);
        }
        NavigationView::Projects => {
            if let Some(draft) = model.open_project_draft().cloned() {
                let mut path_text = draft.path_text;
                render_path_field(
                    ui,
                    "Project path",
                    &mut path_text,
                    PathFieldKind::ExistingDirectory,
                    colors,
                );
                model.update_open_project_path(path_text);
                ui.horizontal(|ui| {
                    if ui.button("Open").clicked() {
                        let _ = model.request_save_open_project();
                    }
                    if ui.button("Cancel").clicked() {
                        model.cancel_open_project();
                    }
                });
                return;
            }
            if ui.button("Open project").clicked() {
                model.begin_open_project();
            }
            ui.add_space(4.0);
            if model.pending_remove_confirmation().is_some() {
                ui.label(
                    egui::RichText::new(DRIFT_REMOVE_CONFIRMATION_MESSAGE).color(colors.warning),
                );
                if ui
                    .button(egui::RichText::new("Confirm Remove").color(colors.danger))
                    .clicked()
                {
                    let _ = model.confirm_pending_remove();
                }
                ui.add_space(4.0);
            }
            for actions in project_actions(model).chunks(3) {
                ui.horizontal(|ui| {
                    for action in actions {
                        render_project_action_button(ui, model, colors, *action);
                    }
                });
            }
        }
        NavigationView::Agents => {
            render_agent_editor_controls(ui, model, colors);
        }
        NavigationView::Plugins => {
            render_plugin_controls(ui, model, colors);
        }
        NavigationView::Dashboard => {}
    }
}

fn render_plugin_action_button(
    ui: &mut egui::Ui,
    model: &mut GuiModel,
    _colors: UiColors,
    action: PluginAction,
) {
    if !ui.button(action.label()).clicked() {
        return;
    }

    match action {
        PluginAction::ScanPlugins => {
            let _ = model.request_scan_plugins();
        }
        PluginAction::Enable => {
            let _ = model.request_enable_selected_plugin();
        }
        PluginAction::Disable => {
            let _ = model.request_disable_selected_plugin();
        }
    }
}

fn render_plugin_controls(ui: &mut egui::Ui, model: &mut GuiModel, colors: UiColors) {
    ui.horizontal(|ui| {
        for action in plugin_actions(model) {
            render_plugin_action_button(ui, model, colors, action);
        }
    });
}

fn render_skill_action_button(
    ui: &mut egui::Ui,
    model: &mut GuiModel,
    _colors: UiColors,
    action: SkillAction,
) {
    let label = action.label();
    let clicked = ui.button(label).clicked();

    if !clicked {
        return;
    }

    match action {
        SkillAction::ScanAgentSpaces => {
            let _ = model.request_scan_agent_spaces();
        }
        SkillAction::Enable => {
            let _ = model.request_enable_selected_skill_instance();
        }
        SkillAction::Disable => {
            let _ = model.request_disable_selected_skill_instance_with_confirmation(false);
        }
    }
}

fn render_skill_controls(ui: &mut egui::Ui, model: &mut GuiModel, colors: UiColors) {
    if model
        .pending_disable_skill_instance_confirmation()
        .is_some()
    {
        ui.label(
            egui::RichText::new(SKILL_INSTANCE_DISABLE_CONFIRMATION_MESSAGE).color(colors.warning),
        );
        if ui
            .button(egui::RichText::new("Confirm Disable").color(colors.danger))
            .clicked()
        {
            let _ = model.confirm_pending_disable_skill_instance();
        }
        ui.add_space(4.0);
    }

    ui.horizontal(|ui| {
        for action in skill_actions(model) {
            render_skill_action_button(ui, model, colors, action);
        }
    });
}

fn render_agent_action_button(
    ui: &mut egui::Ui,
    model: &mut GuiModel,
    colors: UiColors,
    action: AgentAction,
) {
    let label = action.label();
    let clicked = if matches!(action, AgentAction::RemoveCustom) {
        ui.button(egui::RichText::new(label).color(colors.danger))
            .clicked()
    } else {
        ui.button(label).clicked()
    };
    if !clicked {
        return;
    }

    match action {
        AgentAction::EditSelected => {
            let _ = model.begin_edit_selected_agent_path();
        }
        AgentAction::ResetDefault => {
            let _ = model.request_reset_selected_agent_project_dirs();
        }
        AgentAction::RemoveCustom => {
            let _ = model.request_remove_selected_custom_agent();
        }
        AgentAction::AddCustom => {
            model.begin_add_custom_agent();
        }
    }
}

fn render_agent_editor_controls(ui: &mut egui::Ui, model: &mut GuiModel, colors: UiColors) {
    if let Some(draft) = model.agent_editor_draft().cloned() {
        let mut id_text = draft.id_text;
        let mut label_text = draft.label_text;
        let mut project_dir_text = draft.project_dir_text;
        let is_add = matches!(draft.mode, AgentEditorMode::AddCustom);

        if is_add {
            ui.label(egui::RichText::new("Agent id").color(colors.ink_subtle));
            ui.text_edit_singleline(&mut id_text);
            ui.label(egui::RichText::new("Label").color(colors.ink_subtle));
            ui.text_edit_singleline(&mut label_text);
        }
        render_path_field(
            ui,
            "Project Skill dir",
            &mut project_dir_text,
            PathFieldKind::AgentProjectDir,
            colors,
        );
        model.update_agent_editor_identity(id_text, label_text);
        model.update_agent_editor_project_dir(project_dir_text);

        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                let _ = model.request_save_agent_editor();
            }
            if ui.button("Cancel").clicked() {
                model.cancel_agent_editor();
            }
        });
        return;
    }

    ui.horizontal(|ui| {
        for action in agent_actions(model) {
            render_agent_action_button(ui, model, colors, action);
        }
    });
}

fn render_path_field(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    kind: PathFieldKind,
    colors: UiColors,
) {
    ui.label(egui::RichText::new(label).color(colors.ink_subtle));
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(value)
                .font(egui::TextStyle::Monospace)
                .desired_width(190.0),
        );
        if ui
            .button("Browse")
            .on_hover_text("Choose a folder")
            .clicked()
        {
            if let Some(path) = browse_for_folder(value) {
                *value = path;
            }
        }
        let can_reveal = path_exists(value);
        if ui
            .add_enabled(can_reveal, egui::Button::new("Reveal"))
            .on_hover_text("Reveal in Finder")
            .clicked()
        {
            reveal_path(value);
        }
        if ui
            .add_enabled(!value.trim().is_empty(), egui::Button::new("Copy"))
            .on_hover_text("Copy path")
            .clicked()
        {
            ui.output_mut(|output| output.copied_text = value.trim().to_string());
        }
    });
    if let Some(message) = path_validation_message(value, kind) {
        ui.label(
            egui::RichText::new(message)
                .color(colors.warning)
                .size(12.0),
        );
    }
}

fn path_exists(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && Utf8Path::new(trimmed).exists()
}

#[cfg(target_os = "macos")]
fn browse_for_folder(current_value: &str) -> Option<String> {
    let mut script =
        "POSIX path of (choose folder with prompt \"Choose a Skill-kits folder\")".to_string();
    let trimmed = current_value.trim();
    if !trimmed.is_empty() && Utf8Path::new(trimmed).is_dir() {
        script = format!(
            "POSIX path of (choose folder with prompt \"Choose a Skill-kits folder\" default location POSIX file {})",
            applescript_string(trimmed)
        );
    }
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!path.is_empty()).then_some(path)
}

#[cfg(not(target_os = "macos"))]
fn browse_for_folder(_current_value: &str) -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
fn reveal_path(value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return;
    }
    let _ = Command::new("open").arg("-R").arg(trimmed).status();
}

#[cfg(not(target_os = "macos"))]
fn reveal_path(_value: &str) {}

#[cfg(target_os = "macos")]
fn applescript_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn render_project_action_button(
    ui: &mut egui::Ui,
    model: &mut GuiModel,
    colors: UiColors,
    action: ProjectAction,
) {
    let label = action.label();
    let clicked = if matches!(action, ProjectAction::Remove) {
        ui.button(egui::RichText::new(label).color(colors.danger))
            .clicked()
    } else {
        ui.button(label).clicked()
    };

    if !clicked {
        return;
    }

    match action {
        ProjectAction::Refresh => {
            let _ = model.request_refresh_selected_project();
        }
        ProjectAction::AdoptSelected => {
            let _ = model.request_adopt_selected_discovered_project_skill();
        }
        ProjectAction::AdoptAll => {
            let _ = model.request_adopt_all_discovered_for_selected_project();
        }
        ProjectAction::ImportAsNew => {
            let _ = model.request_import_selected_project_conflict_as_new();
        }
        ProjectAction::Skip => {
            let _ = model.skip_selected_project_conflict();
        }
        ProjectAction::Deploy => {
            let _ = model.request_deploy_selected_skill_to_target_agent();
        }
        ProjectAction::Enable => {
            let _ = model.request_enable_selected_deployment();
        }
        ProjectAction::Disable => {
            let _ = model.request_disable_selected_deployment();
        }
        ProjectAction::Redeploy => {
            let _ = model.request_redeploy_selected_deployment();
        }
        ProjectAction::Overwrite => {
            let _ = model.request_overwrite_selected_deployment();
        }
        ProjectAction::Promote => {
            let _ = model.request_promote_selected_deployment();
        }
        ProjectAction::Remove => {
            let _ = model.request_remove_selected_deployment(false);
        }
    }
}

fn scope_label(scope: &GuiScope) -> String {
    match scope {
        GuiScope::GlobalInventory => "Managed Inventory".to_string(),
        GuiScope::Project(path) => path
            .file_name()
            .map(ToString::to_string)
            .unwrap_or_else(|| path.to_string()),
    }
}
