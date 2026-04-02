use generational_arena::{Arena, Index};
use std::collections::HashMap;

use ningyo_binding::tracker::TrackerPacket;

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

pub struct Trackers {
    trackers: Arena<Tracker>,
    last_tracker_data: HashMap<Index, TrackerPacket>,
}

impl Trackers {
    pub fn new() -> Self {
        Self {
            trackers: Arena::new(),
            last_tracker_data: HashMap::new(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (Index, &Tracker)> {
        self.trackers.iter()
    }

    pub fn tracker(&self, index: Index) -> Option<&Tracker> {
        self.trackers.get(index)
    }

    pub fn register(&mut self, tracker: Tracker) -> Index {
        self.trackers.insert(tracker)
    }

    pub fn unregister(&mut self, index: Index) -> Option<Tracker> {
        let out = self.trackers.remove(index);

        self.last_tracker_data.remove(&index);

        out
    }

    pub fn report_data(&mut self, index: Index, packet: TrackerPacket) {
        self.last_tracker_data.insert(index, packet);
    }

    pub fn data(&self, index: Index) -> Option<&TrackerPacket> {
        self.last_tracker_data.get(&index)
    }
}
