use std::ffi::c_void;
use std::ptr::null_mut;

use crate::texshare::TryIntoGdkTexture;
use gdk4_win32::D3D12TextureBuilder;
use gdk4_win32::ffi;
use glib::translate::{ToGlibPtr, from_glib_full};
use ningyo_texshare::ExportableTexture;

unsafe extern "C" fn ng_gtk_wgpu_into_gdk_texture_destroy(data: glib::ffi::gpointer) {
    let _ = unsafe { Box::from_raw(data as *mut ExportableTexture) };
}

impl TryIntoGdkTexture for ExportableTexture {
    fn into_gdk_texture(
        self,
        _device: &wgpu::Device,
        _display: &gdk4::Display,
        old_texture: Option<gdk4::Texture>,
    ) -> Result<gdk4::Texture, Box<dyn std::error::Error>> {
        let d3d_tex = self.as_d3d_resource()?;
        let d3d_resource = d3d_tex.as_id3d12_resource();
        let builder = D3D12TextureBuilder::new();

        unsafe {
            ffi::gdk_d3d12_texture_builder_set_resource(
                ToGlibPtr::to_glib_none(&builder).0,
                std::mem::transmute(d3d_resource),
            );
        }

        let boxed_self = Box::into_raw(Box::new(self));
        let mut error = null_mut();

        if let Some(old_texture) = old_texture {
            unsafe {
                ffi::gdk_d3d12_texture_builder_set_update_texture(
                    ToGlibPtr::to_glib_none(&builder).0,
                    ToGlibPtr::to_glib_none(&old_texture).0,
                );
            }
        }

        let maybe_builder = unsafe {
            ffi::gdk_d3d12_texture_builder_build(
                ToGlibPtr::to_glib_none(&builder).0,
                Some(ng_gtk_wgpu_into_gdk_texture_destroy),
                boxed_self as *mut c_void,
                &mut error,
            )
        };

        if maybe_builder.is_null() {
            Err(unsafe {
                let g_error: glib::Error = from_glib_full(error);

                g_error
            }
            .into())
        } else {
            Ok(unsafe { from_glib_full(maybe_builder) })
        }
    }
}
