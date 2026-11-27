use bevy_ecs::prelude::*;
use glam::Mat4;

use crate::hierarchy::{Children, Parent};
use crate::name::Name;
use crate::transform::{GlobalTransform, Transform};

pub struct Scene {
    pub world: World,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            world: World::new(),
        }
    }

    pub fn spawn(&mut self, transform: Transform) -> Entity {
        self.world
            .spawn((transform, GlobalTransform::default()))
            .id()
    }

    pub fn set_parent(&mut self, child: Entity, parent: Entity) {
        self.world.entity_mut(child).insert(Parent(parent));
        if let Some(mut ch) = self.world.get_mut::<Children>(parent) {
            ch.0.push(child);
        } else {
            self.world.entity_mut(parent).insert(Children(vec![child]));
        }
    }

    pub fn set_name(&mut self, entity: Entity, name: impl Into<String>) {
        self.world.entity_mut(entity).insert(Name::new(name));
    }

    pub fn get_name(&self, entity: Entity) -> Option<&str> {
        self.world.get::<Name>(entity).map(|n| n.0.as_str())
    }

    /// Удаляет сущность вместе с детьми и отвязывает её от родителя.
    pub fn despawn(&mut self, entity: Entity) {
        // Отвязать от родителя
        if let Some(p) = self.world.get::<Parent>(entity).map(|p| p.0) {
            if let Some(mut ch) = self.world.get_mut::<Children>(p) {
                ch.0.retain(|e| *e != entity);
            }
        }
        // Рекурсивно удалить детей
        let children: Vec<Entity> = self
            .world
            .get::<Children>(entity)
            .map(|c| c.0.clone())
            .unwrap_or_default();
        for c in children {
            self.despawn(c);
        }
        // Удалить саму сущность
        let _ = self.world.despawn(entity);
    }

    pub fn update(&mut self) {
        let mut roots: Vec<Entity> = Vec::new();
        {
            let mut q = self.world.query_filtered::<Entity, Without<Parent>>();
            for e in q.iter(&self.world) {
                roots.push(e);
            }
        }
        for root in roots {
            Self::propagate(&mut self.world, root, Mat4::IDENTITY);
        }
    }

    fn propagate(world: &mut World, entity: Entity, parent_world: Mat4) {
        let local = world.get::<Transform>(entity).copied().unwrap_or_default();
        let world_mat = parent_world * local.matrix();
        if let Some(mut gt) = world.get_mut::<GlobalTransform>(entity) {
            gt.0 = world_mat;
        }
        let children: Vec<Entity> = world
            .get::<Children>(entity)
            .map(|c| c.0.clone())
            .unwrap_or_default();
        for child in children {
            Self::propagate(world, child, world_mat);
        }
    }

    pub fn global_matrix(&self, entity: Entity) -> Option<Mat4> {
        self.world.get::<GlobalTransform>(entity).map(|g| g.0)
    }
}
