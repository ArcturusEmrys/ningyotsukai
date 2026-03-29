use inox2d::formats::inp::parse_inp_parts;
use inox2d::math::rect::Rect;
use inox2d::model::Model;
use inox2d::puppet::Puppet as InoxPuppet;
use inox2d::texture::ShallowTexture;

use json::JsonValue;
use std::error::Error;
use std::io::Read;

use glam::Vec2;

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
}

impl Puppet {
    pub fn open(file: impl Read) -> Result<Self, Box<dyn Error>> {
        let (puppet_json, textures, vendors) = parse_inp_parts(file)?;
        let puppet_data = InoxPuppet::new_from_json(&puppet_json)?;
        let model = Model {
            puppet: puppet_data,
            textures,
            vendors,
        };

        Ok(Self {
            position: Vec2::new(0.0, 0.0),
            scale: 1.0,
            puppet_json,
            model,
            is_render_initialized: false,
            textures: vec![],
            bounds: None,
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

    /// Update the puppet's physics simulations.
    pub fn update(&mut self, dt: f32) {
        self.ensure_render_initialized();

        if dt > 0.0 {
            self.model.puppet.begin_frame();
            self.model.puppet.end_frame(dt);
        }

        self.bounds = self.model.puppet.bounds();
    }

    /// Get the current puppet bounds.
    pub fn bounds(&self) -> Option<&Rect> {
        self.bounds.as_ref()
    }
}
