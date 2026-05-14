//! GtkGlArea analog that provides a drawable WGPU surface.

use std::cell::RefCell;
use std::error::Error;
use std::sync::OnceLock;

use glib;
use gtk4;

use glib::subclass::signal::SignalType;
use glib::subclass::{InitializingObject, Signal};
use glib::types::StaticType;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use ningyo_texshare::prelude::*;
use ningyo_texshare::{ExportableTexture, ExtendedDevice};

use pollster::block_on;

use crate::boxed::{BoxedWgpuDeviceDescriptor, BoxedWgpuTexture, BoxedWgpuTextureUsages};
use crate::texshare::TryIntoGdkTexture;
use crate::widget::subclass::{WgpuAreaClass, WgpuAreaExt, WgpuAreaImpl};

#[derive(Default)]
struct WgpuAreaState {
    needs_resize: bool,
    needs_render: bool,
}

#[derive(Default)]
pub struct WgpuAreaImp {
    state: RefCell<WgpuAreaState>,

    wgpu_instance: RefCell<Option<wgpu::Instance>>,
    wgpu_adapter: RefCell<Option<wgpu::Adapter>>,
    wgpu_device: RefCell<Option<ExtendedDevice>>,
    wgpu_queue: RefCell<Option<wgpu::Queue>>,

    /// WGPU texture, double-buffered.
    ///
    /// We absolutely cannot hand out the exportable texture to client code, as
    /// clear operations will fail on it.
    wgpu_texture: RefCell<Option<(wgpu::Texture, ExportableTexture)>>,
    texture: RefCell<Option<gdk4::Texture>>,
}

#[glib::object_subclass]
impl ObjectSubclass for WgpuAreaImp {
    const NAME: &'static str = "NGWgpuArea";
    type Type = WgpuArea;
    type ParentType = gtk4::Widget;
    type Class = WgpuAreaClass;

    fn class_init(class: &mut Self::Class) {
        class.set_css_name("ningyo-wgpuarea");
    }

    fn instance_init(_obj: &InitializingObject<Self>) {}
}

impl ObjectImpl for WgpuAreaImp {
    fn constructed(&self) {
        self.parent_constructed();
    }

    fn signals() -> &'static [glib::subclass::Signal] {
        static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
        SIGNALS.get_or_init(|| {
            vec![
                Signal::builder("render")
                    .return_type::<bool>()
                    .run_last()
                    .class_handler(|args| {
                        let obj = args[0].get::<Self::Type>().unwrap();
                        if let Some(render) = obj.class().as_ref().render {
                            Some(unsafe {
                                render(obj.as_ptr() as *mut glib::gobject_ffi::GObject).into()
                            })
                        } else {
                            None
                        }
                    })
                    .accumulator(|_hint, _accum, retval| {
                        let handled: bool = retval.get().unwrap();

                        if handled {
                            std::ops::ControlFlow::Break(handled.into())
                        } else {
                            std::ops::ControlFlow::Continue(handled.into())
                        }
                    })
                    .build(),
                Signal::builder("resize")
                    .param_types([SignalType::with_static_scope(
                        BoxedWgpuTexture::static_type(),
                    )])
                    .return_type::<bool>()
                    .run_last()
                    .class_handler(|args| {
                        let obj = args[0].get::<Self::Type>().unwrap();
                        let tex = args[1].clone();
                        if let Some(resize) = obj.class().as_ref().resize {
                            Some(unsafe {
                                resize(
                                    obj.as_ptr() as *mut glib::gobject_ffi::GObject,
                                    std::mem::transmute(tex),
                                )
                                .into()
                            })
                        } else {
                            None
                        }
                    })
                    .build(),
            ]
        })
    }
}

impl WidgetImpl for WgpuAreaImp {
    fn realize(&self) {
        self.parent_realize();

        let me = self.obj().clone();
        block_on((async move || WgpuAreaImp::create_instance(me).await)()).unwrap();
    }

    fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
        self.parent_size_allocate(width, height, baseline);
        self.state.borrow_mut().needs_resize = true;
        self.obj().queue_draw();
    }

    fn snapshot(&self, snapshot: &gtk4::Snapshot) {
        let needs_resize = self.state.borrow().needs_resize;
        let needs_render = self.state.borrow().needs_render;

        if needs_render {
            if needs_resize {
                if let (Some(adapter), Some(device), Some(queue)) = (
                    &*self.wgpu_adapter.borrow(),
                    &*self.wgpu_device.borrow(),
                    &*self.wgpu_queue.borrow(),
                ) {
                    let size = wgpu::Extent3d {
                        width: self.obj().width() as u32 * self.obj().scale_factor() as u32,
                        height: self.obj().height() as u32 * self.obj().scale_factor() as u32,
                        depth_or_array_layers: 1,
                    };
                    let backing_texture = device
                        .create_texture_exportable(
                            adapter,
                            queue,
                            &wgpu::TextureDescriptor {
                                size,
                                mip_level_count: 1,
                                sample_count: 1,
                                dimension: wgpu::TextureDimension::D2,

                                //NOTE: Selected to match GDK's requirements.
                                //If this is not BGRA8, GDK will do a CPU side
                                //texture conversion!!
                                format: wgpu::TextureFormat::Bgra8Unorm,
                                usage: self.obj().preferred_texture_usages()
                                    | wgpu::TextureUsages::COPY_DST,
                                label: Some("NGWgpuArea backing texture"),
                                view_formats: &[],
                            },
                        )
                        .expect("Exported texture");
                    let buffer_texture = device.device().create_texture(&wgpu::TextureDescriptor {
                        size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,

                        //NOTE: Likewise, this needs to match the backing
                        //texture or the copy fails
                        format: wgpu::TextureFormat::Bgra8Unorm,
                        usage: self.obj().preferred_texture_usages()
                            | wgpu::TextureUsages::COPY_SRC,
                        label: Some("NGWgpuArea buffer texture"),
                        view_formats: &[],
                    });

                    *self.wgpu_texture.borrow_mut() = Some((buffer_texture, backing_texture));
                }

                //NOTE: We specifically give the subclass the buffer texture
                //so we can copy to the exportable one.
                self.obj()
                    .emit_resize(self.wgpu_texture.borrow().as_ref().unwrap().0.clone());
                self.state.borrow_mut().needs_resize = false;
            }

            // TODO: Add option to disable the implicit clear.
            // TODO: Add option to render directly to the backing texture, with
            // the restriction that clearing the texture crashes your program.
            {
                let device = self.wgpu_device.borrow();
                let device = device.as_ref().unwrap();
                let queue = self.wgpu_queue.borrow();
                let queue = queue.as_ref().unwrap();

                let texture = self.wgpu_texture.borrow();
                let texture = texture.as_ref().unwrap();
                let (buffer_texture, _backing_texture) = texture;

                let mut encoder =
                    device
                        .device()
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("NGWgpuArea internal buffer clear"),
                        });

                encoder.clear_texture(
                    buffer_texture,
                    &wgpu::ImageSubresourceRange {
                        aspect: wgpu::TextureAspect::All,
                        base_mip_level: 0,
                        mip_level_count: None,
                        base_array_layer: 0,
                        array_layer_count: None,
                    },
                );

                queue.submit(std::iter::once(encoder.finish()));
            }

            self.obj().emit_render();

            {
                let device = self.wgpu_device.borrow();
                let device = device.as_ref().unwrap();
                let queue = self.wgpu_queue.borrow();
                let queue = queue.as_ref().unwrap();

                device
                    .device()
                    .poll(wgpu::PollType::Wait {
                        submission_index: None,
                        timeout: None,
                    })
                    .unwrap();

                let texture = self.wgpu_texture.borrow();
                let texture = texture.as_ref().unwrap();
                let (buffer_texture, backing_texture) = texture;

                let mut encoder =
                    device
                        .device()
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("NGWgpuArea internal copy to backing texture"),
                        });

                encoder.copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: buffer_texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyTextureInfo {
                        texture: backing_texture.texture(),
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d {
                        width: buffer_texture.width(),
                        height: buffer_texture.height(),
                        depth_or_array_layers: 1,
                    },
                );

                queue.submit(std::iter::once(encoder.finish()));

                device
                    .device()
                    .poll(wgpu::PollType::Wait {
                        submission_index: None,
                        timeout: None,
                    })
                    .unwrap();
            }

            let old_texture = self.texture.borrow().clone();
            let texture = self
                .wgpu_texture
                .borrow()
                .as_ref()
                .unwrap()
                .1
                .clone()
                .into_gdk_texture(
                    &self.wgpu_device.borrow().as_ref().unwrap().device(),
                    &self.obj().display(),
                    old_texture,
                )
                .expect("working gdk4 import");

            *self.texture.borrow_mut() = Some(texture);
        }

        if let Some(ref texture) = *self.texture.borrow() {
            snapshot.append_texture(
                texture,
                &graphene::Rect::new(
                    0.0,
                    0.0,
                    self.obj().width() as f32,
                    self.obj().height() as f32,
                ),
            );
        }
    }
}

impl WgpuAreaImpl for WgpuAreaImp {
    fn render(&self) -> glib::Propagation {
        glib::Propagation::Stop
    }

    fn resize(&self, _texture: wgpu::Texture) -> glib::Propagation {
        glib::Propagation::Stop
    }
}

impl WgpuAreaImp {
    async fn create_instance(me: WgpuArea) -> Result<(), Box<dyn Error>> {
        #[allow(unused)]
        let mut instance_desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();

        let instance = wgpu::Instance::new_with_extensions(instance_desc)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                ..Default::default()
            })
            .await?;
        let (dd, label) = me.preferred_device_descriptor();
        let dd_label = wgpu::DeviceDescriptor {
            label: Some(&label),
            ..dd
        };
        let (device, queue) = adapter.request_device_with_extensions(&dd_label).await?;

        *me.imp().wgpu_instance.borrow_mut() = Some(instance);
        *me.imp().wgpu_adapter.borrow_mut() = Some(adapter);
        *me.imp().wgpu_device.borrow_mut() = Some(device);
        *me.imp().wgpu_queue.borrow_mut() = Some(queue);

        Ok(())
    }
}

glib::wrapper! {
    pub struct WgpuArea(ObjectSubclass<WgpuAreaImp>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl WgpuArea {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Retrieve the WgpuArea's preferred device features.
    ///
    /// This may be overridden by subclasses in the WgpuAreaImpl trait.
    fn preferred_device_descriptor(&self) -> (wgpu::DeviceDescriptor<'static>, String) {
        if let Some(add) = self.class().as_ref().preferred_device_descriptor {
            let value: glib::Value = unsafe {
                //glib::Value doesn't seem to have an "owned Gvalue" case
                //so lets transmute it lol
                std::mem::transmute(add(self.as_ptr() as *mut glib::gobject_ffi::GObject))
            };

            let boxed_dd = value.get::<BoxedWgpuDeviceDescriptor>().unwrap();

            (boxed_dd.0, boxed_dd.1.to_string())
        } else {
            (wgpu::DeviceDescriptor::default(), "".to_string())
        }
    }

    /// Retrieve the WgpuArea's preferred texture usages.
    ///
    /// This may be overridden by subclasses in the WgpuAreaImpl trait.
    fn preferred_texture_usages(&self) -> wgpu::TextureUsages {
        if let Some(add) = self.class().as_ref().preferred_texture_usage {
            let value: glib::Value = unsafe {
                //glib::Value doesn't seem to have an "owned Gvalue" case
                //so lets transmute it lol
                std::mem::transmute(add(self.as_ptr() as *mut glib::gobject_ffi::GObject))
            };

            let tu: wgpu::TextureUsages = value.get::<BoxedWgpuTextureUsages>().unwrap().into();

            tu
        } else {
            wgpu::TextureUsages::empty()
        }
    }

    pub fn queue_render(&self) {
        self.imp().state.borrow_mut().needs_render = true;
        self.queue_draw();
    }

    /// Retrieve the object's instance.
    ///
    /// This function returns None if the instance has not yet been created.
    pub fn instance(&self) -> Option<wgpu::Instance> {
        (*self.imp().wgpu_instance.borrow()).clone()
    }

    /// Retrieve the object's adapter.
    ///
    /// This function returns None if the adapter has not yet been created.
    pub fn adapter(&self) -> Option<wgpu::Adapter> {
        (*self.imp().wgpu_adapter.borrow()).clone()
    }

    /// Retrieve the object's device.
    ///
    /// This function returns None if the device has not yet been created.
    pub fn device(&self) -> Option<wgpu::Device> {
        let data = self.imp().wgpu_device.borrow();
        if let Some(ref device) = *data {
            Some(device.device().clone())
        } else {
            None
        }
    }

    /// Retrieve the object's device, with texture-sharing extensions.
    ///
    /// This function returns None if the device has not yet been created.
    pub fn extended_device(&self) -> Option<ExtendedDevice> {
        (*self.imp().wgpu_device.borrow()).clone()
    }

    /// Retrieve the object's queue.
    ///
    /// This function returns None if the queue has not yet been created.
    pub fn queue(&self) -> Option<wgpu::Queue> {
        (*self.imp().wgpu_queue.borrow()).clone()
    }

    /// Retrieve the current texture to draw to.
    ///
    /// This is equivalent to the last texture that was sent to resize.
    pub fn texture(&self) -> Option<wgpu::Texture> {
        if let Some((buffer_texture, _backing_texture)) = &*self.imp().wgpu_texture.borrow() {
            Some(buffer_texture.clone())
        } else {
            None
        }
    }
}
