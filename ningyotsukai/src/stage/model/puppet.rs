use std::ops::{Deref, DerefMut};
use std::time::Instant;

use inox2d::math::rect::RectBounds;
use inox2d::model::Model;
use inox2d::params::Param;
use inox2d::puppet::Puppet as InoxPuppet;
use inox2d::texture::ShallowTexture;
use inox2d::{formats::inp::parse_inp_parts, params::ParamUuid};

use json::JsonValue;
use std::collections::HashMap;
use std::error::Error;
use std::io::Read;
use std::sync::{Arc, Mutex};

use glam::Vec2;
use mlua::Error as LuaError;

use ningyo_binding::tracker::TrackerPacket;
use ningyo_binding::{Binding, ExpressionEval, parse_bindings};

use owning_ref::{OwningRef, OwningRefMut};

#[derive(Clone)]
pub struct Puppet(Arc<Mutex<PuppetInner>>);
struct PuppetInner {
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
    bounds: Option<RectBounds>,

    /// Binding data configuration for this puppet.
    ///
    /// Defines the rules by which incoming tracker data drives the puppet.
    ///
    /// The additional two parameters indicate the last processed input and
    /// output for the binding.
    bindings: Vec<(Binding, f32, f32, Option<LuaError>)>,

    /// Index of param UUIDs to strings.
    param_uuid_index: HashMap<ParamUuid, String>,

    /// The Lua expression evaluation environment.
    expression_eval: ExpressionEval,

    /// The timestamp this puppet was loaded.
    ///
    /// This should be created once at the time the puppet is loaded and NOT
    /// stored or recovered from anywhere.
    time_started: Instant,
}

impl Puppet {
    pub fn open(file: impl Read) -> Result<Self, Box<dyn Error>> {
        let (puppet_json, textures, vendors) = parse_inp_parts(file)?;
        let bindings = parse_bindings(&vendors)
            .unwrap_or_else(|| vec![])
            .into_iter()
            .map(|binding| (binding, 0.0, 0.0, None))
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

        Ok(Self(Arc::new(Mutex::new(PuppetInner {
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
            time_started: Instant::now(),
        }))))
    }

    pub fn ensure_render_initialized(&mut self) {
        let mut inner = self.0.lock().unwrap();
        if !inner.is_render_initialized {
            inner.model.puppet.init_transforms();
            inner.model.puppet.init_rendering();
            inner.model.puppet.init_params();
            inner.model.puppet.init_physics();

            // One frame is required to prevent Inox from choking.
            inner.model.puppet.begin_frame();
            inner.model.puppet.end_frame(0.01);
        }

        inner.is_render_initialized = true;

        // NOTE: Time should move forward even if we aren't getting tracker
        // updates.
        inner
            .expression_eval
            .set_jiffies(Instant::now() - inner.time_started);
    }

    pub fn model(&self) -> impl Deref<Target = Model> {
        OwningRef::new(self.0.lock().unwrap()).map(|me| &me.model)
    }

    pub fn model_mut(&mut self) -> impl DerefMut<Target = Model> {
        OwningRefMut::new(self.0.lock().unwrap()).map_mut(|me| &mut me.model)
    }

    pub fn position(&self) -> Vec2 {
        self.0.lock().unwrap().position
    }

    pub fn set_position(&mut self, new_pos: Vec2) {
        self.0.lock().unwrap().position = new_pos;
    }

    pub fn scale(&self) -> f32 {
        self.0.lock().unwrap().scale
    }

    pub fn set_scale(&mut self, new_scale: f32) {
        self.0.lock().unwrap().scale = new_scale
    }

    pub fn apply_bindings(&mut self, packet: TrackerPacket) {
        let me = self.0.lock().unwrap();

        me.expression_eval
            .set_jiffies(Instant::now() - me.time_started);
        me.expression_eval.set_tracker_packet(packet);
    }

    /// Get the current puppet bounds.
    pub fn bounds(&self) -> Option<RectBounds> {
        let me = self.0.lock().unwrap();

        me.bounds.clone()
    }

    pub fn param_by_uuid(&self, uuid: ParamUuid) -> Option<impl Deref<Target = Param>> {
        let me = self.0.lock().unwrap();

        OwningRef::new(me)
            .try_map(|me| {
                let name = me.param_uuid_index.get(&uuid).ok_or(())?;
                me.model.puppet.params().get(name).ok_or(())
            })
            .ok()
    }

    pub fn bindings(&self) -> impl Deref<Target = [(Binding, f32, f32, Option<LuaError>)]> {
        OwningRef::new(self.0.lock().unwrap()).map(|me| me.bindings.as_slice())
    }

    pub fn bindings_mut(
        &mut self,
    ) -> impl DerefMut<Target = [(Binding, f32, f32, Option<LuaError>)]> {
        OwningRefMut::new(self.0.lock().unwrap()).map_mut(|me| me.bindings.as_mut_slice())
    }

    /// Update the puppet's physics and apply tracker data to this puppet.
    pub fn update(&mut self, dt: f32) {
        #[cfg(feature = "timing")]
        let mut start_time = std::time::Instant::now();

        self.ensure_render_initialized();

        #[cfg(feature = "timing")]
        let mut lap_time = std::time::Instant::now();

        #[cfg(feature = "timing")]
        {
            eprintln!("  Update:");
            eprintln!(
                "    Pre-init: {}ms",
                (lap_time - start_time).as_micros() as f32 / 1_000.0
            );
        }

        let mut inner = self.0.lock().unwrap();

        if dt > 0.0 {
            inner.model.puppet.begin_frame();
        }

        #[cfg(feature = "timing")]
        {
            start_time = lap_time;
            lap_time = std::time::Instant::now();
            eprintln!(
                "    Begin frame: {}ms",
                (lap_time - start_time).as_micros() as f32 / 1_000.0
            );
        }

        let PuppetInner {
            bindings,
            expression_eval,
            param_uuid_index,
            model,
            ..
        } = &mut *inner;

        for (binding, bind_in_value, bind_out_value, bind_last_error) in bindings.iter_mut() {
            match binding.eval(expression_eval) {
                Ok((in_value, Some(out_value))) => {
                    *bind_in_value = in_value.unwrap_or(*bind_in_value);
                    *bind_out_value = out_value;
                    *bind_last_error = None;

                    if let Some(param_name) = param_uuid_index.get(&binding.param) {
                        let mut orig = model
                            .puppet
                            .param_ctx
                            .as_ref()
                            .unwrap()
                            .get(param_name)
                            .unwrap();

                        orig[binding.axis as usize] = out_value;

                        model
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
                Err(e) => *bind_last_error = Some(e),
            }
        }

        #[cfg(feature = "timing")]
        {
            start_time = lap_time;
            lap_time = std::time::Instant::now();
            eprintln!(
                "    Bindings: {}ms",
                (lap_time - start_time).as_micros() as f32 / 1_000.0
            );
        }

        if dt > 0.0 {
            inner.model.puppet.end_frame(dt);
        }

        #[cfg(feature = "timing")]
        {
            start_time = lap_time;
            lap_time = std::time::Instant::now();
            eprintln!(
                "    End frame: {}ms",
                (lap_time - start_time).as_micros() as f32 / 1_000.0
            );
        }

        inner.bounds = inner.model.puppet.bounds();

        #[cfg(feature = "timing")]
        {
            start_time = lap_time;
            lap_time = std::time::Instant::now();
            eprintln!(
                "    Bounds: {}ms",
                (lap_time - start_time).as_micros() as f32 / 1_000.0
            );
        }
    }
}
