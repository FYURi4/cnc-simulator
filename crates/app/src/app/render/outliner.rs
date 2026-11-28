use crate::app::App;
use crate::types::UiAction;
use crate::ui::show_node;
use bevy_ecs::prelude::*;
use engine_core::{Parent, Scene};

impl App {
    pub(super) fn draw_outliner(
        ui: &mut egui::Ui,
        scene: &mut Scene,
        selected: Option<Entity>,
        editing: &mut Option<Entity>,
        new_selection: &mut Option<Entity>,
        new_parent: &mut Option<(Entity, Entity)>,
        ui_action: &mut UiAction,
    ) {
        egui::TopBottomPanel::top("outliner")
            .resizable(true)
            .default_height(200.0)
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Outliner");
                    if ui.small_button("＋ Folder").clicked() {
                        *ui_action = UiAction::CreateFolder;
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let roots = {
                        let mut roots_vec = Vec::new();
                        let mut q = scene
                            .world
                            .query_filtered::<Entity, bevy_ecs::query::Without<Parent>>();
                        for e in q.iter(&scene.world) {
                            roots_vec.push(e);
                        }
                        roots_vec
                            .into_iter()
                            .filter(|e| scene.get_name(*e) != Some("3D Cursor"))
                            .collect::<Vec<_>>()
                    };
                    for root in roots {
                        show_node(
                            ui,
                            scene,
                            root,
                            selected,
                            editing,
                            new_selection,
                            new_parent,
                        );
                    }
                });
            });
    }
}
