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
use ningyo_gtk_wgpu::WgpuArea;
use ningyo_gtk_wgpu::prelude::*;
use ningyo_gtk_wgpu::subclass::prelude::*;
use ningyo_render_wgpu::{WgpuRenderer, WgpuResources};

use generational_arena::Index;

use crate::document::{Document, DocumentManager};
use crate::stage::Puppet as StagePuppet;

#[derive(Default)]
pub struct StageRendererState {
    document: Option<Document>,

    document_manager: Option<DocumentManager>,

    resources: Option<Arc<Mutex<WgpuResources>>>,

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

    fn class_init(_class: &mut Self::Class) {}

    fn instance_init(_obj: &InitializingObject<Self>) {}
}

#[glib::derived_properties]
impl ObjectImpl for StageRendererImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for StageRendererImp {
    fn realize(&self) {
        self.parent_realize();

        let resources = Arc::new(Mutex::new(WgpuResources::new_with_user_device(
            self.obj().device().unwrap(),
            self.obj().queue().unwrap(),
        )));

        let mut state = self.state.borrow_mut();

        if let Some(document_manager) = &state.document_manager {
            document_manager.use_resources(self.obj().adapter().unwrap(), resources.clone());
        }

        state.resources = Some(resources);
    }
}

impl WgpuAreaImpl for StageRendererImp {
    fn preferred_device_descriptor(&self) -> (wgpu::DeviceDescriptor<'static>, glib::GString) {
        (
            WgpuResources::preferred_device_descriptor(),
            "WGPU Renderer".into(),
        )
    }

    fn preferred_texture_usage(&self) -> wgpu::TextureUsages {
        WgpuRenderer::required_render_target_uses()
    }

    fn resize(&self, texture: wgpu::Texture) -> glib::Propagation {
        self.viewport_changed();

        for (_, renderer) in self.state.borrow_mut().renderers.iter_mut() {
            renderer.set_render_target(texture.clone()).unwrap();
        }

        glib::Propagation::Proceed
    }

    fn render(&self) -> glib::Propagation {
        let mut state = self.state.borrow_mut();
        let StageRendererState {
            document,
            document_manager: _,
            resources,
            renderers,
            #[cfg(feature = "renderdoc")]
            doc,
        } = &mut *state;
        let document = document.as_mut().unwrap();

        #[cfg(feature = "renderdoc")]
        {
            use std::ptr::null;
            if doc.is_none() {
                *doc = renderdoc::RenderDoc::new().ok();
            }

            //TODO: Can I get native window handles out of GTK?
            if doc.is_some() {
                doc.as_mut().unwrap().start_frame_capture(null(), null());
            }
        }

        // TODO: Issue a clear command on one of the renderers.
        for (index, puppet) in document.stage().iter() {
            let mut renderer = renderers.entry(index).or_insert_with(|| {
                WgpuRenderer::new_headless_with_resources(
                    resources.clone().unwrap(),
                    &*puppet.model(),
                )
                .unwrap()
            });

            renderer
                .set_render_target(self.obj().texture().unwrap())
                .unwrap();

            self.apply_viewport_to_renderer(&mut renderer, &puppet);

            renderer
                .draw(&puppet.model().puppet)
                .expect("successful draw");
        }
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

        for (index, puppet) in document.stage().iter() {
            if let Some(render) = state.renderers.get_mut(&index) {
                self.apply_viewport_to_renderer(render, &puppet);
            }
        }
    }

    fn collect_garbage(&self) {
        let mut state = self.state.borrow_mut();
        let StageRendererState {
            document,
            renderers,
            ..
        } = &mut *state;

        let document = document.as_ref().unwrap();

        document.collect_garbage(renderers);
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

    pub fn with_document(&self, document: Document, document_manager: DocumentManager) -> &Self {
        let mut state = self.imp().state.borrow_mut();

        if let Some(resources) = &state.resources {
            document_manager.use_resources(self.adapter().unwrap(), resources.clone());
        }

        state.document = Some(document);
        state.document_manager = Some(document_manager);
        state.renderers = HashMap::new();

        self
    }
}
