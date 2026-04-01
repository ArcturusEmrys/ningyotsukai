use generational_arena::{Arena, Index};

#[derive(Clone, glib::Variant)]
pub enum TrackerType {
    VTS(String),
}

impl TrackerType {
    pub fn new_vts(ip_addr: String) -> Self {
        TrackerType::VTS(ip_addr)
    }

    pub fn ip_addr(&self) -> &str {
        match self {
            Self::VTS(ip_addr) => ip_addr,
        }
    }
}

#[derive(Clone, glib::Variant, glib::Boxed)]
#[boxed_type(name = "NGTTracker")]
pub struct Tracker {
    pub(crate) name: String,
    pub(crate) tracker_type: TrackerType,
}

impl Tracker {
    pub fn new() -> Self {
        Tracker {
            name: "".to_string(),
            tracker_type: TrackerType::VTS("".to_string()),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn tracker_type(&self) -> &TrackerType {
        &self.tracker_type
    }

    pub fn as_ip_addr(&self) -> &str {
        match &self.tracker_type {
            TrackerType::VTS(ip_addr) => ip_addr,
        }
    }
}

pub struct Trackers(Arena<Tracker>);

impl Trackers {
    pub fn new() -> Self {
        Self(Arena::new())
    }

    pub fn iter(&self) -> impl Iterator<Item = (Index, &Tracker)> {
        self.0.iter()
    }

    pub fn tracker(&self, index: Index) -> Option<&Tracker> {
        self.0.get(index)
    }

    pub fn register(&mut self, tracker: Tracker) -> Index {
        self.0.insert(tracker)
    }

    pub fn unregister(&mut self, index: Index) -> Option<Tracker> {
        self.0.remove(index)
    }
}
