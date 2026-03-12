use inox2d::formats::inp::parse_inp_parts;
use inox2d::model::{Model, ModelTexture, VendorData};
use inox2d::puppet::Puppet;
use json::JsonValue;
use std::error::Error;
use std::io::Read;

pub struct Document {
    pub(crate) puppet_json: JsonValue,
    pub(crate) model: Model,
    is_render_initialized: bool,
}

impl Document {
    pub fn open(file: impl Read) -> Result<Self, Box<dyn Error>> {
        let (puppet_json, textures, vendors) = parse_inp_parts(file)?;
        let puppet_data = Puppet::new_from_json(&puppet_json)?;
        let model = Model {
            puppet: puppet_data,
            textures,
            vendors,
        };

        Ok(Self {
            puppet_json,
            model,
            is_render_initialized: false,
        })
    }

    pub fn puppet_data(&self) -> &Puppet {
        &self.model.puppet
    }

    pub fn textures(&self) -> &[ModelTexture] {
        self.model.textures.as_slice()
    }

    pub fn vendors(&self) -> &[VendorData] {
        self.model.vendors.as_slice()
    }

    pub fn ensure_render_initialized(&mut self) {
        if !self.is_render_initialized {
            self.model.puppet.init_transforms();
            self.model.puppet.init_rendering();
            self.model.puppet.init_params();
            self.model.puppet.init_physics();
        }

        self.is_render_initialized = true;
    }
}
