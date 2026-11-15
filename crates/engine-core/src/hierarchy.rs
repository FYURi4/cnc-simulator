use bevy_ecs::prelude::*;

#[derive(Component, Clone, Copy, Debug)]
pub struct Parent(pub Entity);

#[derive(Component, Clone, Debug, Default)]
pub struct Children(pub Vec<Entity>);