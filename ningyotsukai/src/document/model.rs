use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, RwLock, Weak};

use generational_arena::Index;
use owning_ref::{OwningRef, OwningRefMut};

use crate::stage::Stage;
use crate::tracker::Trackers;

/// A Ningyotsukai document.
#[derive(Clone)]
pub struct Document(Arc<RwLock<DocumentInner>>);

#[derive(Clone)]
pub struct WeakDocument(Weak<RwLock<DocumentInner>>);
struct DocumentInner {
    stage: Stage,
    trackers: Trackers,
}

impl Default for Document {
    fn default() -> Self {
        Document(Arc::new(RwLock::new(DocumentInner {
            stage: Stage::new_with_size((1920.0, 1080.0)),
            trackers: Trackers::new(),
        })))
    }
}

impl Document {
    pub fn stage(&self) -> impl Deref<Target = Stage> {
        OwningRef::new(self.0.read().unwrap()).map(|me| &me.stage)
    }

    pub fn stage_mut(&mut self) -> impl DerefMut<Target = Stage> {
        OwningRefMut::new(self.0.write().unwrap()).map_mut(|me| &mut me.stage)
    }

    pub fn trackers(&self) -> impl Deref<Target = Trackers> {
        OwningRef::new(self.0.read().unwrap()).map(|me| &me.trackers)
    }

    pub fn trackers_mut(&mut self) -> impl DerefMut<Target = Trackers> {
        OwningRefMut::new(self.0.write().unwrap()).map_mut(|me| &mut me.trackers)
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

    pub fn collect_garbage_set(&self, set: &mut HashSet<Index>) {
        let mut garbage = vec![];
        for index in set.iter() {
            if !self.stage().contains_puppet(*index) {
                garbage.push(*index);
            }
        }

        for index in garbage {
            set.remove(&index);
        }
    }

    pub fn downgrade(&self) -> WeakDocument {
        WeakDocument(Arc::downgrade(&self.0))
    }

    pub fn as_ptr_val(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}

impl PartialEq for Document {
    fn eq(&self, other: &Self) -> bool {
        Arc::as_ptr(&self.0) == Arc::as_ptr(&other.0)
    }
}

impl Eq for Document {}

impl Hash for Document {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(Arc::as_ptr(&self.0) as usize);
    }
}

impl WeakDocument {
    pub fn upgrade(&self) -> Option<Document> {
        Some(Document(self.0.upgrade()?))
    }

    pub fn as_ptr_val(&self) -> usize {
        Weak::as_ptr(&self.0) as usize
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.as_ptr_val() == other.as_ptr_val()
    }
}
