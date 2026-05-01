//! Class structure & trait for subclassing WgpuArea.

use glib::Class;
use glib::translate::from_glib_borrow;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use ningyo_texshare::ExtendedDevice;

use crate::boxed::*;
use crate::widget::class::{WgpuArea, WgpuAreaImp};

/// Trait for method overrides for subclasses of WgpuArea.
pub trait WgpuAreaImpl:
    ObjectImpl + ObjectSubclass<Type: IsA<glib::Object> + IsA<gtk4::Widget>>
{
    /// Alter the device descriptor used during wgpu device creation.
    ///
    /// Necessary in order to request additional WGPU features.
    ///
    /// You may also return a string that will be placed inside the descriptor
    /// as the device's label. Do not populate the label field directly.
    fn preferred_device_descriptor(&self) -> (wgpu::DeviceDescriptor<'static>, glib::GString) {
        (wgpu::DeviceDescriptor::default(), "".into())
    }

    /// Alter the allowed texture usages on the provided render target texture.
    ///
    /// No matter what is returned here, COPY_DST usage is always requested,
    /// as it is necessary for our own internal usages.
    fn preferred_texture_usage(&self) -> wgpu::TextureUsages {
        wgpu::TextureUsages::COPY_DST
    }

    /// Called to inform the WGPU area that it should render.
    fn render(&self) -> glib::Propagation;

    /// Called to inform the WGPU area that the render target has resized.
    ///
    /// Implementations should retain a copy of the target texture and use it
    /// in their render impl as a render target.
    fn resize(&self, target: wgpu::Texture) -> glib::Propagation;
}

unsafe extern "C" fn ng_wgpu_area_preferred_device_descriptor_trampoline<
    T: ObjectSubclass + WgpuAreaImpl,
>(
    ptr: *mut glib::gobject_ffi::GObject,
) -> glib::gobject_ffi::GValue {
    //SAFETY: The returned GValue is owned by the caller.
    unsafe {
        let instance = from_glib_borrow::<_, glib::Object>(ptr);
        glib::Value::from(BoxedWgpuDeviceDescriptor::from(
            instance
                .downcast_ref::<T::Type>()
                .unwrap()
                .imp()
                .preferred_device_descriptor(),
        ))
        .into_raw()
    }
}

unsafe extern "C" fn ng_wgpu_area_preferred_texture_usage_trampoline<
    T: ObjectSubclass + WgpuAreaImpl,
>(
    ptr: *mut glib::gobject_ffi::GObject,
) -> glib::gobject_ffi::GValue {
    //SAFETY: The returned GValue is owned by the caller.
    unsafe {
        let instance = from_glib_borrow::<_, glib::Object>(ptr);
        glib::Value::from(BoxedWgpuTextureUsages::from(
            instance
                .downcast_ref::<T::Type>()
                .unwrap()
                .imp()
                .preferred_texture_usage(),
        ))
        .into_raw()
    }
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
    tex: glib::gobject_ffi::GValue,
) -> bool {
    unsafe {
        let instance = from_glib_borrow::<_, glib::Object>(ptr);
        let texture_value: glib::Value = std::mem::transmute(tex);
        let texture = texture_value.get::<BoxedWgpuTexture>().unwrap();

        instance
            .downcast_ref::<T::Type>()
            .unwrap()
            .imp()
            .resize(texture.0)
            .into()
    }
}

#[repr(C)]
pub struct WgpuAreaClass {
    pub parent_class: gtk4::ffi::GtkWidgetClass,

    pub preferred_device_descriptor:
        Option<unsafe extern "C" fn(*mut glib::gobject_ffi::GObject) -> glib::gobject_ffi::GValue>,
    pub preferred_texture_usage:
        Option<unsafe extern "C" fn(*mut glib::gobject_ffi::GObject) -> glib::gobject_ffi::GValue>,
    pub resize: Option<
        unsafe extern "C" fn(*mut glib::gobject_ffi::GObject, glib::gobject_ffi::GValue) -> bool,
    >,
    pub render: Option<unsafe extern "C" fn(*mut glib::gobject_ffi::GObject) -> bool>,
}

unsafe impl ClassStruct for WgpuAreaClass {
    type Type = WgpuAreaImp;
}

unsafe impl<T: WgpuAreaImpl + WidgetImpl> IsSubclassable<T> for WgpuArea {
    fn class_init(class: &mut Class<Self>) {
        Self::parent_class_init::<T>(class);

        let my_class = class.as_mut();
        my_class.preferred_device_descriptor =
            Some(ng_wgpu_area_preferred_device_descriptor_trampoline::<T>);
        my_class.preferred_texture_usage =
            Some(ng_wgpu_area_preferred_texture_usage_trampoline::<T>);
        my_class.render = Some(ng_wgpu_area_render_trampoline::<T>);
        my_class.resize = Some(ng_wgpu_area_resize_trampoline::<T>);
    }
}

/// A convenience trait that applies to all WgpuArea subclasses.
pub trait WgpuAreaExt {
    /// Signal fired to indicate that the drawing area has been resized and
    /// the widget should allocate and provide a new texture to render.
    fn connect_resize<F: Fn(&Self, wgpu::Texture) -> glib::Propagation + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId;

    fn emit_resize(&self, texture: wgpu::Texture);

    /// Signal fired to indicate that the widget is rendering now and that the
    /// associated texture should be updated or replaced.
    fn connect_render<F: Fn(&Self) -> glib::Propagation + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId;

    fn emit_render(&self);

    fn queue_render(&self);

    /// Retrieve the object's instance.
    ///
    /// This function returns None if the instance has not yet been created. It
    /// is guaranteed to be Some if the widget has already been realized.
    fn instance(&self) -> Option<wgpu::Instance>;

    /// Retrieve the object's adapter.
    ///
    /// This function returns None if the adapter has not yet been created. It
    /// is guaranteed to be Some if the widget has already been realized.
    fn adapter(&self) -> Option<wgpu::Adapter>;

    /// Retrieve the object's device.
    ///
    /// This function returns None if the device has not yet been created. It
    /// is guaranteed to be Some if the widget has already been realized.
    fn device(&self) -> Option<wgpu::Device>;

    /// Retrieve the object's extended device.
    ///
    /// An Extended Device is used to create exportable textures for cross-API
    /// or cross-process texture sharing. See `ningyo_texshare` for more
    /// information.
    ///
    /// This function returns None if the extended device has not yet been
    /// created. It is guaranteed to be Some if the widget has already been
    /// realized.
    fn extended_device(&self) -> Option<ExtendedDevice>;

    /// Retrieve the object's queue.
    ///
    /// This function returns None if the queue has not yet been created. It
    /// is guaranteed to be Some if the widget has already been realized.
    fn queue(&self) -> Option<wgpu::Queue>;

    /// Retrieve the object's texture
    ///
    /// This function returns None if the texture has not yet been created. It
    /// is guaranteed to be Some if the widget has already been realized.
    fn texture(&self) -> Option<wgpu::Texture>;
}

impl<T> WgpuAreaExt for T
where
    T: IsA<WgpuArea>,
{
    fn connect_resize<F: Fn(&Self, wgpu::Texture) -> glib::Propagation + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("resize", false, move |values| {
            let me = values[0].get::<Self>().unwrap();
            let texture = values[1].get::<BoxedWgpuTexture>().unwrap();
            Some(f(&me, texture.into()).into())
        })
    }

    fn emit_resize(&self, texture: wgpu::Texture) {
        let texture = BoxedWgpuTexture::from(texture);
        let texture: glib::Value = texture.into();
        self.emit_by_name::<bool>("resize", &[&texture]);
    }

    fn connect_render<F: Fn(&Self) -> glib::Propagation + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("render", false, move |values| {
            let me = values[0].get::<Self>().unwrap();
            Some(f(&me).into())
        })
    }

    fn emit_render(&self) {
        self.emit_by_name::<bool>("render", &[]);
    }

    fn queue_render(&self) {
        self.clone().upcast::<WgpuArea>().queue_render()
    }

    fn instance(&self) -> Option<wgpu::Instance> {
        self.clone().upcast::<WgpuArea>().instance()
    }

    fn adapter(&self) -> Option<wgpu::Adapter> {
        self.clone().upcast::<WgpuArea>().adapter()
    }

    fn device(&self) -> Option<wgpu::Device> {
        self.clone().upcast::<WgpuArea>().device()
    }

    fn extended_device(&self) -> Option<ExtendedDevice> {
        self.clone().upcast::<WgpuArea>().extended_device()
    }

    fn queue(&self) -> Option<wgpu::Queue> {
        self.clone().upcast::<WgpuArea>().queue()
    }

    fn texture(&self) -> Option<wgpu::Texture> {
        self.clone().upcast::<WgpuArea>().texture()
    }
}
