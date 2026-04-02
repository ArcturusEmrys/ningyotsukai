use std::collections::HashMap;

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct TrackerParam {
    name: String,
    datatype: String,
}

#[derive(Clone)]
pub struct TrackerPacket {
    timestamp: u64,
    facefound: bool,
    params: Vec<TrackerParam>,
    data: HashMap<TrackerParam, f64>,
}

impl TrackerPacket {
    pub fn new(timestamp: u64, facefound: bool) -> Self {
        Self {
            timestamp,
            facefound,
            params: vec![],
            data: HashMap::new(),
        }
    }

    /// Set a value by name/datatype pair.
    pub fn insert(&mut self, name: &str, datatype: &str, value: f64) {
        let param = TrackerParam {
            name: name.to_string(),
            datatype: datatype.to_string(),
        };

        self.params.push(param.clone());
        self.data.insert(param, value);
    }

    /// Retrieve the timestamp this data was receieved or computed at.
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Whether or not the face (or other tracker target) was actually found.
    pub fn facefound(&self) -> bool {
        self.facefound
    }

    /// Get a particular value.
    pub fn value(&self, name: &str, datatype: &str) -> Option<f64> {
        self.data
            .get(&TrackerParam {
                name: name.to_string(),
                datatype: datatype.to_string(),
            })
            .copied()
    }

    pub fn iter_params(&self) -> impl Iterator<Item = (&str, &str)> {
        self.params
            .iter()
            .map(|param| (param.name.as_str(), param.datatype.as_str()))
    }

    pub fn param_count(&self) -> usize {
        self.params.len()
    }
}

pub trait AsTrackerPacket {
    fn as_tracker_packet(&self) -> TrackerPacket;
}
