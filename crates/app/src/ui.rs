use bevy_ecs::prelude::*;
use engine_core::Scene;

pub fn show_node(
    ui: &mut egui::Ui,
    scene: &mut Scene,
    entity: Entity,
    selected: Option<Entity>,
    editing: &mut Option<Entity>,
    editing_name: &mut String,
    new_selection: &mut Option<Entity>,
    new_parent: &mut Option<(Entity, Entity)>,
    new_root: &mut Option<Entity>,
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
                let response =
                    ui.add(egui::TextEdit::singleline(editing_name).desired_width(140.0));
                if !response.has_focus() {
                    response.request_focus();
                }
                if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let name = editing_name.trim();
                    if !name.is_empty() {
                        scene.set_name(entity, name.to_owned());
                    }
                    *editing = None;
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
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
                ui.add_sized(
                    [ui.available_width(), 22.0],
                    egui::Button::selectable(is_selected, label),
                )
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
                    editing_name,
                    new_selection,
                    new_parent,
                    new_root,
                );
            }
        });

    let row_response = &header_response.inner;
    if row_response.clicked() {
        *new_selection = Some(entity);
    }
    if row_response.double_clicked() {
        *editing_name = raw_name.clone();
        *editing = Some(entity);
    }
    row_response.context_menu(|ui| {
        if ui.button("Rename").clicked() {
            *editing_name = raw_name.clone();
            *editing = Some(entity);
            ui.close();
        }
        if ui.button("Move to Scene Root").clicked() {
            *new_root = Some(entity);
            ui.close();
        }
    });

    row_response.dnd_set_drag_payload(entity);
    if let Some(dragged) = row_response.dnd_release_payload::<Entity>() {
        let child = *dragged;
        if child != entity {
            *new_parent = Some((child, entity));
        }
    }
}
