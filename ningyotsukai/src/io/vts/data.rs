use json::JsonValue;
use json::object::Object;

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

#[derive(Debug)]
pub struct VtsPacket {
    timestamp: u64,
    hotkey: i32,
    facefound: bool,
    rotation: [f64; 3],
    position: [f64; 3],
    eyeleft: [f64; 3],
    eyeright: [f64; 3],
    blendshapes: Vec<(String, f64)>,
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
                .get("Blendshapes")
                .and_then(|v| as_array(v))
                .map(|v| parse_blendshapes(v))?,
        })
    }
}
