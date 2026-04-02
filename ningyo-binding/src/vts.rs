use json::JsonValue;
use json::object::Object;

use crate::tracker::{AsTrackerPacket, TrackerPacket};

fn as_object(val: &JsonValue) -> Option<&Object> {
    if let JsonValue::Object(v) = val {
        Some(v)
    } else {
        None
    }
}

fn parse_xyz(val: &Object) -> Option<[f64; 3]> {
    Some([
        val.get("x").and_then(|v| v.as_f64())?,
        val.get("y").and_then(|v| v.as_f64())?,
        val.get("z").and_then(|v| v.as_f64())?,
    ])
}

fn as_array(val: &JsonValue) -> Option<&[JsonValue]> {
    if let JsonValue::Array(v) = val {
        Some(v.as_slice())
    } else {
        None
    }
}

fn parse_blendshapes(val: &[JsonValue]) -> Vec<(String, f64)> {
    val.iter()
        .filter_map(|v| {
            let v = as_object(v)?;
            Some((v.get("k")?.as_str()?.to_string(), v.get("v")?.as_f64()?))
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct VtsPacket {
    pub timestamp: u64,
    pub hotkey: i32,
    pub facefound: bool,
    pub rotation: [f64; 3],
    pub position: [f64; 3],
    pub eyeleft: [f64; 3],
    pub eyeright: [f64; 3],
    pub blendshapes: Vec<(String, f64)>,
}

impl VtsPacket {
    pub fn parse(data: &JsonValue) -> Option<Self> {
        let data = as_object(data)?;
        Some(Self {
            timestamp: data.get("Timestamp").and_then(|v| v.as_u64())?,
            hotkey: data.get("Hotkey").and_then(|v| v.as_i32())?,
            facefound: data.get("FaceFound").and_then(|v| v.as_bool())?,
            rotation: data
                .get("Rotation")
                .and_then(|v| parse_xyz(as_object(v)?))?,
            position: data
                .get("Position")
                .and_then(|v| parse_xyz(as_object(v)?))?,
            eyeleft: data.get("EyeLeft").and_then(|v| parse_xyz(as_object(v)?))?,
            eyeright: data
                .get("EyeRight")
                .and_then(|v| parse_xyz(as_object(v)?))?,
            blendshapes: data
                .get("BlendShapes")
                .and_then(|v| as_array(v))
                .map(|v| parse_blendshapes(v))?,
        })
    }
}

impl AsTrackerPacket for VtsPacket {
    fn as_tracker_packet(&self) -> TrackerPacket {
        let mut packet = TrackerPacket::new(self.timestamp, self.facefound);

        packet.insert("Head", "BoneRotRoll", self.rotation[0]);
        packet.insert("Head", "BoneRotPitch", self.rotation[1]);
        packet.insert("Head", "BoneRotYaw", self.rotation[2]);
        packet.insert("Head", "BonePosX", self.position[0]);
        packet.insert("Head", "BonePosY", self.position[1]);
        packet.insert("Head", "BonePosZ", self.position[2]);
        packet.insert("ftEyeXLeft", "Blendshape", self.eyeleft[0]);
        packet.insert("ftEyeYLeft", "Blendshape", self.eyeleft[1]);
        packet.insert("ftEyeZLeft", "Blendshape", self.eyeleft[2]);
        packet.insert("ftEyeXRight", "Blendshape", self.eyeright[0]);
        packet.insert("ftEyeYRight", "Blendshape", self.eyeright[1]);
        packet.insert("ftEyeZRight", "Blendshape", self.eyeright[2]);

        for (name, value) in &self.blendshapes {
            packet.insert(name, "Blendshape", *value);
        }

        packet
    }
}
