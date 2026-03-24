use glib;
use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use glib::subclass::InitializingObject;

use std::cell::RefCell;
use std::ffi::{CStr, c_void};
use std::num::NonZero;
use std::ptr::null;
use std::sync::{Arc, Mutex};

use gl46;
use glow;
use inox2d::render::InoxRendererExt;
use inox2d_opengl::OpenglRenderer;

use crate::document::Document;

struct State {
    document: Arc<Mutex<Document>>,
    renderer: Option<OpenglRenderer>,
    glfns: Option<gl46::GlFns>,
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

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn lookup_gl_symbol(symbol: &CStr) -> *const c_void {
    #[cfg(windows)]
    {
        match windows::Win32::Graphics::OpenGL::wglGetProcAddress(windows::core::PCSTR::from_raw(
            symbol.as_ptr() as *const u8,
        )) {
            Some(fun) => fun as *const c_void,
            None => null::<c_void>(),
        }
    }
    #[cfg(target_os = "linux")]
    {
        egl::get_proc_address(symbol.to_str().unwrap()) as *const c_void
    }
    #[cfg(all(not(windows), not(target_os = "linux")))]
    {
        eprintln!("GL not implemented on this platform");
        null::<c_void>()
    }
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn lookup_gl_symbol_from_ptr(p: *const u8) -> *const c_void {
    let c_str = std::ffi::CStr::from_ptr(p as *const i8);
    lookup_gl_symbol(c_str) as *mut c_void
}

impl InoxRenderPreview {
    pub fn new(document: Arc<Mutex<Document>>) -> Self {
        let selfish: Self = glib::Object::builder().build();

        *selfish.imp().state.borrow_mut() = Some(State {
            document,
            renderer: None,
            glfns: None,
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

            // We need to make a glow::context, but we need to give it access
            // to wgl/glx/egl/etcGetProcAddress. GDK does not allow you to ask
            // for extension addresses directly and the developers would much
            // rather boil the ocean getting all their downstreams to use
            // libepoxy, so we have to do this.
            //
            // Also we have to do this AFTER context creation or WGL gets
            // grumpy.
            //
            // SAFETY: I have no idea what happens if you give this a bad name
            let (gl, native_gl) = unsafe {
                let gl = glow::Context::from_loader_function_cstr(|p| lookup_gl_symbol(p));
                let stupid_box = Box::new(|p| lookup_gl_symbol_from_ptr(p));
                let native_lookup: &dyn Fn(*const u8) -> *const c_void = &stupid_box;

                let native_gl = gl46::GlFns::load_from(native_lookup).expect("native GL");
                (gl, native_gl)
            };

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

        let render_self = self.clone();
        self.imp().gl_view.connect_render(move |gl_area, _context| {
            if let Some(e) = gl_area.error() {
                render_self.display_error(e.message());
            }

            let mut state_outer = render_self.imp().state.borrow_mut();
            let state = state_outer.as_mut().unwrap();
            let mut document = state.document.lock().unwrap();

            document.model.puppet.begin_frame();
            document.model.puppet.end_frame(1.0);

            let renderer = state.renderer.as_mut().unwrap();
            let native_gl = state.glfns.as_ref().unwrap();

            let mut buffer_id = 0;
            unsafe {
                native_gl.GetIntegerv(gl46::GL_DRAW_FRAMEBUFFER_BINDING, &mut buffer_id);
            }

            // Work around GLArea forgetting to attach the target FB
            if buffer_id == 0 {
                gl_area.attach_buffers();

                unsafe {
                    native_gl.GetIntegerv(gl46::GL_DRAW_FRAMEBUFFER_BINDING, &mut buffer_id);
                }
            }

            unsafe {
                native_gl.ClearColor(0.0, 0.0, 0.0, 1.0);
                native_gl.Clear(gl46::GL_COLOR_BUFFER_BIT);
            }

            renderer.set_surface_framebuffer(
                NonZero::new(buffer_id as u32).map(|b| glow::NativeFramebuffer(b)),
            );

            renderer
                .draw(&document.model.puppet)
                .expect("successful draw");

            unsafe {
                native_gl.BindFramebuffer(gl46::GL_FRAMEBUFFER, buffer_id as u32);
                native_gl.Flush();
            }

            if let Some(e) = gl_area.error() {
                render_self.display_error(e.message());
            }

            glib::Propagation::Proceed
        });
    }
}
