use std::cell::RefCell;
use std::error::Error;
use std::sync::{Arc, Mutex};

use glib::subclass::InitializingObject;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use inox2d::render::InoxRendererExt;

use ningyo_gtk_wgpu::WgpuArea;
use ningyo_gtk_wgpu::prelude::*;
use ningyo_gtk_wgpu::subclass::prelude::*;

use ningyo_render_wgpu::{RenderTarget, WgpuRenderer, WgpuResources};

use ningyo_extensions::WidgetExt2;

use crate::document::Document;
use crate::render_preview::InoxRenderPreview;
use crate::render_preview::wgpu::InoxWgpuPreview;

struct State {
    document: Arc<Mutex<Document>>,
    renderer: Option<WgpuRenderer<'static>>,
    target: Option<Arc<Mutex<RenderTarget<'static>>>>,
    last_mus: Option<i64>,
}

#[derive(Default)]
pub struct MyWgpuAreaImp {
    state: RefCell<Option<State>>,
}

#[glib::object_subclass]
impl ObjectSubclass for MyWgpuAreaImp {
    const NAME: &'static str = "PIMyWgpuArea";
    type Type = MyWgpuArea;
    type ParentType = WgpuArea;

    fn class_init(_class: &mut Self::Class) {}

    fn instance_init(_obj: &InitializingObject<Self>) {}
}

impl ObjectImpl for MyWgpuAreaImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for MyWgpuAreaImp {
    fn realize(&self) {
        //TODO: These generate errors, we should propagate them upwards
        self.parent_realize();

        let mut state = self.state.borrow_mut();
        if let Some(state) = state.as_mut() {
            let resources = Arc::new(WgpuResources::new_with_user_device(
                self.obj().device().unwrap(),
                self.obj().queue().unwrap(),
            ));

            let target = Arc::new(Mutex::new(RenderTarget::new_texture_target()));

            let mut document = state.document.lock().unwrap();
            document.ensure_render_initialized();

            let renderer = WgpuRenderer::new_headless_with_resources(
                resources,
                &document.model,
                target.clone(),
            )
            .unwrap();

            state.renderer = Some(renderer);
            state.target = Some(target);
        }
    }
}

impl WgpuAreaImpl for MyWgpuAreaImp {
    fn preferred_device_descriptor(
        &self,
        adapter: &wgpu::Adapter,
    ) -> (wgpu::DeviceDescriptor<'static>, glib::GString) {
        (
            WgpuResources::preferred_device_descriptor(adapter),
            "WGPU Renderer".into(),
        )
    }

    fn preferred_texture_usage(&self) -> wgpu::TextureUsages {
        WgpuRenderer::required_render_target_uses()
    }

    fn render(&self) -> glib::ControlFlow {
        let mut state_outer = self.state.borrow_mut();
        let state = state_outer.as_mut().unwrap();
        let document = state.document.lock().unwrap();

        if let Err(e) = state
            .target
            .as_mut()
            .unwrap()
            .lock()
            .unwrap()
            .clear(&self.obj().device().unwrap(), &self.obj().queue().unwrap())
        {
            self.obj()
                .closest::<InoxWgpuPreview>()
                .unwrap()
                .display_error(&format!("{}", e));
        }

        let renderer = state.renderer.as_mut().unwrap();

        renderer
            .draw(&document.model.puppet)
            .expect("successful draw");

        glib::ControlFlow::Continue
    }

    fn resize(&self, target_tex: wgpu::Texture) -> glib::Propagation {
        let mut state = self.state.borrow_mut();
        let state_inner = state.as_mut().unwrap();
        let target = state_inner.target.as_mut().unwrap();
        let mut target_inner = target.lock().unwrap();

        let width = target_tex.width();
        let height = target_tex.height();

        if let Err(e) = (|| {
            target_inner.set_render_target(target_tex)?;
            target_inner.apply(&self.obj().device().unwrap(), &self.obj().queue().unwrap())?;
            Ok::<_, Box<dyn Error>>(())
        })() {
            self.obj()
                .closest::<InoxWgpuPreview>()
                .unwrap()
                .display_error(&format!("{}", e));
        }

        let document = state_inner.document.lock().unwrap();
        let bounds = document.model.puppet.bounds();

        if let Some(bounds) = bounds {
            let bounds_width = bounds.bottom_right_point().x - bounds.top_left_point().x;
            let bounds_height = bounds.bottom_right_point().y - bounds.top_left_point().y;

            let bounds_aspect_ratio = bounds_width / bounds_height;
            let widget_aspect_ratio = width as f32 / height as f32;

            let scale = if bounds_aspect_ratio > widget_aspect_ratio {
                width as f32 / bounds_width
            } else {
                height as f32 / bounds_height
            };

            let vp_camera = target_inner.viewport_camera_mut(0).unwrap();

            vp_camera.scale.x = scale;
            vp_camera.scale.y = scale;

            vp_camera.position.x = -bounds.top_left_point().x;
            vp_camera.position.y = -bounds.top_left_point().y;
        }

        glib::Propagation::Proceed
    }
}

glib::wrapper! {
    pub struct MyWgpuArea(ObjectSubclass<MyWgpuAreaImp>)
        @extends WgpuArea, gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl MyWgpuArea {
    pub fn bind(&self, document: Arc<Mutex<Document>>) {
        *self.imp().state.borrow_mut() = Some(State {
            document,
            renderer: None,
            target: None,
            last_mus: None,
        });

        self.add_tick_callback({
            let tick_self = self.clone();
            move |_, clock| {
                let mut state_outer = tick_self.imp().state.borrow_mut();
                let state = state_outer.as_mut().unwrap();
                let mut document = state.document.lock().unwrap();

                let mus = clock.frame_time();
                if let Some(last_mus) = state.last_mus {
                    let del_mus = mus - last_mus;
                    let dt = del_mus as f32 / 1_000_000.0;

                    document.model.puppet.begin_frame();
                    tick_self
                        .closest::<InoxRenderPreview>()
                        .unwrap()
                        .do_param_set(&mut *document);

                    document.model.puppet.end_frame(dt);

                    tick_self.queue_render();
                }

                state.last_mus = Some(mus);

                glib::ControlFlow::Continue
            }
        });
    }
}
