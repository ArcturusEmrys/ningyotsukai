use glam::Vec2;
use inox2d::params::ParamUuid;

use ningyo_extensions::prelude::*;

pub struct RatioBinding {
    pub inverse: bool,
    pub in_range: Vec2,
    pub out_range: Vec2,
}

impl RatioBinding {
    /// Evaluate a ratio binding parameter.
    fn eval(&self, t: f32) -> f32 {
        let in_range_width = self.in_range.y - self.in_range.x;
        let mut inner_t = (t - self.in_range.x) / in_range_width;
        if inner_t < 0.0 {
            inner_t = 0.0;
        } else if inner_t > 1.0 {
            inner_t = 1.0;
        }
        if self.inverse {
            inner_t = 1.0 - inner_t;
        }

        let out_range_width = self.out_range.y - self.out_range.x;
        let mut outer_t = (inner_t - self.out_range.x) / out_range_width;
        if outer_t < self.out_range.x {
            outer_t = self.out_range.x;
        } else if outer_t > self.out_range.y {
            outer_t = self.out_range.y;
        }

        outer_t
    }
}

pub enum BindingType {
    Ratio(RatioBinding),
    Expression(String),
}

impl BindingType {
    fn eval(&self, t: f32) -> f32 {
        match self {
            Self::Ratio(binding) => binding.eval(t),
            Self::Expression(_) => unimplemented!(),
        }
    }
}

pub struct Binding {
    pub name: String,
    pub param: ParamUuid,
    pub axis: u8,
    pub dampen_level: f32,
    pub source_name: String,
    pub source_display_name: String,
    pub source_type: String,
    pub binding_type: BindingType,
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

    pub fn eval(&self, in_value: f32) -> f32 {
        match &self.binding_type {
            BindingType::Ratio(ratio) => ratio.eval(in_value),
            BindingType::Expression(_) => 0.0, //TODO: unimplemented
        }
    }
}
