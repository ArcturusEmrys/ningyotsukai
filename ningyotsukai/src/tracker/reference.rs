use generational_arena::Index;
use std::cell::RefCell;
use std::sync::{Arc, Mutex, Weak};

use gtk4::subclass::prelude::*;

use crate::document::Document;
use crate::tracker::model::Tracker;

/// Reference to an individual tracker in a document.
///
/// This is also available in a GObject wrapped form, see `TrackerRefItem`.
#[derive(Clone)]
pub struct TrackerRef(Weak<Mutex<Document>>, Index);

impl TrackerRef {
    pub fn new(document: &Arc<Mutex<Document>>, tracker: Index) -> Self {
        Self(Arc::downgrade(document), tracker)
    }

    pub fn document(&self) -> Option<Arc<Mutex<Document>>> {
        self.0.upgrade()
    }

    pub fn tracker_index(&self) -> Index {
        self.1
    }

    /// Retrieve the tracker and call a function with it.
    ///
    /// Panics if the tracker is no longer available.
    pub fn with_tracker<T, F: FnOnce(&Tracker) -> T>(&self, f: F) -> T {
        let doc = self.document().unwrap();
        let doc = doc.lock().unwrap();
        let track = doc.trackers().tracker(self.tracker_index()).unwrap();

        f(track)
    }
}

#[derive(Default)]
pub struct TrackerRefItemImp {
    track_ref: RefCell<Option<TrackerRef>>,
}

#[glib::object_subclass]
impl ObjectSubclass for TrackerRefItemImp {
    const NAME: &'static str = "NGTTrackerRefItem";
    type Type = TrackerRefItem;
}

impl ObjectImpl for TrackerRefItemImp {}

glib::wrapper! {
    pub struct TrackerRefItem(ObjectSubclass<TrackerRefItemImp>);
}

impl From<TrackerRef> for TrackerRefItem {
    fn from(track_ref: TrackerRef) -> Self {
        let me: Self = glib::Object::builder().build();

        *(me.imp().track_ref.borrow_mut()) = Some(track_ref);

        me
    }
}

impl TrackerRefItem {
    pub fn contents(&self) -> TrackerRef {
        self.imp().track_ref.borrow().as_ref().unwrap().clone()
    }
}
