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
use ningyo_gtk_wgpu::prelude::*;
use ningyo_gtk_wgpu::subclass::prelude::*;
use ningyo_gtk_wgpu::{ExportedTexture, WgpuArea};
use ningyo_render_wgpu::WgpuRenderer;

use generational_arena::Index;

use crate::document::Document;
use crate::stage::Puppet as StagePuppet;
use pollster::block_on;

#[derive(Default)]
pub struct StageRendererState {
    document: Option<Arc<Mutex<Document>>>,

    /// All the renderers that exist to render puppets on our stage.
    renderers: HashMap<Index, WgpuRenderer<'static>>,

    /// Renderdoc API
    #[cfg(feature = "renderdoc")]
    doc: Option<renderdoc::RenderDoc<renderdoc::V100>>,
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
    type ParentType = WgpuArea;

    fn class_init(class: &mut Self::Class) {}

    fn instance_init(_obj: &InitializingObject<Self>) {}
}

#[glib::derived_properties]
impl ObjectImpl for StageRendererImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for StageRendererImp {}

impl WgpuAreaImpl for StageRendererImp {
    fn resize(&self, x: i32, y: i32) -> glib::Propagation {
        self.viewport_changed();

        for (_, renderer) in self.state.borrow_mut().renderers.iter_mut() {
            renderer.resize(x as u32, y as u32).unwrap();
        }

        glib::Propagation::Proceed
    }

    fn render(&self) -> glib::Propagation {
        eprintln!("RENDER START");
        let mut state = self.state.borrow_mut();
        let document = state.document.clone().unwrap();
        let document = document.lock().unwrap();

        #[cfg(feature = "renderdoc")]
        {
            use std::ptr::null;
            if state.doc.is_none() {
                state.doc = renderdoc::RenderDoc::new().ok();
            }

            //TODO: Can I get native window handles out of GTK?
            if state.doc.is_some() {
                state
                    .doc
                    .as_mut()
                    .unwrap()
                    .start_frame_capture(null(), null());
            }
        }

        // TODO: Issue a clear command on one of the renderers.

        // TODO: WGPU's insistence on async device creation means we can't
        // create renderers inline.
        let mut texture = None;
        let mut device = None;

        for (index, puppet) in document.stage().iter() {
            if !state.renderers.contains_key(&index) {
                let new_renderer = if let Some((_, zygote)) = state.renderers.iter().next() {
                    zygote.clone_with_model(&puppet.model()).unwrap()
                } else {
                    block_on(WgpuRenderer::new_headless(&puppet.model())).unwrap()
                };

                state.renderers.insert(index, new_renderer);
            }
            let mut renderer = state.renderers.get_mut(&index).unwrap();

            if texture.is_none() {
                texture = renderer.target_texture();
                device = Some(renderer.device());
            }

            self.apply_viewport_to_renderer(&mut renderer, &puppet);

            renderer
                .draw(&puppet.model().puppet)
                .expect("successful draw");
        }

        if let (Some(texture), Some(device)) = (texture, device) {
            // TODO: Upstream this into the gtk-wgpu library.
            // TODO: Do we REALLY need to do this every frame?!
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: None,
                })
                .unwrap();
            let export = ExportedTexture::export_to_dmabuf(&device, &texture).unwrap();
            self.obj()
                .attach_texture(export.into_gdk_texture().unwrap());
        }

        drop(document);
        drop(state);

        self.collect_garbage();

        #[cfg(feature = "renderdoc")]
        {
            use std::ptr::null;
            let mut state = self.state.borrow_mut();

            if state.doc.is_some() {
                state
                    .doc
                    .as_mut()
                    .unwrap()
                    .end_frame_capture(null(), null());
            }
        }

        eprintln!("RENDER END");

        glib::Propagation::Proceed
    }
}

impl StageRendererImp {
    fn apply_viewport_to_renderer(
        &self,
        renderer: &mut WgpuRenderer<'static>,
        puppet: &StagePuppet,
    ) {
        let width = self.obj().width().abs() as u32;
        let height = self.obj().height().abs() as u32;
        let dpi = self.obj().scale_factor().abs() as u32;

        let mut scale = puppet.scale();
        let zoom = if let Some(ref zadjust) = *self.zadjustment.borrow() {
            10.0_f32.powf(zadjust.value() as f32)
        } else {
            1.0
        };

        scale *= zoom;

        let mut x = 0.0;
        let mut y = 0.0;

        //Cancel out the center coordinate offset Inox uses
        x -= width as f32 / 2.0 / scale;
        y -= height as f32 / 2.0 / scale;

        // Apply the viewport scale and position
        if let Some(ref hadjust) = *self.hadjustment.borrow() {
            x -= hadjust.value() as f32 / puppet.scale();
        }
        if let Some(ref vadjust) = *self.vadjustment.borrow() {
            y -= vadjust.value() as f32 / puppet.scale();
        }

        x += puppet.position().x / puppet.scale();
        y += puppet.position().y / puppet.scale();

        renderer.camera.position.x = x;
        renderer.camera.position.y = y;
        renderer.camera.scale.x = scale * dpi as f32;
        renderer.camera.scale.y = scale * dpi as f32;

        if width > 0 && height > 0 && dpi > 0 {
            renderer.resize(width * dpi, height * dpi).unwrap();
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

        document.collect_garbage(&mut state.renderers);
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
        @extends WgpuArea, gtk4::Widget,
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
