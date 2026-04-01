use crate::tracker::reference::TrackerRef;

#[derive(Clone, Eq, PartialEq, Hash)]
pub enum TrackerCookie {
    TrackerRef(TrackerRef),
    Sequential(u32),
}

impl Default for TrackerCookie {
    fn default() -> Self {
        Self::Sequential(0)
    }
}
