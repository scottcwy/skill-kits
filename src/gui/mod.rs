pub mod agents;
pub mod dashboard;
pub mod projects;
pub mod skills;
pub mod state;

use crate::core::paths::AppPaths;
use eframe::egui;
use state::{GuiModel, GuiScope, NavigationView, RenderableView, UiColors};

pub struct SkillKitsGuiApp {
    model: GuiModel,
    colors: UiColors,
}

impl SkillKitsGuiApp {
    pub fn from_paths(paths: &AppPaths) -> crate::core::Result<Self> {
        Ok(Self::new(GuiModel::load(paths)?))
    }

    pub fn new(model: GuiModel) -> Self {
        Self {
            model,
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

        egui::TopBottomPanel::top("top_bar")
            .frame(egui::Frame::none().fill(self.colors.surface_1))
            .exact_height(42.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(egui::RichText::new("Skill-kits").strong());
                    ui.separator();
                    ui.label(scope_label(&self.model.active_scope));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!(
                            "{} pending intent(s)",
                            self.model.pending_intents().len()
                        ));
                    });
                });
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
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(self.colors.canvas))
            .show(ctx, |ui| {
                let renderable = self.model.renderable_view();
                render_main(ui, &renderable, self.colors);
            });
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

fn render_main(ui: &mut egui::Ui, renderable: &RenderableView, colors: UiColors) {
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
                for cell in &row.cells {
                    ui.label(cell);
                }
                ui.end_row();
            }
        });

    if renderable.main_rows.is_empty() {
        ui.add_space(20.0);
        ui.label(egui::RichText::new("No rows").color(colors.ink_subtle));
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

fn scope_label(scope: &GuiScope) -> String {
    match scope {
        GuiScope::GlobalInventory => "Global Inventory".to_string(),
        GuiScope::Project(path) => path
            .file_name()
            .map(ToString::to_string)
            .unwrap_or_else(|| path.to_string()),
    }
}
