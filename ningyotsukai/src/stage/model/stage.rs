use crate::stage::model::puppet::Puppet;

use generational_arena::{Arena, Index};
use glam::Vec2;

/// The place puppets are rendered to.
pub struct Stage {
    size: Vec2,
    puppets: Arena<Puppet>,
}

impl Stage {
    pub fn new_with_size(size: (f32, f32)) -> Self {
        Stage {
            size: Vec2::new(size.0, size.1),
            puppets: Arena::new(),
        }
    }

    pub fn size(&self) -> Vec2 {
        self.size
    }

    pub fn add_puppet(&mut self, mut puppet: Puppet) -> Index {
        puppet.ensure_render_initialized();
        self.puppets.insert(puppet)
    }

    pub fn puppet(&self, index: Index) -> Option<Puppet> {
        self.puppets.get(index).cloned()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Index, &Puppet)> {
        self.puppets.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Index, &mut Puppet)> {
        self.puppets.iter_mut()
    }

    pub fn contains_puppet(&self, index: Index) -> bool {
        self.puppets.contains(index)
    }

    pub fn update(&mut self, dt: f32) {
        for (_, puppet) in self.iter_mut() {
            puppet.update(dt);
        }
    }
}
