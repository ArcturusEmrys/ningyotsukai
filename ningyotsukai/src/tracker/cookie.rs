use generational_arena::Index;
use std::sync::{Arc, Mutex, Weak};

use crate::document::Document;

/// Reference to an individual tracker in a document.
#[derive(Clone)]
pub struct TrackerRef(Weak<Mutex<Document>>, Index);

impl TrackerRef {
    pub fn new(document: Arc<Mutex<Document>>, tracker: Index) -> Self {
        Self(Arc::downgrade(&document), tracker)
    }

    pub fn document(&self) -> Option<Arc<Mutex<Document>>> {
        self.0.upgrade()
    }

    pub fn tracker(&self) -> Index {
        self.1
    }
}

#[derive(Clone)]
pub enum TrackerCookie {
    TrackerRef(TrackerRef),
    Sequential(u32),
}

impl TrackerCookie {
    pub fn tracker_ref(document: Arc<Mutex<Document>>, tracker: Index) -> Self {
        Self::TrackerRef(TrackerRef::new(document, tracker))
    }

    pub fn sequential(index: u32) -> Self {
        Self::Sequential(index)
    }
}

impl Default for TrackerCookie {
    fn default() -> Self {
        Self::Sequential(0)
    }
}
