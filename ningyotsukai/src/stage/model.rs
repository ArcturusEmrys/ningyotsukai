/// A 2D position or size.
pub struct Coord(f32, f32);

impl Coord {
    pub fn x(&self) -> f32 {
        self.0
    }

    pub fn y(&self) -> f32 {
        self.1
    }
}

/// The place puppets are rendered to.
pub struct Stage {
    size: Coord,
}

impl Stage {
    pub fn new_with_size(size: (f32, f32)) -> Self {
        Stage {
            size: Coord(size.0, size.1),
        }
    }

    pub fn size(&self) -> &Coord {
        &self.size
    }
}
