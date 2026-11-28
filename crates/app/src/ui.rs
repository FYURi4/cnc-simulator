use bevy_ecs::prelude::*;
use engine_core::Scene;

pub fn show_node(
    ui: &mut egui::Ui,
    scene: &mut Scene,
    entity: Entity,
    selected: Option<Entity>,
    editing: &mut Option<Entity>,
    new_selection: &mut Option<Entity>,
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
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{:?}", entity));
    let is_selected = selected == Some(entity);

    let id = ui.make_persistent_id(entity);
    let state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true);

    state
        .show_header(ui, |ui| {
            if *editing == Some(entity) {
                let mut name = raw_name.clone();
                let response = ui.add(egui::TextEdit::singleline(&mut name).desired_width(120.0));
                if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if !name.is_empty() {
                        scene.set_name(entity, name);
                    }
                    *editing = None;
                }
            } else {
                let label = if is_selected {
                    egui::RichText::new(raw_name.clone())
                        .strong()
                        .color(egui::Color32::YELLOW)
                } else {
                    egui::RichText::new(raw_name.clone())
                };
                let resp = ui.add(egui::Button::selectable(is_selected, label));
                let rest_rect = egui::Rect::from_min_max(
                    egui::pos2(resp.rect.right(), resp.rect.top()),
                    ui.max_rect().right_bottom(),
                );
                let row_resp = ui.interact(rest_rect, id.with("row_click"), egui::Sense::click());
                let combined = resp.union(row_resp);
                if combined.clicked() {
                    *new_selection = Some(entity);
                }
                if combined.double_clicked() {
                    *editing = Some(entity);
                }
            }
        })
        .body(|ui| {
            for child in children {
                show_node(ui, scene, child, selected, editing, new_selection);
            }
        });
}
