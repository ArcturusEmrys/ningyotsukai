use crate::stage::Stage;

/// A Ningyotsukai document.
pub struct Document {
    stage: Stage,
}

impl Default for Document {
    fn default() -> Self {
        Document {
            stage: Stage::new_with_size((1920.0, 1080.0)),
        }
    }
}

impl Document {
    pub fn stage(&self) -> &Stage {
        &self.stage
    }

    pub fn stage_mut(&mut self) -> &mut Stage {
        &mut self.stage
    }
}
