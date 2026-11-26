use egui;
use bevy_ecs::prelude::*;
use engine_core::{Parent, Scene};
use crate::app::App;
use crate::ui::show_node;

impl App {
    pub(super) fn draw_outliner(ui: &mut egui::Ui, scene: &mut Scene, selected: Option<Entity>, editing: &mut Option<Entity>, new_selection: &mut Option<Entity>) {
        egui::TopBottomPanel::top("outliner")
                                    .resizable(true)
                                    .default_height(200.0)
                                    .show_inside(ui, |ui| {
                                        ui.heading("Outliner");
                                        ui.separator();
                                        egui::ScrollArea::vertical().show(ui, |ui| {
                                            let roots = {
                                                let mut roots_vec = Vec::new();
                                                let mut q = scene.world.query_filtered::<Entity, bevy_ecs::query::Without<Parent>>();
                                                for e in q.iter(&scene.world) { roots_vec.push(e); }
                                                roots_vec.into_iter().filter(|e| scene.get_name(*e) != Some("3D Cursor")).collect::<Vec<_>>()
                                            };
                                            for root in roots {
                                                show_node(ui, scene, root, selected, editing, new_selection);
                                            }
                                        });
                                    });
    }
}
