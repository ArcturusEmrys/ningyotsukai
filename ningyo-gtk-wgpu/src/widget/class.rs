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
    in_async_render: bool,
    async_render_complete: bool,

    wgpu_instance: Option<wgpu::Instance>,
    wgpu_adapter: Option<wgpu::Adapter>,
    wgpu_device: Option<ExtendedDevice>,
    wgpu_queue: Option<wgpu::Queue>,

    /// WGPU texture, double-buffered.
    ///
    /// We absolutely cannot hand out the exportable texture to client code, as
    /// clear operations will fail on it.
    wgpu_texture: Option<(wgpu::Texture, ExportableTexture)>,
    texture: Option<gdk4::Texture>,

    use_backing_texture: bool,
}

#[derive(Default)]
pub struct WgpuAreaImp {
    state: RefCell<WgpuAreaState>,
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
        let (
            needs_resize,
            needs_render,
            in_async_render,
            async_render_complete,
            adapter,
            device,
            queue,
        ) = {
            let state_ref = self.state.borrow();
            let state = &*state_ref;
            (
                state.needs_resize,
                state.needs_render,
                state.in_async_render,
                state.async_render_complete,
                state.wgpu_adapter.clone().unwrap(),
                state.wgpu_device.clone().unwrap(),
                state.wgpu_queue.clone().unwrap(),
            )
        };

        if needs_render {
            if !in_async_render {
                if needs_resize {
                    let size = wgpu::Extent3d {
                        width: self.obj().width() as u32 * self.obj().scale_factor() as u32,
                        height: self.obj().height() as u32 * self.obj().scale_factor() as u32,
                        depth_or_array_layers: 1,
                    };

                    let format = if cfg!(target_os = "windows") {
                        // NOTE: This specifically matches GDK requirements.
                        // If this is not BGRA8, GDK-Win32 does CPU-side
                        // texture conversions!!
                        wgpu::TextureFormat::Bgra8Unorm
                    } else {
                        wgpu::TextureFormat::Rgba8Unorm
                    };

                    let backing_texture = device
                        .create_texture_exportable(
                            &adapter,
                            &queue,
                            &wgpu::TextureDescriptor {
                                size,
                                mip_level_count: 1,
                                sample_count: 1,
                                dimension: wgpu::TextureDimension::D2,
                                format,
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
                        format,
                        usage: self.obj().preferred_texture_usages()
                            | wgpu::TextureUsages::COPY_SRC,
                        label: Some("NGWgpuArea buffer texture"),
                        view_formats: &[],
                    });

                    self.state.borrow_mut().wgpu_texture =
                        Some((buffer_texture.clone(), backing_texture.clone()));

                    //NOTE: We specifically give the subclass the buffer texture
                    //so we can copy to the exportable one.
                    self.obj()
                        .emit_resize(if self.state.borrow().use_backing_texture {
                            backing_texture.texture().clone()
                        } else {
                            buffer_texture
                        });
                    self.state.borrow_mut().needs_resize = false;
                }
            }
        }

        if needs_render {
            if !in_async_render {
                let did_complete = self.obj().emit_render();
                if did_complete == glib::ControlFlow::Break {
                    //Async rendering in progress.
                    //Do nothing until we hear back from the subclass.
                    self.state.borrow_mut().in_async_render = true;
                    self.state.borrow_mut().async_render_complete = false;
                } else {
                    //We only poll the GPU in the sync path
                    device
                        .device()
                        .poll(wgpu::PollType::Wait {
                            submission_index: None,
                            timeout: None,
                        })
                        .unwrap();
                }
            }

            if !in_async_render || (in_async_render && async_render_complete) {
                self.state.borrow_mut().needs_render = false;
                if in_async_render && async_render_complete {
                    self.state.borrow_mut().in_async_render = false;
                    self.state.borrow_mut().async_render_complete = false;
                }
                let ((buffer_texture, backing_texture), old_gdk_texture, use_backing_texture) = {
                    let state = self.state.borrow();
                    (
                        state.wgpu_texture.clone().unwrap(),
                        state.texture.clone(),
                        state.use_backing_texture,
                    )
                };

                if !use_backing_texture {
                    let mut encoder =
                        device
                            .device()
                            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("NGWgpuArea internal copy to backing texture"),
                            });

                    encoder.copy_texture_to_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &buffer_texture,
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

                    let index = queue.submit(std::iter::once(encoder.finish()));

                    device
                        .device()
                        .poll(wgpu::PollType::Wait {
                            submission_index: Some(index),
                            timeout: None,
                        })
                        .unwrap();
                }

                let texture = backing_texture
                    .clone()
                    .into_gdk_texture(&device.device(), &self.obj().display(), old_gdk_texture)
                    .expect("working gdk4 import");

                self.state.borrow_mut().texture = Some(texture);
            }
        }

        if let Some(ref texture) = self.state.borrow_mut().texture {
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
    fn render(&self) -> glib::ControlFlow {
        glib::ControlFlow::Break
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

        let mut state = me.imp().state.borrow_mut();

        state.wgpu_instance = Some(instance);
        state.wgpu_adapter = Some(adapter);
        state.wgpu_device = Some(device);
        state.wgpu_queue = Some(queue);

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

    pub fn async_render_complete(&self) {
        let mut state = self.imp().state.borrow_mut();

        if state.in_async_render {
            state.needs_render = true;
            state.async_render_complete = true;
            self.queue_draw();
        }
    }

    /// Retrieve the object's instance.
    ///
    /// This function returns None if the instance has not yet been created.
    pub fn instance(&self) -> Option<wgpu::Instance> {
        self.imp().state.borrow().wgpu_instance.clone()
    }

    /// Retrieve the object's adapter.
    ///
    /// This function returns None if the adapter has not yet been created.
    pub fn adapter(&self) -> Option<wgpu::Adapter> {
        self.imp().state.borrow().wgpu_adapter.clone()
    }

    /// Retrieve the object's device.
    ///
    /// This function returns None if the device has not yet been created.
    pub fn device(&self) -> Option<wgpu::Device> {
        self.imp()
            .state
            .borrow()
            .wgpu_device
            .as_ref()
            .map(|d| d.device().clone())
    }

    /// Retrieve the object's device, with texture-sharing extensions.
    ///
    /// This function returns None if the device has not yet been created.
    pub fn extended_device(&self) -> Option<ExtendedDevice> {
        self.imp().state.borrow().wgpu_device.clone()
    }

    /// Retrieve the object's queue.
    ///
    /// This function returns None if the queue has not yet been created.
    pub fn queue(&self) -> Option<wgpu::Queue> {
        self.imp().state.borrow().wgpu_queue.clone()
    }

    /// Retrieve the current texture to draw to.
    ///
    /// This is equivalent to the last texture that was sent to resize.
    ///
    /// If the user has enabled `render_to_backing_texture`, this yields the
    /// backing texture, for consistency.
    pub fn texture(&self) -> Option<wgpu::Texture> {
        let state = self.imp().state.borrow();
        if let Some((buffer_texture, backing_texture)) = state.wgpu_texture.as_ref() {
            if state.use_backing_texture {
                Some(backing_texture.texture().clone())
            } else {
                Some(buffer_texture.clone())
            }
        } else {
            None
        }
    }

    /// Enable direct rendering to the widget's backing texture.
    ///
    /// WgpuArea normally provides an internal buffer texture for client code
    /// to render to. This buffer texture is then copied to a specially
    /// allocated shared backing texture and provided to GTK.
    ///
    /// When this setting is enabled, texture() returns the backing texture,
    /// and the buffer-to-backing texture copy is skipped.
    ///
    /// The backing texture is allocated with a specific configuration that
    /// does not permit the use of all valid WGPU operations. Issuing a render
    /// command that is incompatible with this texture will panic your program.
    /// Notably, the backing texture cannot be cleared using the usual
    /// `Encoder.clear_texture()` command. Normal polygon drawing and texture
    /// copies will work.
    pub fn render_to_backing_texture(&self, use_backing_texture: bool) {
        self.imp().state.borrow_mut().use_backing_texture = use_backing_texture;
    }
}
