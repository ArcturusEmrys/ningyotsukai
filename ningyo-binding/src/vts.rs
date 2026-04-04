use json::JsonValue;
use json::object::Object;

use ningyo_extensions::prelude::*;

use crate::tracker::{AsTrackerPacket, TrackerPacket};

/// Cascading optional addition that treats optionals like NaNs.
fn maybe_add(val1: Option<f32>, val2: Option<f32>) -> Option<f32> {
    Some(val1? + val2?)
}

/// Cascading optional subtraction that treats optionals like NaNs.
fn maybe_sub(val1: Option<f32>, val2: Option<f32>) -> Option<f32> {
    Some(val1? - val2?)
}

/// Cascading optional division that treats optionals like NaNs.
fn maybe_div(val1: Option<f32>, val2: Option<f32>) -> Option<f32> {
    Some(val1? / val2?)
}

fn parse_xyz(val: &Object) -> Option<[f32; 3]> {
    Some([
        val.get("x").and_then(|v| v.as_f32())?,
        val.get("y").and_then(|v| v.as_f32())?,
        val.get("z").and_then(|v| v.as_f32())?,
    ])
}

fn parse_blendshapes(val: &[JsonValue]) -> Vec<(String, f32)> {
    val.iter()
        .filter_map(|v| {
            let v = v.as_object()?;
            Some((v.get("k")?.as_str()?.to_string(), v.get("v")?.as_f32()?))
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct VtsPacket {
    pub timestamp: u64,
    pub hotkey: i32,
    pub facefound: bool,
    pub rotation: [f32; 3],
    pub position: [f32; 3],
    pub eyeleft: [f32; 3],
    pub eyeright: [f32; 3],
    pub blendshapes: Vec<(String, f32)>,
}

impl VtsPacket {
    pub fn parse(data: &JsonValue) -> Option<Self> {
        let data = data.as_object()?;
        Some(Self {
            timestamp: data.get("Timestamp").and_then(|v| v.as_u64())?,
            hotkey: data.get("Hotkey").and_then(|v| v.as_i32())?,
            facefound: data.get("FaceFound").and_then(|v| v.as_bool())?,
            rotation: data
                .get("Rotation")
                .and_then(|v| parse_xyz(v.as_object()?))?,
            position: data
                .get("Position")
                .and_then(|v| parse_xyz(v.as_object()?))?,
            eyeleft: data
                .get("EyeLeft")
                .and_then(|v| parse_xyz(v.as_object()?))?,
            eyeright: data
                .get("EyeRight")
                .and_then(|v| parse_xyz(v.as_object()?))?,
            blendshapes: data
                .get("BlendShapes")
                .and_then(|v| v.as_list())
                .map(|v| parse_blendshapes(v))?,
        })
    }

    /// Synthesize normalized parameters for the left eye.
    fn synthesize_left_eye(
        packet: &mut TrackerPacket,
        blink: &str,
        x_max: &str,
        x_min: &str,
        y_max: &str,
        y_min: &str,
        squint: &str,
        widen: &str,
    ) {
        packet.insert_if(
            "ftEyeBlinkLeft",
            "Blendshape",
            packet.value(blink, "Blendshape"),
        );
        packet.insert_if(
            "ftEyeXLeft",
            "Blendshape",
            maybe_sub(
                packet.value(x_max, "Blendshape"),
                packet.value(x_min, "Blendshape"),
            ),
        );
        packet.insert_if(
            "ftEyeYLeft",
            "Blendshape",
            maybe_sub(
                packet.value(y_max, "Blendshape"),
                packet.value(y_min, "Blendshape"),
            ),
        );
        packet.insert_if(
            "ftEyeSquintLeft",
            "Blendshape",
            packet.value(squint, "Blendshape"),
        );
        packet.insert_if(
            "ftEyeWidenLeft",
            "Blendshape",
            //TODO: Is this the correct parameter?
            packet.value(widen, "Blendshape"),
        );
    }

    /// Synthesize normalized parameters for the right eye.
    fn synthesize_right_eye(
        packet: &mut TrackerPacket,
        blink: &str,
        x_max: &str,
        x_min: &str,
        y_max: &str,
        y_min: &str,
        squint: &str,
        widen: &str,
    ) {
        packet.insert_if(
            "ftEyeBlinkRight",
            "Blendshape",
            packet.value(blink, "Blendshape"),
        );
        packet.insert_if(
            "ftEyeXRight",
            "Blendshape",
            maybe_sub(
                packet.value(x_max, "Blendshape"),
                packet.value(x_min, "Blendshape"),
            ),
        );
        packet.insert_if(
            "ftEyeYRight",
            "Blendshape",
            maybe_sub(
                packet.value(y_max, "Blendshape"),
                packet.value(y_min, "Blendshape"),
            ),
        );
        packet.insert_if(
            "ftEyeSquintRight",
            "Blendshape",
            packet.value(squint, "Blendshape"),
        );
        packet.insert_if(
            "ftEyeWidenRight",
            "Blendshape",
            //TODO: Is this the correct parameter?
            packet.value(widen, "Blendshape"),
        );
    }

    /// Synthesize normalized parameters for the mouth.
    fn synthesize_mouth(
        packet: &mut TrackerPacket,
        jaw_open: Option<&str>,
        mouth_left: &str,
        mouth_right: &str,
        mouth_smile_left: &str,
        mouth_smile_right: &str,
        mouth_frown_left: &str,
        mouth_frown_right: &str,
    ) {
        // Some tracker sources (notably VTS on iOS) do not provide a jaw open
        // shape and synthesize it differently.
        if let Some(jaw_open) = jaw_open {
            packet.insert_if(
                "ftMouthOpen",
                "Blendshape",
                packet.value(jaw_open, "Blendshape"),
            );
        }

        packet.insert_if(
            "ftMouthX",
            "Blendshape",
            maybe_div(
                maybe_sub(
                    maybe_add(Some(1.0), packet.value(mouth_left, "Blendshape")),
                    packet.value(mouth_right, "Blendshape"),
                ),
                Some(2.0),
            ),
        );

        packet.insert_if(
            "ftMouthEmotion",
            "Blendshape",
            maybe_sub(
                maybe_div(
                    maybe_add(
                        packet.value(mouth_smile_left, "Blendshape"),
                        packet.value(mouth_smile_right, "Blendshape"),
                    ),
                    Some(2.0),
                ),
                maybe_div(
                    maybe_add(
                        packet.value(mouth_frown_left, "Blendshape"),
                        packet.value(mouth_frown_right, "Blendshape"),
                    ),
                    Some(2.0),
                ),
            )
            .map(|v| (1.0 + v).clamp(0.0, 2.0) / 2.0),
        );
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

        // As per https://github.com/Inochi2D/facetrack-d/blob/main/source/ft/adaptors/vtsproto.d,
        // Inochi Session performs a significant amount of blendshape
        // normalization and synthesis to paper over the difference between
        // various users of the VTS tracker protocol... including an Android
        // version that I didn't even know existed?!?
        // (I wouldn't have had to have bought an iPhone 13 Mini just to run
        // facial mocap on it lol)
        // Anyway, this logic is all copied from that.

        if packet.contains("jawOpen", "Blendshape") {
            if packet.contains("eyeLookOut_L", "Blendshape") {
                // VTube Studio for Android
                Self::synthesize_left_eye(
                    &mut packet,
                    "EyeBlinkLeft",
                    "eyeLookOut_L",
                    "eyeLookIn_L",
                    "eyeLookUp_L",
                    "eyeLookDown_L",
                    "eyeSquint_L",
                    "eyeSquint_L",
                );
                Self::synthesize_right_eye(
                    &mut packet,
                    "EyeBlinkRight",
                    "eyeLookIn_R",
                    "eyeLookOut_R",
                    "eyeLookUp_R",
                    "eyeLookDown_R",
                    "eyeSquint_R",
                    "eyeSquint_R",
                );
                Self::synthesize_mouth(
                    &mut packet,
                    Some("jawOpen"),
                    "mouthLeft",
                    "mouthRight",
                    "mouthSmile_L",
                    "mouthSmile_R",
                    "mouthFrown_L",
                    "mouthFrown_R",
                );
            } else if (packet.contains("eyeLookOutLeft", "Blendshape")) {
                //Meowface
                Self::synthesize_left_eye(
                    &mut packet,
                    "eyeBlinkLeft",
                    "eyeLookOutLeft",
                    "eyeLookInLeft",
                    "eyeLookUpLeft",
                    "eyeLookDownLeft",
                    "eyeSquintLeft",
                    "eyeSquintLeft",
                );
                Self::synthesize_right_eye(
                    &mut packet,
                    "eyeBlinkRight",
                    "eyeLookInRight",
                    "eyeLookOutRight",
                    "eyeLookUpRight",
                    "eyeLookDownRight",
                    "eyeSquintRight",
                    "eyeSquintRight",
                );
                Self::synthesize_mouth(
                    &mut packet,
                    Some("jawOpen"),
                    "mouthLeft",
                    "mouthRight",
                    "mouthSmileLeft",
                    "mouthSmileRight",
                    "mouthFrownLeft",
                    "mouthFrownRight",
                );
            }
        } else if packet.contains("JawOpen", "Blendshape") {
            // VTube Studio for iOS
            Self::synthesize_left_eye(
                &mut packet,
                "EyeBlinkLeft",
                "EyeLookOutLeft",
                "EyeLookInLeft",
                "EyeLookUpLeft",
                "EyeLookDownLeft",
                "EyeSquintLeft",
                "EyeWideLeft",
            );
            Self::synthesize_right_eye(
                &mut packet,
                "EyeBlinkRight",
                "EyeLookInRight",
                "EyeLookOutRight",
                "EyeLookUpRight",
                "EyeLookDownRight",
                "EyeSquintRight",
                "EyeWideRight",
            );
            Self::synthesize_mouth(
                &mut packet,
                None, //iOS normalization is weird
                "MouthLeft",
                "MouthRight",
                "MouthSmileLeft",
                "MouthSmileRight",
                "MouthFrownLeft",
                "MouthFrownRight",
            );

            packet.insert_if(
                "ftMouthOpen",
                "Blendshape",
                maybe_add(
                    maybe_div(
                        maybe_add(
                            packet.value("MouthLowerDownLeft", "Blendshape"),
                            packet.value("MouthUpperUpLeft", "Blendshape"),
                        ),
                        Some(2.0),
                    ),
                    maybe_div(
                        maybe_add(
                            packet.value("MouthLowerDownRight", "Blendshape"),
                            packet.value("MouthUpperUpRight", "Blendshape"),
                        ),
                        Some(2.0),
                    ),
                )
                .map(|v| v.clamp(0.0, 1.0)),
            );
        }

        // TODO: The original normalization code I stole this from says there
        // should be error handling here, and I have to agree.
        //
        // Unfortunately I didn't build this method to have error reporting!

        packet
    }
}
