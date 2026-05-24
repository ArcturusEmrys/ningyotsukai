use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread::spawn;
use std::time::Instant;

use ningyo_render_wgpu::WgpuResources;
use ningyo_texshare::ExtendedDevice;

use crate::document::{Document, WeakDocument};
use crate::render::SinkPlugin;
use crate::render::comm::{RenderMessage, RenderResponse};
use crate::render::offscreen::OffscreenRender;

struct RenderThread {
    wgpu_resources: Option<Arc<WgpuResources>>,
    wgpu_adapter: Option<wgpu::Adapter>,
    extended_device: Option<ExtendedDevice>,
    wgpu_queue: Option<wgpu::Queue>,

    last_time: Instant,

    renderers: Vec<OffscreenRender>,
    unregistered_documents: Vec<WeakDocument>,

    plugins: Vec<Box<dyn SinkPlugin>>,

    /// The last recorded ViewportChanged message.
    ///
    /// We can't immediately process these as we may have multiples of them in
    /// flight.
    last_viewport_unlock: Option<(Document, wgpu::Texture, f32, f32, f32)>,

    #[cfg(feature = "timing")]
    start_time: std::time::Instant,

    #[cfg(feature = "timing")]
    last_segment_time: std::time::Instant,

    /// Renderdoc API
    #[cfg(feature = "renderdoc")]
    doc: Option<renderdoc::RenderDoc<renderdoc::V100>>,
}

impl RenderThread {
    fn new() -> Self {
        let wgpu_resources = None;
        let wgpu_adapter = None;

        let renderers = vec![];
        let unregistered_documents = vec![];

        let plugins = vec![];

        let last_time = Instant::now();

        RenderThread {
            wgpu_resources,
            wgpu_adapter,
            extended_device: None,
            wgpu_queue: None,
            last_time,
            renderers,
            unregistered_documents,
            plugins,
            last_viewport_unlock: None,

            #[cfg(feature = "renderdoc")]
            doc: None,

            #[cfg(feature = "timing")]
            start_time: last_time.clone(),

            #[cfg(feature = "timing")]
            last_segment_time: last_time.clone(),
        }
    }

    fn register_document(&mut self, document: Document) {
        let size = document.stage().size();

        self.renderers.push(OffscreenRender::new(
            document.clone(),
            self.wgpu_resources.clone().unwrap(),
            self.extended_device.as_ref().unwrap().device().clone(),
            self.wgpu_queue.clone().unwrap(),
        ));

        for plugin in &mut self.plugins {
            plugin.publish_stream(
                document.clone(),
                "Ningyotsukai Document".to_string(),
                size,
                (60, 1),
            );
        }
    }

    fn viewport_change(
        &mut self,
        document: Document,
        texture: wgpu::Texture,
        center_x: f32,
        center_y: f32,
        scale: f32,
    ) {
        for renderer in &mut self.renderers {
            if renderer.is_for_document(&document) {
                renderer.viewport_change(texture.clone(), center_x, center_y, scale);
            }
        }
    }

    #[cfg(feature = "timing")]
    fn lap(&mut self, segment_name: &str) {
        let cur_segment_time = std::time::Instant::now();
        let last_segment_dur = cur_segment_time - self.last_segment_time;

        self.last_segment_time = cur_segment_time;

        eprintln!(
            "  {}: {}ms",
            segment_name,
            last_segment_dur.as_micros() as f64 / 1000.0
        );
    }

    fn start_frame(&mut self) {
        #[cfg(feature = "timing")]
        {
            self.start_time = std::time::Instant::now();
            eprintln!("BEGIN FRAME",);
            let time_since_last = self.start_time - self.last_time;
            eprintln!(
                "  Time since last update: {}ms",
                time_since_last.as_micros() as f64 / 1000.0
            );
        }

        #[cfg(feature = "renderdoc")]
        {
            use std::ptr::null;
            if self.doc.is_none() {
                self.doc = renderdoc::RenderDoc::new().ok();
            }

            //TODO: Can I get native window handles out of GTK?
            if self.doc.is_some() {
                if let Some(device) = self.extended_device.as_ref() {
                    #[allow(unused)]
                    let device = device.device();
                    #[cfg(target_os = "windows")]
                    {
                        use windows::core::Interface;

                        if let Some(dx12_device) = unsafe { device.as_hal::<wgpu_hal::dx12::Api>() }
                        {
                            let dx12_context = dx12_device.raw_device();
                            self.doc
                                .as_mut()
                                .unwrap()
                                .start_frame_capture(dx12_context.as_raw(), null());
                        }
                    }

                    #[cfg(not(target_os = "windows"))]
                    self.doc
                        .as_mut()
                        .unwrap()
                        .start_frame_capture(null(), null());
                }
            }
        }

        #[cfg(all(feature = "renderdoc", feature = "timing"))]
        self.lap("Renderdoc setup");

        let mut garbage = vec![];
        for (index, renderer) in self.renderers.iter_mut().enumerate().rev() {
            if !renderer.is_valid() {
                garbage.push(index);
            } else {
                renderer.collect_garbage();
            }
        }

        for index in garbage {
            self.renderers.remove(index);
        }

        #[cfg(feature = "timing")]
        self.lap("Dead renderer cleanup");
    }

    fn end_frame(&mut self) {
        #[cfg(feature = "tracy")]
        {
            if self.wgpu_resources.is_some() {
                self.wgpu_resources.as_ref().unwrap().end_frame().unwrap();
            }

            #[cfg(all(feature = "timing"))]
            self.lap("Tracy teardown");
        }

        #[cfg(feature = "renderdoc")]
        {
            use std::ptr::null;
            if self.doc.is_some() {
                if let Some(device) = self.extended_device.as_ref() {
                    #[allow(unused)]
                    let device = device.device();

                    #[cfg(target_os = "windows")]
                    {
                        use windows::core::Interface;

                        if let Some(dx12_device) = unsafe { device.as_hal::<wgpu_hal::dx12::Api>() }
                        {
                            let dx12_context = dx12_device.raw_device();
                            self.doc
                                .as_mut()
                                .unwrap()
                                .end_frame_capture(dx12_context.as_raw(), null());
                        } else {
                            unreachable!();
                        }
                    }

                    #[cfg(not(target_os = "windows"))]
                    self.doc.as_mut().unwrap().end_frame_capture(null(), null());
                }
            }
        }

        #[cfg(feature = "timing")]
        {
            #[cfg(feature = "renderdoc")]
            self.lap("Renderdoc teardown");

            let end_time = std::time::Instant::now();
            let time_elapsed = end_time - self.start_time;

            eprintln!(
                "Thread only: {}ms / {} FPS",
                time_elapsed.as_micros() as f64 / 1000.0,
                1_000_000.0 / time_elapsed.as_micros() as f64
            );

            let time_elapsed = end_time - self.last_time;

            eprintln!(
                "Total time: {}ms / {} FPS",
                time_elapsed.as_micros() as f64 / 1000.0,
                1_000_000.0 / time_elapsed.as_micros() as f64
            );
        }
    }

    /// Main loop for off-canvas rendering.
    fn main<C>(&mut self, recv: Receiver<RenderMessage<C>>, send: Sender<RenderResponse<C>>) {
        loop {
            match recv.try_recv() {
                Ok(RenderMessage::UseResources(c, adapter, resources, extended_device, queue)) => {
                    self.wgpu_resources = Some(resources);
                    self.wgpu_adapter = Some(adapter);
                    self.extended_device = Some(extended_device);
                    self.wgpu_queue = Some(queue);

                    #[cfg(feature = "pipewire")]
                    {
                        let adapter = self.wgpu_adapter.clone().unwrap();
                        let device = self.extended_device.clone().unwrap();
                        let queue = self.wgpu_queue.clone().unwrap();

                        self.plugins
                            .push(crate::render::pipewire::PipewirePlugin::new(
                                adapter, device, queue,
                            ));
                    }

                    #[cfg(feature = "spout")]
                    {
                        self.plugins.push(crate::render::spout::SpoutPlugin::new());
                    }

                    let mut doclist = vec![];

                    for document in self.unregistered_documents.drain(..) {
                        if let Some(document) = document.upgrade() {
                            doclist.push(document);
                        }
                    }

                    for document in doclist {
                        self.register_document(document);
                    }

                    send.send(RenderResponse::Ack(c)).unwrap();
                }
                Ok(RenderMessage::RegisterDocument(c, document)) => {
                    if self.wgpu_adapter.is_none() || self.wgpu_resources.is_none() {
                        self.unregistered_documents.push(document.downgrade());
                    } else {
                        self.register_document(document);
                    }

                    send.send(RenderResponse::Ack(c)).unwrap();
                }
                Ok(RenderMessage::RenderViewport {
                    cookie,
                    document,
                    texture,
                    center_x,
                    center_y,
                    scale,
                }) => {
                    self.last_viewport_unlock =
                        Some((document, texture, center_x, center_y, scale));
                    send.send(RenderResponse::Ack(cookie)).unwrap();
                }
                Err(TryRecvError::Empty) => {
                    // The channel is empty. Run an update.
                    let cur_time = Instant::now();
                    let del_time = cur_time - self.last_time;
                    let dt = del_time.as_micros() as f32 / 1_000_000.0;

                    self.start_frame();

                    for renderer in self.renderers.iter_mut() {
                        renderer.update(dt);
                    }

                    #[cfg(feature = "timing")]
                    self.lap("Update");

                    for renderer in self.renderers.iter_mut() {
                        //TODO: Force an allocation every frame so that plugins
                        //don't ever see intermediate results.
                        //Ideally, this should be a ring buffer.
                        renderer.alloc_texture();
                        renderer.render();
                    }

                    #[cfg(feature = "timing")]
                    self.lap("Artboard render");

                    if let (Some(device), Some(queue)) =
                        (self.extended_device.as_ref(), self.wgpu_queue.as_ref())
                    {
                        device
                            .device()
                            .poll(wgpu::PollType::Wait {
                                submission_index: None,
                                timeout: None,
                            })
                            .unwrap();

                        for plugin in &mut self.plugins {
                            for renderer in &mut self.renderers {
                                plugin.update_stream_image(
                                    renderer.document().upgrade().unwrap(),
                                    self.wgpu_adapter.as_ref().unwrap(),
                                    self.extended_device.as_ref().unwrap(),
                                    queue,
                                    renderer.texture().clone(),
                                );
                            }
                        }
                    }

                    #[cfg(feature = "timing")]
                    self.lap("Plugin update");

                    if let Some((document, texture, center_x, center_y, scale)) =
                        self.last_viewport_unlock.take()
                    {
                        self.viewport_change(document, texture, center_x, center_y, scale);

                        for renderer in self.renderers.iter_mut() {
                            let index = renderer.render_viewport();
                            send.send(RenderResponse::RenderComplete(
                                renderer.document().upgrade().unwrap(),
                                index,
                            ))
                            .unwrap();
                        }

                        #[cfg(feature = "timing")]
                        self.lap("Viewport render");
                    }

                    self.end_frame();

                    self.last_time = self.start_time;

                    send.send(RenderResponse::DidFrameUpdate).unwrap();
                }
                Ok(RenderMessage::UnregisterDocument(c, document)) => {
                    let index = self
                        .renderers
                        .iter()
                        .enumerate()
                        .find(|(_, d)| d.is_for_document(&document));

                    if let Some((index, _)) = index {
                        self.renderers.remove(index);
                    }

                    let unreg_index = self
                        .unregistered_documents
                        .iter()
                        .enumerate()
                        .find(|(_, d)| d.ptr_eq(&document.downgrade()));

                    if let Some((index, _)) = unreg_index {
                        self.unregistered_documents.remove(index);
                    }

                    send.send(RenderResponse::Ack(c)).unwrap();
                }
                Ok(RenderMessage::Shutdown) | Err(_) => return,
            }
        }
    }
}

/// Spawn the render thread.
///
/// Returns communication channels for sending requests and receiving
/// responses.
pub fn render_start<C: Send + 'static>() -> (Sender<RenderMessage<C>>, Receiver<RenderResponse<C>>)
{
    let (send_message, recv_message) = channel();
    let (send_response, recv_response) = channel();

    spawn(|| {
        RenderThread::new().main::<C>(recv_message, send_response);
    });

    (send_message, recv_response)
}
