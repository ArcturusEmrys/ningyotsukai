use glib;
use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use glib::subclass::InitializingObject;

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use inox2d::render::InoxRendererExt;
use inox2d_opengl::OpenglRenderer;

use crate::document::Document;
use ningyo_extensions::GLAreaExt2;

struct State {
    document: Arc<Mutex<Document>>,
    renderer: Option<OpenglRenderer>,
    glfns: Option<gl46::GlFns>,
    last_mus: Option<i64>,
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/render_preview/window.ui")]
pub struct InoxRenderPreviewImp {
    state: RefCell<Option<State>>,

    #[template_child]
    paned_view: TemplateChild<gtk4::Paned>,
    #[template_child]
    gl_view: TemplateChild<gtk4::GLArea>,
    #[template_child]
    error_view: TemplateChild<gtk4::Frame>,
    #[template_child]
    error_label: TemplateChild<gtk4::Label>,
}

#[glib::object_subclass]
impl ObjectSubclass for InoxRenderPreviewImp {
    const NAME: &'static str = "PIInoxRenderPreview";
    type Type = InoxRenderPreview;
    type ParentType = gtk4::Window;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for InoxRenderPreviewImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for InoxRenderPreviewImp {}

impl WindowImpl for InoxRenderPreviewImp {}

glib::wrapper! {
    pub struct InoxRenderPreview(ObjectSubclass<InoxRenderPreviewImp>)
        @extends gtk4::Window, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Accessible,
            gtk4::Native, gtk4::Root, gtk4::ShortcutManager;
}

impl InoxRenderPreview {
    pub fn new(document: Arc<Mutex<Document>>) -> Self {
        let selfish: Self = glib::Object::builder().build();

        *selfish.imp().state.borrow_mut() = Some(State {
            document,
            renderer: None,
            glfns: None,
            last_mus: None,
        });
        selfish.bind();

        selfish
    }

    fn display_error(&self, error: &str) {
        self.imp()
            .paned_view
            .set_start_child(Some(&*self.imp().error_view));
        self.imp().error_label.set_label(error);
    }

    fn bind(&self) {
        // Inox2D needs a context at creation time, so we defer creation until
        // the first render on the GLArea.
        let realize_self = self.clone();

        self.imp().gl_view.set_has_stencil_buffer(true);

        self.imp().gl_view.connect_realize(move |gl_area| {
            let annoying_self_borrow = realize_self.imp().state.borrow();
            let mut document = annoying_self_borrow
                .as_ref()
                .unwrap()
                .document
                .lock()
                .unwrap();

            gl_area.make_current();
            if let Some(e) = gl_area.error() {
                realize_self.display_error(e.message());
            }

            document.ensure_render_initialized();

            let gl = gl_area.as_glow_context();
            let native_gl = gl_area.as_native_gl();

            // TODO: GLDebug logging
            let renderer = OpenglRenderer::new(gl, &document.model);
            drop(document);
            drop(annoying_self_borrow);

            match renderer {
                Ok(mut renderer) => {
                    renderer.camera.scale.x = 0.15;
                    renderer.camera.scale.y = 0.15;

                    let mut state_outer = realize_self.imp().state.borrow_mut();
                    let state = state_outer.as_mut().unwrap();
                    state.glfns = Some(native_gl);
                    state.renderer = Some(renderer)
                }
                Err(e) => {
                    realize_self.display_error(&format!("Error initializing renderer: {}", e))
                }
            }
        });

        let resize_self = self.clone();
        self.imp()
            .gl_view
            .connect_resize(move |gl_area, width, height| {
                gl_area.make_current();
                if let Some(e) = gl_area.error() {
                    resize_self.display_error(e.message());
                }

                if width > 0 && height > 0 {
                    let mut state_outer = resize_self.imp().state.borrow_mut();
                    let state = state_outer.as_mut().unwrap();
                    state
                        .renderer
                        .as_mut()
                        .unwrap()
                        .resize(width as u32, height as u32);
                }
            });

        let tick_self = self.clone();
        self.imp().gl_view.add_tick_callback(move |_, clock| {
            let mut state_outer = tick_self.imp().state.borrow_mut();
            let state = state_outer.as_mut().unwrap();
            let mut document = state.document.lock().unwrap();

            let mus = clock.frame_time();
            if let Some(last_mus) = state.last_mus {
                let del_mus = mus - last_mus;
                let dt = del_mus as f32 / 1_000_000.0;

                document.model.puppet.begin_frame();
                document.model.puppet.end_frame(dt);

                tick_self.imp().gl_view.queue_render();
            }

            state.last_mus = Some(mus);

            glib::ControlFlow::Continue
        });

        let render_self = self.clone();
        self.imp().gl_view.connect_render(move |gl_area, _context| {
            if let Some(e) = gl_area.error() {
                render_self.display_error(e.message());
            }

            let mut state_outer = render_self.imp().state.borrow_mut();
            let state = state_outer.as_mut().unwrap();
            let document = state.document.lock().unwrap();

            let renderer = state.renderer.as_mut().unwrap();
            let native_gl = state.glfns.as_ref().unwrap();
            let fb = gl_area.framebuffer(native_gl);

            unsafe {
                native_gl.ClearColor(0.0, 0.0, 0.0, 1.0);
                native_gl.Clear(gl46::GL_COLOR_BUFFER_BIT);
            }

            renderer.set_surface_framebuffer(Some(fb));

            renderer
                .draw(&document.model.puppet)
                .expect("successful draw");

            unsafe {
                native_gl.BindFramebuffer(gl46::GL_FRAMEBUFFER, fb.0.into());
                native_gl.Flush();
            }

            if let Some(e) = gl_area.error() {
                render_self.display_error(e.message());
            }

            glib::Propagation::Proceed
        });
    }
}
