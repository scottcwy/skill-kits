pub mod agents;
pub mod dashboard;
pub mod projects;
pub mod skills;
pub mod state;

use crate::core::paths::AppPaths;
use eframe::egui;
use state::{
    AgentEditorMode, GuiController, GuiModel, GuiScope, GuiStatusKind, NavigationView,
    RenderableView, UiColors, DRIFT_REMOVE_CONFIRMATION_MESSAGE,
    GLOBAL_UNINSTALL_CONFIRMATION_MESSAGE,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillAction {
    InstallLocal,
    Scan,
    Deploy,
    Uninstall,
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
    const ONBOARDING: [Self; 1] = [Self::AdoptAll];
    const SELECTED_DISCOVERED: [Self; 2] = [Self::AdoptSelected, Self::AdoptAll];
    const CONFLICT: [Self; 2] = [Self::ImportAsNew, Self::Skip];

    const NORMAL: [Self; 6] = [
        Self::Enable,
        Self::Disable,
        Self::Redeploy,
        Self::Overwrite,
        Self::Promote,
        Self::Remove,
    ];

    const MISSING_MANAGED_SOURCE: [Self; 2] = [Self::Promote, Self::Remove];

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
            Self::InstallLocal => "Install local",
            Self::Scan => "Scan",
            Self::Deploy => "Deploy",
            Self::Uninstall => "Uninstall",
        }
    }
}

pub fn skill_actions(model: &GuiModel) -> Vec<SkillAction> {
    let mut actions = vec![SkillAction::InstallLocal];
    if model.selected_skill().is_none() {
        return actions;
    }

    actions.push(SkillAction::Scan);
    if model.has_explicit_project_deploy_target() {
        actions.push(SkillAction::Deploy);
    }
    actions.push(SkillAction::Uninstall);
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
    let mut deploy_action = if model.has_project_deploy_target() {
        vec![ProjectAction::Deploy]
    } else {
        Vec::new()
    };

    if model.selected_deployment_status().is_none() {
        if model.selected_project_summary().is_none() {
            return deploy_action;
        }
        if model
            .selected_project_summary()
            .is_some_and(|project| project.pending_conflicts.is_empty())
            && model
                .selected_project_summary()
                .is_some_and(|project| project.discovered_unmanaged_count == 0)
        {
            deploy_action.extend(ProjectAction::REFRESH);
            return deploy_action;
        }
    }

    if model
        .selected_project_summary()
        .is_some_and(|project| !project.pending_conflicts.is_empty())
        && model.selected_deployment_status().is_none()
    {
        deploy_action.extend(ProjectAction::CONFLICT);
        return deploy_action;
    }

    if model.selected_discovered_project_skill().is_some()
        && model.selected_deployment_status().is_none()
    {
        deploy_action.extend(ProjectAction::SELECTED_DISCOVERED);
        return deploy_action;
    }

    if model
        .selected_project_summary()
        .is_some_and(|project| project.discovered_unmanaged_count > project.pending_conflicts.len())
        && model.selected_deployment_status().is_none()
    {
        deploy_action.extend(ProjectAction::ONBOARDING);
        return deploy_action;
    }

    if model
        .selected_deployment_status()
        .is_some_and(crate::gui::projects::is_invalid_toggle)
    {
        return Vec::new();
    }

    if model
        .selected_deployment_status()
        .is_some_and(|status| status.missing_managed_source)
    {
        ProjectAction::MISSING_MANAGED_SOURCE.to_vec()
    } else {
        let mut actions = ProjectAction::NORMAL.to_vec();
        deploy_action.append(&mut actions);
        deploy_action
    }
}

pub struct SkillKitsGuiApp {
    model: GuiModel,
    controller: GuiController,
    colors: UiColors,
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
        }
    }

    pub fn model(&self) -> &GuiModel {
        &self.model
    }
}

pub fn run_native(paths: AppPaths) -> anyhow::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Skill-kits",
        options,
        Box::new(move |_cc| Ok(Box::new(SkillKitsGuiApp::from_paths(&paths)?))),
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))
}

impl eframe::App for SkillKitsGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_dark_theme(ctx, self.colors);
        self.execute_one_pending_intent();

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
                        ui.label(self.model.pending_action_status_label());
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
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.separator();
                    ui.label(egui::RichText::new("Scope").color(self.colors.ink_subtle));
                    if ui
                        .selectable_label(
                            matches!(self.model.active_scope, GuiScope::GlobalInventory),
                            "Global Inventory",
                        )
                        .clicked()
                    {
                        self.model.select_scope(GuiScope::GlobalInventory);
                    }
                    let projects = self.model.recent_projects.clone();
                    for project in projects {
                        if ui
                            .selectable_label(
                                matches!(
                                    &self.model.active_scope,
                                    GuiScope::Project(path) if path == &project.path
                                ),
                                project.name,
                            )
                            .clicked()
                        {
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
                render_inspector(ui, &renderable, self.colors);
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

    egui::Grid::new("main_table")
        .striped(false)
        .min_col_width(92.0)
        .spacing([18.0, 8.0])
        .show(ui, |ui| {
            for column in &renderable.columns {
                ui.label(
                    egui::RichText::new(column)
                        .color(colors.ink_subtle)
                        .strong(),
                );
            }
            ui.end_row();

            for row in &renderable.main_rows {
                let mut row_clicked = false;
                for (index, cell) in row.cells.iter().enumerate() {
                    let response = ui.selectable_label(false, cell);
                    if index == 0 {
                        response.clone().on_hover_text("Select row");
                    }
                    row_clicked |= response.clicked();
                }
                if row_clicked {
                    model.select_render_row(&row.id);
                }
                ui.end_row();
            }
        });

    if renderable.main_rows.is_empty() {
        ui.add_space(20.0);
        ui.label(
            egui::RichText::new(renderable.empty_message.unwrap_or("No rows"))
                .color(colors.ink_subtle),
        );
    }
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
            ui.label(egui::RichText::new(line).color(colors.ink_muted));
        }
        ui.add_space(8.0);
    }
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
                ui.label(egui::RichText::new("Project path").color(colors.ink_subtle));
                ui.text_edit_singleline(&mut path_text);
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
        NavigationView::Dashboard => {}
    }
}

fn render_skill_action_button(
    ui: &mut egui::Ui,
    model: &mut GuiModel,
    colors: UiColors,
    action: SkillAction,
) {
    let label = action.label();
    let clicked = if matches!(action, SkillAction::Uninstall) {
        ui.button(egui::RichText::new(label).color(colors.danger))
            .clicked()
    } else {
        ui.button(label).clicked()
    };

    if !clicked {
        return;
    }

    match action {
        SkillAction::InstallLocal => {
            model.begin_install_local_skill();
        }
        SkillAction::Scan => {
            let _ = model.request_scan_selected_skill();
        }
        SkillAction::Deploy => {
            let _ = model.request_deploy_selected_skill_to_default_agent();
        }
        SkillAction::Uninstall => {
            let _ = model.request_uninstall_selected_skill(false);
        }
    }
}

fn render_skill_controls(ui: &mut egui::Ui, model: &mut GuiModel, colors: UiColors) {
    if model.pending_uninstall_confirmation().is_some() {
        ui.label(egui::RichText::new(GLOBAL_UNINSTALL_CONFIRMATION_MESSAGE).color(colors.warning));
        if ui
            .button(egui::RichText::new("Confirm Uninstall").color(colors.danger))
            .clicked()
        {
            let _ = model.confirm_pending_uninstall();
        }
        ui.add_space(4.0);
    }

    if let Some(draft) = model.install_local_skill_draft().cloned() {
        let mut path_text = draft.path_text;
        ui.label(egui::RichText::new("Local Skill path").color(colors.ink_subtle));
        ui.text_edit_singleline(&mut path_text);
        model.update_install_local_skill_path(path_text);
        ui.horizontal(|ui| {
            if ui.button("Install").clicked() {
                let _ = model.request_save_install_local_skill();
            }
            if ui.button("Cancel").clicked() {
                model.cancel_install_local_skill();
            }
        });
        return;
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
        ui.label(egui::RichText::new("Project Skill dir").color(colors.ink_subtle));
        ui.text_edit_singleline(&mut project_dir_text);
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
        GuiScope::GlobalInventory => "Global Inventory".to_string(),
        GuiScope::Project(path) => path
            .file_name()
            .map(ToString::to_string)
            .unwrap_or_else(|| path.to_string()),
    }
}
