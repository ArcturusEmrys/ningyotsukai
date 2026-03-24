use inox2d::formats::inp::parse_inp_parts;
use inox2d::model::Model;
use inox2d::puppet::Puppet as InoxPuppet;
use inox2d::texture::{ShallowTexture, decode_model_textures};

use json::JsonValue;
use std::error::Error;
use std::io::Read;

use crate::stage::model::coord::Coord;

pub struct Puppet {
    position: Coord,
    scale: f64,
    puppet_json: JsonValue,
    model: Model,
    is_render_initialized: bool,
    textures: Vec<ShallowTexture>,
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
            position: Coord::new(0.0, 0.0),
            scale: 1.0,
            puppet_json,
            model,
            is_render_initialized: false,
            textures: vec![],
        })
    }

    pub fn ensure_render_initialized(&mut self) {
        if !self.is_render_initialized {
            self.model.puppet.init_transforms();
            self.model.puppet.init_rendering();
            self.model.puppet.init_params();
            self.model.puppet.init_physics();
        }

        // One frame is required to prevent Inox from choking.
        self.model.puppet.begin_frame();
        self.model.puppet.end_frame(0.01);

        self.is_render_initialized = true;
    }
}
