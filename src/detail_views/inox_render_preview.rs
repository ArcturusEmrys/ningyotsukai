use glib;
use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use glib::subclass::InitializingObject;

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::null;
use std::sync::{Arc, Mutex};

use glow;
use inox2d::node::InoxNodeUuid;
use inox2d::render::InoxRendererExt;
use inox2d_opengl::OpenglRenderer;

use crate::document::Document;
use crate::string_ext::StrExt;

struct State {
    document: Arc<Mutex<Document>>,
    renderer: Option<OpenglRenderer>,
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/inox_render_preview.ui")]
pub struct InoxRenderPreviewImp {
    state: RefCell<Option<State>>,

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
    type ParentType = gtk4::Box;

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

impl BoxImpl for InoxRenderPreviewImp {}

glib::wrapper! {
    pub struct InoxRenderPreview(ObjectSubclass<InoxRenderPreviewImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::Orientable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl InoxRenderPreview {
    pub fn new(document: Arc<Mutex<Document>>) -> Self {
        let selfish: Self = glib::Object::builder().build();

        document.lock().unwrap().ensure_render_initialized();

        *selfish.imp().state.borrow_mut() = Some(State {
            document,
            renderer: None,
        });
        selfish.bind();

        selfish
    }

    fn display_error(&self, error: &str) {
        self.remove(&self.imp().gl_view.get());
        self.append(&self.imp().error_view.get());

        self.imp().error_label.set_label(error);
    }

    fn bind(&self) {
        // Inox2D needs a context at creation time, so we defer creation until
        // the first render on the GLArea.
        let realize_self = self.clone();

        self.imp().gl_view.connect_realize(move |gl_area| {
            gl_area.make_current();
            if let Some(e) = gl_area.error() {
                realize_self.display_error(e.message());
            }

            // We need to make a glow::context, but we need to give it access
            // to wgl/glx/egl/etcGetProcAddress. GDK does not allow you to ask
            // for extension addresses directly and the developers would much
            // rather boil the ocean getting all their downstreams to use
            // libepoxy, so we have to do this.
            //
            // Also we have to do this AFTER context creation or WGL gets
            // grumpy.
            let mut gl = unsafe {
                glow::Context::from_loader_function_cstr(|symbol| {
                    #[cfg(windows)]
                    {
                        match windows::Win32::Graphics::OpenGL::wglGetProcAddress(
                            windows::core::PCSTR::from_raw(symbol.as_ptr() as *const u8),
                        ) {
                            Some(fun) => fun as *const c_void,
                            None => null::<c_void>(),
                        }
                    }
                    #[cfg(not(windows))]
                    {
                        realize_self.display_error("GL not implemented on this platform");
                        null::<c_void>()
                    }
                })
            };

            // TODO: GLDebug logging
            let renderer = {
                let state = &realize_self.imp().state.borrow();
                OpenglRenderer::new(gl, &state.as_ref().unwrap().document.lock().unwrap().model)
            };

            match renderer {
                Ok(mut renderer) => {
                    renderer.camera.scale.x = 0.15;
                    renderer.camera.scale.y = 0.15;
                    realize_self
                        .imp()
                        .state
                        .borrow_mut()
                        .as_mut()
                        .unwrap()
                        .renderer = Some(renderer)
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

        let render_self = self.clone();
        self.imp().gl_view.connect_render(move |gl_area, context| {
            let mut state_outer = render_self.imp().state.borrow_mut();
            let state = state_outer.as_mut().unwrap();
            let mut document = state.document.lock().unwrap();

            document.model.puppet.begin_frame();
            document.model.puppet.end_frame(1.0);

            let renderer = state.renderer.as_mut().unwrap();
            //TODO: The render trait update branch will impact this code.
            renderer.on_begin_draw(&document.model.puppet);
            renderer.draw(&document.model.puppet);
            renderer.on_end_draw(&document.model.puppet);

            glib::Propagation::Stop
        });
    }
}
