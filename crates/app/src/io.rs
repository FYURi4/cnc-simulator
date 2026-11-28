mod persistence;
mod scene;

pub use persistence::{load_scene, save_scene, spawn_saved_node};
pub use scene::{
    add_mesh_node, build_default_scene, capture_selected, capture_snapshot, despawn_all,
    paste_node,
};
