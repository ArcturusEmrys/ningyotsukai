//! Class structure & trait for subclassing WgpuArea.

use glib::Class;
use glib::translate::from_glib_borrow;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use crate::widget::class::{WgpuArea, WgpuAreaImp};

pub trait WgpuAreaImpl:
    ObjectImpl + ObjectSubclass<Type: IsA<glib::Object> + IsA<gtk4::Widget>>
{
    /// Called to inform the WGPU area that it should render.
    fn render(&self) -> glib::Propagation;

    /// Called to inform the WGPU area that it should allocate and provide a
    /// resized texture.
    fn resize(&self, x: i32, y: i32) -> glib::Propagation;
}

unsafe extern "C" fn ng_wgpu_area_render_trampoline<T: ObjectSubclass + WgpuAreaImpl>(
    ptr: *mut glib::gobject_ffi::GObject,
) -> bool {
    unsafe {
        let instance = from_glib_borrow::<_, glib::Object>(ptr);
        instance
            .downcast_ref::<T::Type>()
            .unwrap()
            .imp()
            .render()
            .into()
    }
}

unsafe extern "C" fn ng_wgpu_area_resize_trampoline<T: ObjectSubclass + WgpuAreaImpl>(
    ptr: *mut glib::gobject_ffi::GObject,
    w: i32,
    h: i32,
) -> bool {
    unsafe {
        let instance = from_glib_borrow::<_, glib::Object>(ptr);
        instance
            .downcast_ref::<T::Type>()
            .unwrap()
            .imp()
            .resize(w, h)
            .into()
    }
}

#[repr(C)]
pub struct WgpuAreaClass {
    pub parent_class: gtk4::ffi::GtkWidgetClass,

    pub resize: Option<unsafe extern "C" fn(*mut glib::gobject_ffi::GObject, i32, i32) -> bool>,
    pub render: Option<unsafe extern "C" fn(*mut glib::gobject_ffi::GObject) -> bool>,
}

unsafe impl ClassStruct for WgpuAreaClass {
    type Type = WgpuAreaImp;
}

unsafe impl<T: WgpuAreaImpl + WidgetImpl> IsSubclassable<T> for WgpuArea {
    fn class_init(class: &mut Class<Self>) {
        Self::parent_class_init::<T>(class);

        let my_class = class.as_mut();
        my_class.render = Some(ng_wgpu_area_render_trampoline::<T>);
        my_class.resize = Some(ng_wgpu_area_resize_trampoline::<T>);
    }
}
