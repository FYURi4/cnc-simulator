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
        if child == parent {
            return;
        }

        let mut ancestor = Some(parent);
        while let Some(current) = ancestor {
            if current == child {
                return;
            }
            ancestor = self.world.get::<Parent>(current).map(|p| p.0);
        }

        if self.world.get::<Parent>(child).map(|p| p.0) == Some(parent) {
            return;
        }

        if let Some(old_parent) = self.world.get::<Parent>(child).map(|p| p.0) {
            if let Some(mut children) = self.world.get_mut::<Children>(old_parent) {
                children.0.retain(|entity| *entity != child);
            }
        }

        self.world.entity_mut(child).insert(Parent(parent));
        if let Some(mut children) = self.world.get_mut::<Children>(parent) {
            if !children.0.contains(&child) {
                children.0.push(child);
            }
        } else {
            self.world.entity_mut(parent).insert(Children(vec![child]));
        }
    }

    /// Reparents an entity while keeping its world-space transform unchanged.
    pub fn reparent_preserve_world(&mut self, child: Entity, parent: Entity) -> bool {
        if child == parent
            || self.world.get_entity(child).is_err()
            || self.world.get_entity(parent).is_err()
        {
            return false;
        }

        self.update();
        let Some(child_world) = self.global_matrix(child) else {
            return false;
        };
        let Some(parent_world) = self.global_matrix(parent) else {
            return false;
        };

        let mut ancestor = Some(parent);
        while let Some(current) = ancestor {
            if current == child {
                return false;
            }
            ancestor = self.world.get::<Parent>(current).map(|p| p.0);
        }

        let local = parent_world.inverse() * child_world;
        let (scale, rotation, matrix_translation) = local.to_scale_rotation_translation();
        let pivot = self
            .world
            .get::<Transform>(child)
            .map(|t| t.pivot_offset)
            .unwrap_or_default();
        let translation = matrix_translation + rotation * (scale * pivot);

        if let Some(mut transform) = self.world.get_mut::<Transform>(child) {
            transform.translation = translation;
            transform.rotation = rotation;
            transform.scale = scale;
        } else {
            return false;
        }

        self.set_parent(child, parent);
        self.update();
        true
    }

    pub fn set_name(&mut self, entity: Entity, name: impl Into<String>) {
        self.world.entity_mut(entity).insert(Name::new(name));
    }

    pub fn get_name(&self, entity: Entity) -> Option<&str> {
        self.world.get::<Name>(entity).map(|n| n.0.as_str())
    }

    /// Удаляет сущность вместе с детьми и отвязывает её от родителя.
    pub fn despawn(&mut self, entity: Entity) {
        if let Some(p) = self.world.get::<Parent>(entity).map(|p| p.0) {
            if let Some(mut ch) = self.world.get_mut::<Children>(p) {
                ch.0.retain(|e| *e != entity);
            }
        }
        let children: Vec<Entity> = self
            .world
            .get::<Children>(entity)
            .map(|c| c.0.clone())
            .unwrap_or_default();
        for c in children {
            self.despawn(c);
        }
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

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}
