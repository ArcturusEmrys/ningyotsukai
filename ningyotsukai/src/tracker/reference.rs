use generational_arena::Index;
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, Weak};

use gtk4::subclass::prelude::*;

use crate::document::Document;
use crate::tracker::model::Tracker;

/// Reference to an individual tracker in a document.
///
/// This is also available in a GObject wrapped form, see `TrackerRefItem`.
#[derive(Clone)]
pub struct TrackerRef(Weak<Mutex<Document>>, usize, Index);

impl TrackerRef {
    pub fn new(document: &Arc<Mutex<Document>>, tracker: Index) -> Self {
        Self(
            Arc::downgrade(document),
            document.as_ptr() as usize,
            tracker,
        )
    }

    pub fn document(&self) -> Option<Arc<Mutex<Document>>> {
        self.0.upgrade()
    }

    pub fn tracker_index(&self) -> Index {
        self.2
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

    pub fn with_param_ref(&self, name: &str, datatype: &str) -> TrackerParamRef {
        TrackerParamRef(
            self.0.clone(),
            (),
            self.2,
            name.to_string(),
            datatype.to_string(),
        )
    }
}

impl PartialEq for TrackerRef {
    fn eq(&self, other: &Self) -> bool {
        self.2 == other.2
            && ((self.0.upgrade().is_some() && self.0.ptr_eq(&other.0)) || self.1 == other.1)
    }
}

impl Eq for TrackerRef {}

impl Hash for TrackerRef {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.1.hash(h);
        self.2.hash(h);
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

/// Reference to an individual param in a tracker in a document.
///
/// This is also available in a GObject wrapped form, see `TrackerParamRefItem`.
#[derive(Clone)]
pub struct TrackerParamRef(Weak<Mutex<Document>>, (), Index, String, String);

impl TrackerParamRef {
    pub fn new(
        document: &Arc<Mutex<Document>>,
        tracker: Index,
        name: String,
        datatype: String,
    ) -> Self {
        Self(Arc::downgrade(document), (), tracker, name, datatype)
    }

    pub fn document(&self) -> Option<Arc<Mutex<Document>>> {
        self.0.upgrade()
    }

    pub fn tracker_index(&self) -> Index {
        self.2
    }

    pub fn param_name(&self) -> &str {
        &self.3
    }

    pub fn param_datatype(&self) -> &str {
        &self.4
    }

    /// Retrieve the tracker and call a function with it.
    ///
    /// Panics if the tracker is no longer available.
    pub fn value(&self) -> Option<f64> {
        let doc = self.document()?;
        let doc = doc.lock().unwrap();
        let track = doc.trackers().data(self.tracker_index())?;

        track.value(&self.3, &self.4)
    }
}

#[derive(Default)]
pub struct TrackerParamRefItemImp {
    track_ref: RefCell<Option<TrackerParamRef>>,
}

#[glib::object_subclass]
impl ObjectSubclass for TrackerParamRefItemImp {
    const NAME: &'static str = "NGTTrackerParamRefItem";
    type Type = TrackerParamRefItem;
}

impl ObjectImpl for TrackerParamRefItemImp {}

glib::wrapper! {
    pub struct TrackerParamRefItem(ObjectSubclass<TrackerParamRefItemImp>);
}

impl From<TrackerParamRef> for TrackerParamRefItem {
    fn from(track_ref: TrackerParamRef) -> Self {
        let me: Self = glib::Object::builder().build();

        *(me.imp().track_ref.borrow_mut()) = Some(track_ref);

        me
    }
}

impl TrackerParamRefItem {
    pub fn contents(&self) -> TrackerParamRef {
        self.imp().track_ref.borrow().as_ref().unwrap().clone()
    }
}
