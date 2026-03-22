use crate::stage::model::coord::Coord;
use crate::stage::model::puppet::Puppet;

/// The place puppets are rendered to.
pub struct Stage {
    size: Coord,
    puppets: Vec<Puppet>,
}

impl Stage {
    pub fn new_with_size(size: (f32, f32)) -> Self {
        Stage {
            size: Coord::new(size.0, size.1),
            puppets: vec![],
        }
    }

    pub fn size(&self) -> &Coord {
        &self.size
    }

    pub fn add_puppet(&mut self, puppet: Puppet) {
        self.puppets.push(puppet);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Puppet> {
        self.puppets.iter()
    }
}
