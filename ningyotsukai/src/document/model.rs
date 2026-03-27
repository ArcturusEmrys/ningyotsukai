use crate::stage::Stage;

use generational_arena::Index;
use std::collections::HashMap;

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

    /// Given a map of puppet-associated items, clear out any entries whose
    /// keys do not correspond to a puppet on the current stage.
    pub fn collect_garbage<T>(&self, map: &mut HashMap<Index, T>) {
        let mut garbage = vec![];
        for index in map.keys() {
            if !self.stage().contains_puppet(*index) {
                garbage.push(*index);
            }
        }

        for index in garbage {
            map.remove(&index);
        }
    }
}
