/// A 2D position or size.
#[derive(Clone, Copy)]
pub struct Coord(f32, f32);

impl Coord {
    pub fn new(x: f32, y: f32) -> Self {
        Self(x, y)
    }

    pub fn x(&self) -> f32 {
        self.0
    }

    pub fn y(&self) -> f32 {
        self.1
    }
}
