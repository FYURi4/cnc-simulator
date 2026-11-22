pub mod hierarchy;
pub mod name;
pub mod scene;
pub mod transform;

pub mod gcode;

pub use hierarchy::{Children, Parent};
pub use name::Name;
pub use scene::Scene;
pub use transform::{GlobalTransform, Transform};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}