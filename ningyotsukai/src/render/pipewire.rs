use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::mem::forget;
use std::ops::Range;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::rc::Rc;
use std::slice;
use std::thread::spawn;

use libspa::pod::Pod;
use libspa::pod::builder::Builder;
use libspa::sys;
use libspa::utils::{Direction, Id};

use pipewire::channel::{Receiver, Sender, channel};
use pipewire::context::ContextRc;
use pipewire::core::CoreRc;
use pipewire::keys;
use pipewire::main_loop::MainLoopRc;
use pipewire::properties::properties;
use pipewire::stream::{StreamFlags, StreamListener, StreamRc};

use glam::Vec2;

use ningyo_render_wgpu::WgpuRenderer;
use ningyo_texshare::prelude::*;

use crate::document::Document;
use crate::render::SinkPlugin;

/// Guess what? We had to fork our own copy of the builder macro!
#[macro_export]
macro_rules! builder_add {
    ($builder:expr, None) => {
        libspa::pod::builder::Builder::add_none($builder)
    };
    ($builder:expr, Bool($val:expr)) => {
        libspa::pod::builder::Builder::add_bool($builder, $val)
    };
    ($builder:expr, Id($val:expr)) => {
        libspa::pod::builder::Builder::add_id($builder, $val)
    };
    ($builder:expr, Int($val:expr)) => {
        libspa::pod::builder::Builder::add_int($builder, $val)
    };
    ($builder:expr, Long($val:expr)) => {
        libspa::pod::builder::Builder::add_long($builder, $val)
    };
    ($builder:expr, Float($val:expr)) => {
        libspa::pod::builder::Builder::add_float($builder, $val)
    };
    ($builder:expr, Double($val:expr)) => {
        libspa::pod::builder::Builder::add_double($builder, $val)
    };
    ($builder:expr, String($val:expr)) => {
        libspa::pod::builder::Builder::add_string($builder, $val)
    };
    ($builder:expr, Bytes($val:expr)) => {
        libspa::pod::builder::Builder::add_bytes($builder, $val)
    };
    ($builder:expr, Pointer($type_:expr, $val:expr)) => {
        libspa::pod::builder::Builder::add_bool($builder, $type_, $val)
    };
    ($builder:expr, Fd($val:expr)) => {
        libspa::pod::builder::Builder::add_fd($builder, $val)
    };
    ($builder:expr, Rectangle($val:expr)) => {
        libspa::pod::builder::Builder::add_rectangle($builder, $val)
    };
    ($builder:expr, Fraction($val:expr)) => {
        libspa::pod::builder::Builder::add_fraction($builder, $val)
    };
    (
        $builder:expr,
        ChoiceNone($flags:expr, $value_type:tt $value:tt)
    ) => {
        'outer: {
            let mut frame: ::std::mem::MaybeUninit<libspa::sys::spa_pod_frame> = ::std::mem::MaybeUninit::uninit();
            let res = unsafe { libspa::pod::builder::Builder::push_choice($builder, &mut frame, libspa::sys::SPA_CHOICE_None, $flags) };
            if res.is_err() {
                break 'outer res;
            }

            let res = builder_add!($builder, $value_type $value);
            if res.is_err() {
                break 'outer res;
            }

            unsafe { libspa::pod::builder::Builder::pop($builder, frame.assume_init_mut()) }

            Ok(())
        }
    };
    (
        $builder:expr,
        ChoiceFlags($value_type:tt $value:tt)
    ) => {
        'outer: {
            let mut frame: ::std::mem::MaybeUninit<libspa::sys::spa_pod_frame> = ::std::mem::MaybeUninit::uninit();
            let res = unsafe { libspa::pod::builder::Builder::push_choice($builder, &mut frame, libspa::sys::SPA_CHOICE_Flags, 0) };
            if res.is_err() {
                break 'outer res;
            }

            let res = builder_add!($builder, $value_type $value);
            if res.is_err() {
                break 'outer res;
            }

            unsafe { libspa::pod::builder::Builder::pop($builder, frame.assume_init_mut()) }

            Ok(())
        }
    };
    (
        $builder:expr,
        ChoiceEnum($flags:expr, $( $value_type:tt $value:tt ),* $(,)?)
    ) => {
        'outer: {
            let mut frame: ::std::mem::MaybeUninit<libspa::sys::spa_pod_frame> = ::std::mem::MaybeUninit::uninit();
            let res = unsafe { libspa::pod::builder::Builder::push_choice($builder, &mut frame, libspa::sys::SPA_CHOICE_Enum, $flags) };
            if res.is_err() {
                break 'outer res;
            }

            $(
                let res = builder_add!($builder, $value_type $value);
                if res.is_err() {
                    break 'outer res;
                }
            )*

            unsafe { libspa::pod::builder::Builder::pop($builder, frame.assume_init_mut()) }

            Ok(())
        }
    };
    (
        $builder:expr,
        Struct {
            $( $field_type:tt $field:tt ),* $(,)?
        }
    ) => {
        'outer: {
            let mut frame: ::std::mem::MaybeUninit<$libspa::sys::spa_pod_frame> = ::std::mem::MaybeUninit::uninit();
            let res = unsafe { libspa::pod::builder::Builder::push_struct($builder, &mut frame) };
            if res.is_err() {
                break 'outer res;
            }

            $(
                let res = libspa::__builder_add__!($builder, $field_type $field);
                if res.is_err() {
                    break 'outer res;
                }
            )*

            unsafe { libspa::pod::builder::Builder::pop($builder, frame.assume_init_mut()) }

            Ok(())
        }
    };
    (
        $builder:expr,
        Object($type_:expr, $id:expr $(,)?) {
            $( $key:expr => $value_type:tt $value:tt ($flags:expr) ),* $(,)?
        }
    ) => {
        'outer: {
            let mut frame: ::std::mem::MaybeUninit<libspa::sys::spa_pod_frame> = ::std::mem::MaybeUninit::uninit();
            let res = unsafe { libspa::pod::builder::Builder::push_object($builder, &mut frame, $type_, $id) };
            if res.is_err() {
                break 'outer res;
            }

            $(
                let res = libspa::pod::builder::Builder::add_prop($builder, $key, $flags);
                if res.is_err() {
                    break 'outer res;
                }
                let res = builder_add!($builder, $value_type $value);
                if res.is_err() {
                    break 'outer res;
                }
            )*

            unsafe { libspa::pod::builder::Builder::pop($builder, frame.assume_init_mut()) }

            Ok(())
        }
    };
    // TODO: Sequence
    // TODO: Control
}

/// Ensure that byte data in a vec is aligned to a specific alignment.
fn ensure_alignment(data: &mut Vec<u8>, align: usize) -> Range<usize> {
    let pod_len = data.len();

    data.append(&mut vec![0; align]);

    let misalignment = data.as_ptr() as usize % align;
    let realignment = (align - misalignment) % align;
    if realignment != 0 {
        let from_range = 0..pod_len;

        data.copy_within(from_range, realignment);
    }

    realignment..(pod_len + realignment)
}

pub enum PipewireMessage {
    Shutdown,
    PublishStream {
        document: Document,
        name: String,
        size: glam::Vec2,
        framerate: (u32, u32),
    },
    UpdateStreamImage {
        document: Document,
        texture: wgpu::Texture,
    },
}

#[derive(Debug)] //needs to be debug or we can't unwrap
pub enum PipewireResponse {
    Ack,
}

pub struct OwnedPod(Vec<u8>, Range<usize>);

impl OwnedPod {
    fn from_data(mut data: Vec<u8>) -> Self {
        let to_range = ensure_alignment(&mut data, 8);

        OwnedPod(data, to_range)
    }

    fn as_pod(&self) -> Option<&Pod> {
        Pod::from_bytes(&self.0[self.1.clone()])
    }
}

pub struct PipewireStream {
    _stream: StreamRc,
    _listener: StreamListener<()>,
    last_tex: Option<wgpu::Texture>,
    copy_buffer: Option<wgpu::Buffer>,
}

#[derive(Clone)]
struct PipewireThread(Rc<RefCell<PipewireThreadInner>>);
struct PipewireThreadInner {
    mainloop: MainLoopRc,
    core: CoreRc,
    streams: HashMap<Document, PipewireStream>,
    sender: Sender<PipewireResponse>,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl PipewireThread {
    fn run(
        receiver: Receiver<PipewireMessage>,
        sender: Sender<PipewireResponse>,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) {
        let mainloop = MainLoopRc::new(None).expect("Pipewire Mainloop");
        let context = ContextRc::new(&mainloop, None).unwrap();
        let core = context.connect_rc(None).unwrap();

        let me = PipewireThread(Rc::new(RefCell::new(PipewireThreadInner {
            mainloop: mainloop.clone(),
            core,
            streams: HashMap::new(),
            sender,
            adapter,
            device,
            queue,
        })));

        let _recv_handle = receiver.attach(mainloop.loop_(), {
            let me = me.clone();
            move |msg| {
                me.process_message(msg);
            }
        });

        mainloop.run();
    }

    fn process_message(&self, msg: PipewireMessage) {
        match msg {
            PipewireMessage::PublishStream {
                document,
                name,
                size,
                framerate,
            } => {
                self.publish_stream(document, name, size, framerate);
                let state = self.0.borrow();
                state.sender.send(PipewireResponse::Ack).unwrap();
            }
            PipewireMessage::UpdateStreamImage { document, texture } => {
                self.update_stream_image(document.clone(), texture);
                let state = self.0.borrow();
                state.sender.send(PipewireResponse::Ack).unwrap();
            }
            PipewireMessage::Shutdown => {
                let state = self.0.borrow();
                state.mainloop.quit();
            }
        }
    }

    fn create_dmabuf_texture(&self, size: Vec2) -> (wgpu::Texture, u64, u64, u64, OwnedFd) {
        let state = self.0.borrow_mut();
        let texture = state
            .device
            .create_texture_exportable(
                &state.adapter,
                &state.queue,
                &wgpu::TextureDescriptor {
                    label: Some("Offscreen Texture Buffer"),
                    dimension: wgpu::TextureDimension::D2,
                    size: wgpu::Extent3d {
                        width: size.x as u32,
                        height: size.y as u32,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: WgpuRenderer::required_render_target_uses()
                        | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                },
            )
            .unwrap();

        let mut encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Pipewire internal buffer clear"),
            });

        let view = texture.texture().create_view(&wgpu::TextureViewDescriptor {
            label: Some("Annoying view descriptor for internal clear pass"),
            format: Some(wgpu::TextureFormat::Rgba8Unorm),
            dimension: Some(wgpu::TextureViewDimension::D2),
            usage: Some(texture.texture().usage()),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Pipewire internal clear pass"),
            depth_stencil_attachment: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });

        state.queue.submit(std::iter::once(encoder.finish()));

        let datasize = texture.data_size();
        let mut row_stride = texture.row_stride();
        if row_stride == 0 {
            row_stride = datasize / size.y as u64;
        }

        let dmabuf = texture.as_dmabuf(&state.device).unwrap();
        let modifier = dmabuf.modifier();
        let (texture, fd) = dmabuf.into_fd();

        (texture, datasize, row_stride, modifier, fd)
    }

    fn publish_stream(&self, document: Document, name: String, size: Vec2, framerate: (u32, u32)) {
        let (_, datasize, row_stride, modifier, _) = self.create_dmabuf_texture(size);
        let mut download_row_stride = 4 * size.x as u64;
        let misalign = download_row_stride % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as u64;
        if misalign != 0 {
            download_row_stride += wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as u64 - misalign;
        }

        let mut state = self.0.borrow_mut();
        let stream = StreamRc::new(
            state.core.clone(),
            &name,
            properties! {
                *keys::MEDIA_TYPE => "Video",
                *keys::MEDIA_CATEGORY => "Capture",
                *keys::MEDIA_ROLE => "Camera",
                *keys::MEDIA_CLASS => "Video/Source",
                *keys::NODE_NAME => "Ningyotsukai Puppet",
                *keys::NODE_DESCRIPTION => "Ningyotsukai Virtual Puppet"
            },
        )
        .unwrap();

        let format_with_modifier = {
            let mut data = vec![];
            let mut builder = Builder::new(&mut data);

            builder_add!(&mut builder, Object(sys::SPA_TYPE_OBJECT_Format, sys::SPA_PARAM_EnumFormat) {
                sys::SPA_FORMAT_mediaType => Id(Id(sys::SPA_MEDIA_TYPE_video)) (0),
                sys::SPA_FORMAT_mediaSubtype => Id(Id(sys::SPA_MEDIA_SUBTYPE_raw)) (0),
                sys::SPA_FORMAT_VIDEO_format => Id(Id(sys::SPA_VIDEO_FORMAT_RGBA)) (0),
                sys::SPA_FORMAT_VIDEO_size => Rectangle(sys::spa_rectangle { width: size.x as u32, height: size.y as u32 }) (0),
                sys::SPA_FORMAT_VIDEO_framerate => Fraction(sys::spa_fraction { num: framerate.0, denom: framerate.1 }) (0),
                sys::SPA_FORMAT_VIDEO_modifier => ChoiceEnum(0, Long(modifier as i64), Long(u64::from(drm_fourcc::DrmModifier::Invalid) as i64)) (sys::SPA_POD_PROP_FLAG_MANDATORY | sys::SPA_POD_PROP_FLAG_DONT_FIXATE),
            }).unwrap();

            OwnedPod::from_data(data)
        };

        let format_no_modifier = {
            let mut data = vec![];
            let mut builder = Builder::new(&mut data);

            builder_add!(&mut builder, Object(sys::SPA_TYPE_OBJECT_Format, sys::SPA_PARAM_EnumFormat) {
                sys::SPA_FORMAT_mediaType => Id(Id(sys::SPA_MEDIA_TYPE_video)) (0),
                sys::SPA_FORMAT_mediaSubtype => Id(Id(sys::SPA_MEDIA_SUBTYPE_raw)) (0),
                sys::SPA_FORMAT_VIDEO_format => Id(Id(sys::SPA_VIDEO_FORMAT_RGBA)) (0),
                sys::SPA_FORMAT_VIDEO_size => Rectangle(sys::spa_rectangle { width: size.x as u32, height: size.y as u32 }) (0),
                sys::SPA_FORMAT_VIDEO_framerate => Fraction(sys::spa_fraction { num: framerate.0, denom: framerate.1 }) (0)
            }).unwrap();

            OwnedPod::from_data(data)
        };

        let listener = stream.add_local_listener::<()>().param_changed({
            move |stream, _data, _id, pod| {
                if let Some(pod) = pod { //NOTE: call spa_debug_pod if you want to dump this.
                    if let Ok(pod_object) = pod.as_object() {
                        let specified_dmabuf_modifier = pod_object.find_prop(Id(sys::SPA_FORMAT_VIDEO_modifier)).is_some();
                        let data_type = 1 << sys::SPA_DATA_MemPtr | 1 << sys::SPA_DATA_DmaBuf;

                        let format = {
                            let mut data = vec![];
                            let mut builder = Builder::new(&mut data);

                            if specified_dmabuf_modifier { //TODO: check if they asked for a different modifier
                                builder_add!(&mut builder, Object(sys::SPA_TYPE_OBJECT_Format, sys::SPA_PARAM_Format) {
                                    sys::SPA_FORMAT_mediaType => Id(Id(sys::SPA_MEDIA_TYPE_video)) (0),
                                    sys::SPA_FORMAT_mediaSubtype => Id(Id(sys::SPA_MEDIA_SUBTYPE_raw)) (0),
                                    sys::SPA_FORMAT_VIDEO_format => Id(Id(sys::SPA_VIDEO_FORMAT_RGBA)) (0),
                                    sys::SPA_FORMAT_VIDEO_size => Rectangle(sys::spa_rectangle { width: size.x as u32, height: size.y as u32 }) (0),
                                    sys::SPA_FORMAT_VIDEO_framerate => Fraction(sys::spa_fraction { num: framerate.0, denom: framerate.1 }) (0),
                                    sys::SPA_FORMAT_VIDEO_modifier => Long(modifier as i64) (0),
                                }).unwrap();
                            } else {
                                builder_add!(&mut builder, Object(sys::SPA_TYPE_OBJECT_Format, sys::SPA_PARAM_Format) {
                                    sys::SPA_FORMAT_mediaType => Id(Id(sys::SPA_MEDIA_TYPE_video)) (0),
                                    sys::SPA_FORMAT_mediaSubtype => Id(Id(sys::SPA_MEDIA_SUBTYPE_raw)) (0),
                                    sys::SPA_FORMAT_VIDEO_format => Id(Id(sys::SPA_VIDEO_FORMAT_RGBA)) (0),
                                    sys::SPA_FORMAT_VIDEO_size => Rectangle(sys::spa_rectangle { width: size.x as u32, height: size.y as u32 }) (0),
                                    sys::SPA_FORMAT_VIDEO_framerate => Fraction(sys::spa_fraction { num: framerate.0, denom: framerate.1 }) (0),
                                }).unwrap();
                            }

                            OwnedPod::from_data(data)
                        };

                        let buffer_format = {
                            let mut data = vec![];
                            let mut builder = Builder::new(&mut data);

                            builder_add!(&mut builder, Object (
                                sys::SPA_TYPE_OBJECT_ParamBuffers,
                                sys::SPA_PARAM_Buffers) {
                                    sys::SPA_PARAM_BUFFERS_buffers => Int(3) (0),
                                    sys::SPA_PARAM_BUFFERS_dataType => ChoiceFlags(Int(data_type)) (0),
                                    sys::SPA_PARAM_BUFFERS_blocks => Int(1) (0),
                                    sys::SPA_PARAM_BUFFERS_size => Int(datasize as i32) (0),
                                    sys::SPA_PARAM_BUFFERS_stride => Int(row_stride as i32) (0),
                                }
                            ).unwrap();

                            OwnedPod::from_data(data)
                        };

                        stream.update_params(&mut [format.as_pod().unwrap(), buffer_format.as_pod().unwrap()]).unwrap();
                    }
                }
            }
        }).add_buffer({
            let callback_self = self.clone();
            let callback_document = document.clone();
            move |_stream, _, buffer| {
                let mut state = callback_self.0.borrow_mut();
                let PipewireThreadInner { streams, device, .. } = &mut *state;
                let stream = streams.get_mut(&callback_document).unwrap();

                let spa_buffer = unsafe { &mut *(*buffer).buffer };
                let data = unsafe { &mut *(*spa_buffer).datas };

                if data.type_ == sys::SPA_DATA_DmaBuf {
                    let (texture, _datasize, row_stride, _modifier, fd) = callback_self.create_dmabuf_texture(size);
                    let texture_holder = Box::new(texture);

                    data.fd = fd.as_raw_fd() as i64;
                    data.maxsize = size.y as u32 * row_stride as u32;
                    //data.data = null_mut();

                    forget(fd);

                    unsafe {
                        let ptr = Box::into_raw(texture_holder) as *mut _;
                        (*buffer).user_data = ptr;
                    }
                } else { //Assume MemPtr
                    if stream.copy_buffer.is_none() {
                        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("Pipewire SHM Download Buffer"),
                            size: download_row_stride * size.y as u64,
                            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false
                        });
                        
                        stream.copy_buffer = Some(buffer);
                    }
                }
            }
        }).remove_buffer({
            move |_, _, buffer| {
                unsafe {
                    if !(*buffer).user_data.is_null() {

                        let spa_buffer = &mut *(*buffer).buffer;
                        let data = &mut *(*spa_buffer).datas;

                        if data.type_ == sys::SPA_DATA_DmaBuf {
                            let _owned_texture = Box::from_raw((*buffer).user_data as *mut wgpu::Texture);
                            (*buffer).user_data = std::ptr::null_mut();
                        }

                        if data.fd != -1 {
                            let _fd = OwnedFd::from_raw_fd(data.fd as i32);
                        }
                    }
                }
            }
        }).process({
            let callback_self = self.clone();
            let callback_document = document.clone();
            move |stream, _| {
            let mut state = callback_self.0.borrow_mut();
            let PipewireThreadInner { streams, device, queue, .. } = &mut *state;
            let stream_data = streams.get_mut(&callback_document).unwrap();

            let raw_buffer = unsafe {
                stream.dequeue_raw_buffer()
            };

            if !raw_buffer.is_null() {
                //TODO: This assumes n_datas is 1, which is what we wanted.
                let spa_buffer = unsafe { &mut *(*raw_buffer).buffer };
                let data = unsafe { &mut *(*spa_buffer).datas };

                if let Some(last_tex) = stream_data.last_tex.take() {
                    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Pipewire Texture Copy")
                    });

                    if data.type_ == sys::SPA_DATA_DmaBuf {
                        let texture = unsafe { (*raw_buffer).user_data } as *mut wgpu::Texture;
                        let texture = unsafe { &mut *texture };
                        
                        encoder.copy_texture_to_texture(wgpu::TexelCopyTextureInfo {
                            texture: &last_tex,
                            mip_level: 0,
                            aspect: wgpu::TextureAspect::All,
                            origin: wgpu::Origin3d::ZERO
                        }, wgpu::TexelCopyTextureInfo {
                            texture,
                            mip_level: 0,
                            aspect: wgpu::TextureAspect::All,
                            origin: wgpu::Origin3d::ZERO
                        }, wgpu::Extent3d {
                            width: last_tex.width(),
                            height: last_tex.height(),
                            depth_or_array_layers: 1
                        });
                    } else {
                        encoder.copy_texture_to_buffer(wgpu::TexelCopyTextureInfo {
                            texture: &last_tex,
                            mip_level: 0,
                            aspect: wgpu::TextureAspect::All,
                            origin: wgpu::Origin3d::ZERO
                        }, wgpu::TexelCopyBufferInfo {
                            buffer: stream_data.copy_buffer.as_ref().unwrap(),
                            layout: wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(download_row_stride as u32),
                                rows_per_image: None
                            }
                        }, wgpu::Extent3d {
                            width: last_tex.width(),
                            height: last_tex.height(),
                            depth_or_array_layers: 1
                        });
                    }

                    let index = queue.submit(std::iter::once(encoder.finish()));
                    device.poll(wgpu::PollType::Wait { submission_index: Some(index), timeout: None }).unwrap();

                    if data.type_ == sys::SPA_DATA_DmaBuf {
                        unsafe {
                            let data = &mut *data;
                            data.type_ = sys::SPA_DATA_DmaBuf;
                            data.flags = sys::SPA_DATA_FLAG_READWRITE;

                            let chunk = &mut (*data.chunk);
                            chunk.flags = 0;
                            chunk.size = datasize as u32;
                            chunk.offset = 0;
                            chunk.stride = row_stride as i32;
                        }
                    } else {
                        let buffer = stream_data.copy_buffer.as_ref().unwrap();

                        buffer.map_async(wgpu::MapMode::Read, .., |_| {});
                        device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None }).unwrap();
                        
                        let view = buffer.get_mapped_range(..);
                        let cpu_stride = size.x as usize * 4;

                        unsafe {
                            let memptr_data = slice::from_raw_parts_mut(data.data as *mut u8, datasize as usize);

                            // AAGH WE HAVE TO DO INDIVIDUAL ROW COPIES
                            for row_index in 0..size.y as usize {
                                let cpu_base = row_index * cpu_stride;
                                let gpu_base = row_index * download_row_stride as usize;
                                let src = &view[gpu_base..gpu_base + cpu_stride];
                                memptr_data[cpu_base..cpu_base + cpu_stride].copy_from_slice(src);
                            }
                            
                            let data = &mut *data;
                            data.type_ = sys::SPA_DATA_MemPtr;
                            data.flags = sys::SPA_DATA_FLAG_READWRITE;

                            let chunk = &mut (*data.chunk);
                            chunk.flags = 0;
                            chunk.size = datasize as u32;
                            chunk.offset = 0;
                            chunk.stride = cpu_stride as i32;
                        }

                        drop(view);

                        buffer.unmap();
                    }
                }

                unsafe {
                    stream.queue_raw_buffer(raw_buffer);
                }
            }}
        }).state_changed(|_, _, old, new| {
            //TODO: Surface this to the GUI
            eprintln!("{:?} to {:?}", old, new);
        }).register().unwrap();

        stream
            .connect(
                Direction::Output,
                None,
                StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
                &mut [
                    format_with_modifier.as_pod().unwrap(),
                    format_no_modifier.as_pod().unwrap(),
                ],
            )
            .unwrap();
        stream.set_active(true).unwrap();

        state.streams.insert(
            document,
            PipewireStream {
                _stream: stream,
                _listener: listener,
                last_tex: None,
                copy_buffer: None,
            },
        );
    }

    fn update_stream_image(&self, document: Document, texture: wgpu::Texture) {
        let mut state = self.0.borrow_mut();
        let stream = state.streams.get_mut(&document);

        if let Some(stream) = stream {
            stream.last_tex = Some(texture);
        }
    }
}

pub struct PipewirePlugin {
    msg_send: Sender<PipewireMessage>,
    _resp_recv: Receiver<PipewireResponse>,
}

impl SinkPlugin for PipewirePlugin {
    fn publish_stream(
        &mut self,
        document: Document,
        name: String,
        size: glam::Vec2,
        framerate: (u32, u32),
    ) {
        self.msg_send
            .send(PipewireMessage::PublishStream {
                document,
                name,
                size,
                framerate,
            })
            .unwrap_or_else(|_| panic!("Poisoned"));
    }

    fn update_stream_image(&mut self, document: Document, texture: wgpu::Texture) {
        self.msg_send
            .send(PipewireMessage::UpdateStreamImage { document, texture })
            .unwrap_or_else(|_| panic!("Poisoned"));
    }
}

impl PipewirePlugin {
    /// Spawn a Pipewire thread.
    pub fn new(
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Box<dyn SinkPlugin> {
        let (msg_send, msg_recv) = channel();
        let (resp_send, resp_recv) = channel();

        spawn(move || {
            PipewireThread::run(msg_recv, resp_send, adapter, device, queue);
        });

        Box::new(Self {
            msg_send,
            _resp_recv: resp_recv,
        }) as Box<dyn SinkPlugin>
    }
}

impl Drop for PipewirePlugin {
    fn drop(&mut self) {
        self.msg_send
            .send(PipewireMessage::Shutdown)
            .unwrap_or_else(|_| panic!("poisoned"));
    }
}
