//! Stupid-ass GTK class that exists solely to create a CSS node
use glib;
use gtk4;

use glib::subclass::InitializingObject;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use inox2d::render::InoxRendererExt;
use inox2d_opengl::OpenglRenderer;
use ningyo_extensions::GLAreaExt2;

use generational_arena::Index;

use crate::document::Document;
use crate::stage::Puppet as StagePuppet;

#[derive(Default)]
pub struct StageRendererState {
    document: Option<Arc<Mutex<Document>>>,

    /// All the renderers that exist to render puppets on our stage.
    renderers: HashMap<Index, OpenglRenderer>,

    /// Native GL for our own use.
    native_gl: Option<gl46::GlFns>,

    /// The last tick this widget processed, used to calculate timestamps to
    /// feed to Inox2D.
    last_mus: Option<i64>,
}

#[derive(Default, glib::Properties)]
#[properties(wrapper_type=StageRenderer)]
pub struct StageRendererImp {
    state: RefCell<StageRendererState>,

    #[property(get, set=Self::set_hadjustment)]
    hadjustment: RefCell<Option<gtk4::Adjustment>>,

    #[property(get, set=Self::set_vadjustment)]
    vadjustment: RefCell<Option<gtk4::Adjustment>>,

    #[property(get, set=Self::set_zadjustment)]
    zadjustment: RefCell<Option<gtk4::Adjustment>>,
}

#[glib::object_subclass]
impl ObjectSubclass for StageRendererImp {
    const NAME: &'static str = "NGTStageRenderer";
    type Type = StageRenderer;
    type ParentType = gtk4::GLArea;

    fn class_init(_class: &mut Self::Class) {}

    fn instance_init(_obj: &InitializingObject<Self>) {}
}

#[glib::derived_properties]
impl ObjectImpl for StageRendererImp {
    fn constructed(&self) {
        self.parent_constructed();

        // All masks in Inochi use the stencil buffer.
        self.obj().set_has_stencil_buffer(true);

        self.obj().connect_realize(move |gl_area| {
            let me = gl_area.downcast_ref::<StageRenderer>().unwrap();
            let native_gl = gl_area.as_native_gl();

            me.imp().state.borrow_mut().native_gl = Some(native_gl);
        });

        self.obj().connect_resize(move |gl_area, _width, _height| {
            gl_area.make_current();
            gl_area.imp().viewport_changed();
        });

        let tick = RefCell::new(Some(self.obj().add_tick_callback(move |me, clock| {
            let mut state = me.imp().state.borrow_mut();
            let document = state.document.clone().unwrap();
            let mut document = document.lock().unwrap();

            let mus = clock.frame_time();
            if let Some(last_mus) = state.last_mus {
                let del_mus = mus - last_mus;
                let dt = del_mus as f32 / 1_000_000.0;

                for (_, puppet) in document.stage_mut().iter_mut() {
                    puppet.ensure_render_initialized();
                    puppet.model_mut().puppet.begin_frame();
                    puppet.model_mut().puppet.end_frame(dt);
                }

                me.queue_render();
            }

            state.last_mus = Some(mus);

            //TODO: This tick callback keeps Ningyotsukai running
            glib::ControlFlow::Continue
        })));

        self.obj().connect_unrealize(move |_| {
            // Rust's type system doesn't support destructors, because it's
            // missing some kind of "owned reference" type, so Drop and
            // destroy require you to pretend the object needs to still be
            // logically valid just in case someone... grabs it out of the
            // trash, somehow?
            //
            // Hence we have to store an option, just so we can .take() the
            // callback and remove it.
            tick.borrow_mut().take().map(|tick| tick.remove());
        });
    }
}

impl WidgetImpl for StageRendererImp {}

impl GLAreaImpl for StageRendererImp {
    fn render(&self, _context: &gdk4::GLContext) -> glib::Propagation {
        let mut state = self.state.borrow_mut();
        let document = state.document.clone().unwrap();
        let document = document.lock().unwrap();

        let native_gl = state.native_gl.as_ref().unwrap();
        let fb = self.obj().framebuffer(native_gl);

        unsafe {
            native_gl.ClearColor(0.0, 0.0, 0.0, 0.0);
            native_gl.Clear(gl46::GL_COLOR_BUFFER_BIT | gl46::GL_STENCIL_BUFFER_BIT);
        }

        for (index, puppet) in document.stage().iter() {
            let renderer = state.renderers.entry(index).or_insert_with(|| {
                let gl = self.obj().as_glow_context();

                //TODO: Propagate this error to UI instead of panicing
                let mut render = OpenglRenderer::new(gl, &puppet.model()).unwrap();

                self.apply_viewport_to_renderer(&mut render, &puppet);

                render
            });

            renderer.set_surface_framebuffer(Some(fb));
            renderer
                .draw(&puppet.model().puppet)
                .expect("successful draw");
        }

        drop(document);
        drop(state);

        self.collect_garbage();

        glib::Propagation::Proceed
    }
}

impl StageRendererImp {
    fn apply_viewport_to_renderer(&self, renderer: &mut OpenglRenderer, puppet: &StagePuppet) {
        let width = self.obj().width().abs() as u32;
        let height = self.obj().height().abs() as u32;

        //TODO: Calculate our current viewport position and scale appropriately.
        let mut x = puppet.position().x();
        let mut y = puppet.position().y();
        let mut scale = puppet.scale();

        // Apply the viewport scale and position
        if let Some(ref hadjust) = *self.hadjustment.borrow() {
            x -= hadjust.value() as f32;
        }
        if let Some(ref vadjust) = *self.vadjustment.borrow() {
            y -= vadjust.value() as f32;
        }
        if let Some(ref zadjust) = *self.zadjustment.borrow() {
            let unlogged = 10.0_f32.powf(zadjust.value() as f32);
            scale *= unlogged;
        }

        // Cancel out the center coordinate offset that Inox uses for some reason
        x -= width as f32 / 2.0 / scale;
        y -= height as f32 / 2.0 / scale;

        renderer.camera.position.x = x;
        renderer.camera.position.y = y;
        renderer.camera.scale.x = scale;
        renderer.camera.scale.y = scale;

        if width > 0 && height > 0 {
            renderer.resize(width, height);
        }
    }

    fn viewport_changed(&self) {
        let mut state = self.state.borrow_mut();
        let document = state.document.clone().unwrap();
        let mut document = document.lock().unwrap();

        for (index, puppet) in document.stage_mut().iter_mut() {
            if let Some(render) = state.renderers.get_mut(&index) {
                self.apply_viewport_to_renderer(render, &puppet);
            }
        }
    }

    fn collect_garbage(&self) {
        let mut state = self.state.borrow_mut();
        let document = state.document.clone().unwrap();
        let document = document.lock().unwrap();

        let mut garbage = vec![];
        for index in state.renderers.keys() {
            if !document.stage().contains_puppet(*index) {
                garbage.push(*index);
            }
        }

        for index in garbage {
            state.renderers.remove(&index);
        }
    }

    fn set_hadjustment(&self, adjust: Option<gtk4::Adjustment>) {
        let self_obj = self.obj().clone();
        if let Some(ref adjust) = adjust {
            adjust.connect_value_changed(move |_| {
                self_obj.imp().viewport_changed();
            });
        }

        *self.hadjustment.borrow_mut() = adjust;
    }

    fn set_vadjustment(&self, adjust: Option<gtk4::Adjustment>) {
        let self_obj = self.obj().clone();
        if let Some(ref adjust) = adjust {
            adjust.connect_value_changed(move |_| {
                self_obj.imp().viewport_changed();
            });
        }

        *self.vadjustment.borrow_mut() = adjust;
    }

    fn set_zadjustment(&self, adjust: Option<gtk4::Adjustment>) {
        let self_obj = self.obj().clone();
        if let Some(ref adjust) = adjust {
            adjust.connect_value_changed(move |_| {
                self_obj.imp().viewport_changed();
            });
        }

        *self.zadjustment.borrow_mut() = adjust;
    }
}

glib::wrapper! {
    pub struct StageRenderer(ObjectSubclass<StageRendererImp>)
        @extends gtk4::GLArea, gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl StageRenderer {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn with_document(&self, document: Arc<Mutex<Document>>) -> &Self {
        let mut state = self.imp().state.borrow_mut();

        state.document = Some(document);
        state.renderers = HashMap::new();

        self
    }
}
