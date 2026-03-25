use std::ffi::{CStr, c_void};
use std::num::NonZero;
use std::ptr::null;

use gtk4::prelude::*;

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

pub trait GLAreaExt2: GLAreaExt {
    /// Get native GL functions for unsafe calling.
    fn as_native_gl(&self) -> gl46::GlFns;

    /// Create a Glow context for a given GLArea.
    ///
    /// A set of native GL functions are also returned for use with this
    /// GLArea.
    fn as_glow_context(&self) -> glow::Context;

    /// Returns the ID of the GLArea's draw surface.
    fn framebuffer_id(&self, native_gl: &gl46::GlFns) -> i32;

    fn framebuffer(&self, native_gl: &gl46::GlFns) -> glow::NativeFramebuffer;
}

impl<T> GLAreaExt2 for T
where
    T: IsA<gtk4::GLArea>,
{
    fn as_native_gl(&self) -> gl46::GlFns {
        self.make_current();

        unsafe {
            let stupid_box = Box::new(|p| lookup_gl_symbol_from_ptr(p));
            let native_lookup: &dyn Fn(*const u8) -> *const c_void = &stupid_box;

            gl46::GlFns::load_from(native_lookup).expect("native GL")
        }
    }

    fn as_glow_context(&self) -> glow::Context {
        self.make_current();

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
        unsafe { glow::Context::from_loader_function_cstr(|p| lookup_gl_symbol(p)) }
    }

    fn framebuffer_id(&self, native_gl: &gl46::GlFns) -> i32 {
        self.make_current();

        let mut buffer_id = 0;
        unsafe {
            native_gl.GetIntegerv(gl46::GL_DRAW_FRAMEBUFFER_BINDING, &mut buffer_id);
        }

        // Work around GLArea forgetting to attach the target FB
        if buffer_id == 0 {
            self.attach_buffers();

            unsafe {
                native_gl.GetIntegerv(gl46::GL_DRAW_FRAMEBUFFER_BINDING, &mut buffer_id);
            }
        }

        buffer_id
    }

    fn framebuffer(&self, native_gl: &gl46::GlFns) -> glow::NativeFramebuffer {
        NonZero::new(self.framebuffer_id(native_gl) as u32)
            .map(|b| glow::NativeFramebuffer(b))
            .expect("valid GtkGLArea framebuffer")
    }
}
