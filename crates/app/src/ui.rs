use bevy_ecs::prelude::*;
use engine_core::Scene;

pub fn show_node(
    ui: &mut egui::Ui,
    scene: &mut Scene,
    entity: Entity,
    selected: Option<Entity>,
    editing: &mut Option<Entity>,
    new_selection: &mut Option<Entity>,
    new_parent: &mut Option<(Entity, Entity)>,
) {
    if scene.get_name(entity) == Some("3D Cursor") {
        return;
    }
    let children: Vec<Entity> = scene
        .world
        .get::<engine_core::Children>(entity)
        .map(|c| c.0.clone())
        .unwrap_or_default();
    let raw_name = scene
        .get_name(entity)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{:?}", entity));
    let is_selected = selected == Some(entity);
    let is_folder = scene.world.get::<crate::types::Folder>(entity).is_some();
    let id = ui.make_persistent_id(entity);
    let state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true);

    let (_toggle_response, header_response, _) = state
        .show_header(ui, |ui| {
            if *editing == Some(entity) {
                let mut name = raw_name.clone();
                let response =
                    ui.add(egui::TextEdit::singleline(&mut name).desired_width(120.0));
                if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if !name.is_empty() {
                        scene.set_name(entity, name);
                    }
                    *editing = None;
                }
                response
            } else {
                let label = if is_folder {
                    format!("📁 {}", raw_name)
                } else {
                    raw_name.clone()
                };
                let label = if is_selected {
                    egui::RichText::new(label)
                        .strong()
                        .color(egui::Color32::YELLOW)
                } else {
                    egui::RichText::new(label)
                };
                let button = ui.add(egui::Button::selectable(is_selected, label));
                let row_rect = egui::Rect::from_min_max(
                    button.rect.min,
                    egui::pos2(ui.max_rect().right(), button.rect.bottom()),
                );
                let response =
                    ui.interact(row_rect, id.with("row_drag"), egui::Sense::click_and_drag());
                if response.clicked() {
                    *new_selection = Some(entity);
                }
                if response.double_clicked() {
                    *editing = Some(entity);
                }
                response
            }
        })
        .body(|ui| {
            for child in children {
                show_node(
                    ui,
                    scene,
                    child,
                    selected,
                    editing,
                    new_selection,
                    new_parent,
                );
            }
        });

    header_response.inner.dnd_set_drag_payload(entity);
    if let Some(dragged) = header_response.inner.dnd_release_payload::<Entity>() {
        let child = *dragged;
        if child != entity {
            *new_parent = Some((child, entity));
        }
    }
}
