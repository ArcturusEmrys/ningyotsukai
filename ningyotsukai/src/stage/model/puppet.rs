use inox2d::math::rect::Rect;
use inox2d::model::Model;
use inox2d::params::Param;
use inox2d::puppet::Puppet as InoxPuppet;
use inox2d::texture::ShallowTexture;
use inox2d::{formats::inp::parse_inp_parts, params::ParamUuid};

use json::JsonValue;
use std::collections::HashMap;
use std::error::Error;
use std::io::Read;

use glam::Vec2;

use ningyo_binding::tracker::TrackerPacket;
use ningyo_binding::{Binding, ExpressionEval, parse_bindings};

pub struct Puppet {
    /// The position of the puppet's origin point, (0,0), relative to the stage.
    position: Vec2,

    /// The scale of the puppet. 1x is original puppet scale size.
    scale: f32,

    /// The fully loaded puppet.
    model: Model,

    /// Whether or not rendering has been initialized on the puppet.
    is_render_initialized: bool,

    /// The JSON data in the puppet.
    puppet_json: JsonValue,

    /// The texture data in the puppet.
    textures: Vec<ShallowTexture>,

    /// The bounding box of the puppet in its own coordiate system.
    ///
    /// Will change as the puppet is deformed by parameters.
    /// Is not affected by position or scale.
    bounds: Option<Rect>,

    /// Binding data configuration for this puppet.
    ///
    /// Defines the rules by which incoming tracker data drives the puppet.
    ///
    /// The additional two parameters indicate the last processed input and
    /// output for the binding.
    bindings: Vec<(Binding, f32, f32)>,

    /// Index of param UUIDs to strings.
    param_uuid_index: HashMap<ParamUuid, String>,

    /// The Lua expression evaluation environment.
    expression_eval: ExpressionEval,
}

impl Puppet {
    pub fn open(file: impl Read) -> Result<Self, Box<dyn Error>> {
        let (puppet_json, textures, vendors) = parse_inp_parts(file)?;
        let bindings = parse_bindings(&vendors)
            .unwrap_or_else(|| vec![])
            .into_iter()
            .map(|binding| (binding, 0.0, 0.0))
            .collect();
        let puppet_data = InoxPuppet::new_from_json(&puppet_json)?;
        let model = Model {
            puppet: puppet_data,
            textures,
            vendors,
        };

        let mut param_uuid_index = HashMap::new();
        for (name, param) in model.puppet.params().iter() {
            param_uuid_index.insert(param.uuid, name.clone());
        }

        Ok(Self {
            position: Vec2::new(0.0, 0.0),
            scale: 1.0,
            puppet_json,
            model,
            is_render_initialized: false,
            textures: vec![],
            bounds: None,
            bindings,
            param_uuid_index,
            expression_eval: ExpressionEval::new()?,
        })
    }

    pub fn ensure_render_initialized(&mut self) {
        if !self.is_render_initialized {
            self.model.puppet.init_transforms();
            self.model.puppet.init_rendering();
            self.model.puppet.init_params();
            self.model.puppet.init_physics();

            // One frame is required to prevent Inox from choking.
            self.model.puppet.begin_frame();
            self.model.puppet.end_frame(0.01);
        }

        self.is_render_initialized = true;
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    pub fn model_mut(&mut self) -> &mut Model {
        &mut self.model
    }

    pub fn position(&self) -> Vec2 {
        self.position
    }

    pub fn set_position(&mut self, new_pos: Vec2) {
        self.position = new_pos;
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn set_scale(&mut self, new_scale: f32) {
        self.scale = new_scale
    }

    pub fn apply_bindings(&mut self, packet: TrackerPacket) {
        self.expression_eval.set_tracker_packet(packet);
    }

    /// Get the current puppet bounds.
    pub fn bounds(&self) -> Option<&Rect> {
        self.bounds.as_ref()
    }

    pub fn param_by_uuid(&self, uuid: ParamUuid) -> Option<&Param> {
        let name = self.param_uuid_index.get(&uuid)?;
        self.model.puppet.params().get(name)
    }

    pub fn bindings(&self) -> &[(Binding, f32, f32)] {
        &self.bindings.as_slice()
    }

    pub fn bindings_mut(&mut self) -> &mut [(Binding, f32, f32)] {
        self.bindings.as_mut_slice()
    }

    /// Update the puppet's physics and apply tracker data to this puppet.
    pub fn update(&mut self, dt: f32) {
        self.ensure_render_initialized();

        if dt > 0.0 {
            self.model.puppet.begin_frame();
        }

        for (binding, bind_in_value, bind_out_value) in self.bindings.iter_mut() {
            match binding.eval(&self.expression_eval) {
                Ok((in_value, Some(out_value))) => {
                    *bind_in_value = in_value.unwrap_or(*bind_in_value);
                    *bind_out_value = out_value;

                    if let Some(param_name) = self.param_uuid_index.get(&binding.param) {
                        let mut orig = self
                            .model
                            .puppet
                            .param_ctx
                            .as_ref()
                            .unwrap()
                            .get(param_name)
                            .unwrap();

                        orig[binding.axis as usize] = out_value;

                        self.model
                            .puppet
                            .param_ctx
                            .as_mut()
                            .unwrap()
                            .set(param_name, orig)
                            .unwrap();
                    }
                }
                //Parameter missing
                Ok((_, None)) => {}

                //Lua error during evaluation
                Err(e) => {}
            }
        }

        if dt > 0.0 {
            self.model.puppet.end_frame(dt);
        }

        self.bounds = self.model.puppet.bounds();
    }
}
