use glam::Vec2;
use inox2d::params::ParamUuid;

use ningyo_extensions::prelude::*;

pub struct RatioBinding {
    inverse: bool,
    in_range: Vec2,
    out_range: Vec2,
}

pub enum BindingType {
    Ratio(RatioBinding),
    Expression(String),
}

pub struct Binding {
    name: String,
    param: ParamUuid,
    axis: u8,
    dampen_level: f32,
    source_name: String,
    source_display_name: String,
    source_type: String,
    binding_type: BindingType,
}

impl Binding {
    fn parse_vec2(value: &json::JsonValue) -> Option<glam::Vec2> {
        let list = value.as_list()?;
        let x = list.get(0)?.as_number()?;
        let y = list.get(1)?.as_number()?;

        Some(glam::Vec2::new(x.into(), y.into()))
    }

    pub fn from_payload(value: &json::JsonValue) -> Option<Vec<Binding>> {
        let list = value.as_list()?;
        let mut bindings = vec![];

        for item in list {
            if let Some(item) = item.as_object() {
                let name = item.get("name")?.to_string();
                let param = ParamUuid(item.get("param")?.as_u32()?);
                let axis = item.get("axis")?.as_u8()?;
                let dampen_level = item.get("dampenLevel")?.as_f32()?;
                let source_name = item.get("sourceName")?.to_string();
                let source_display_name = item.get("sourceDisplayName")?.to_string();
                let source_type = item.get("sourceType")?.to_string();
                let binding_type = item.get("bindingType")?;
                let binding = match binding_type.as_str()? {
                    "RatioBinding" => BindingType::Ratio(RatioBinding {
                        inverse: item.get("inverse")?.as_bool()?,
                        in_range: Binding::parse_vec2(item.get("inRange")?)?,
                        out_range: Binding::parse_vec2(item.get("outRange")?)?,
                    }),
                    "ExpressionBinding" => {
                        BindingType::Expression(item.get("expression")?.to_string())
                    }
                    _ => return None, //TODO: more descriptive errors
                };

                bindings.push(Binding {
                    name,
                    param,
                    axis,
                    dampen_level,
                    source_name,
                    source_display_name,
                    source_type,
                    binding_type: binding,
                })
            }
        }

        Some(bindings)
    }
}
