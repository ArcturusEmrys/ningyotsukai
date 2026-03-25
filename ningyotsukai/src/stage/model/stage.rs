use crate::stage::model::coord::Coord;
use crate::stage::model::puppet::Puppet;

use generational_arena::{Arena, Index};

/// The place puppets are rendered to.
pub struct Stage {
    size: Coord,
    puppets: Arena<Puppet>,
}

impl Stage {
    pub fn new_with_size(size: (f32, f32)) -> Self {
        Stage {
            size: Coord::new(size.0, size.1),
            puppets: Arena::new(),
        }
    }

    pub fn size(&self) -> &Coord {
        &self.size
    }

    pub fn add_puppet(&mut self, mut puppet: Puppet) -> Index {
        puppet.ensure_render_initialized();
        self.puppets.insert(puppet)
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
}
