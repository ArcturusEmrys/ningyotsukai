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
                renderer
                    .viewport_change(texture.clone(), center_x, center_y, scale)
                    .unwrap();
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
                "This update: {}ms / {} FPS",
                time_elapsed.as_micros() as f64 / 1000.0,
                1_000_000.0 / time_elapsed.as_micros() as f64
            );

            self.last_time = end_time;
        }
    }

    fn do_update(&mut self, send: &Sender<RenderResponse>) {
        let cur_time = Instant::now();
        let del_time = cur_time - self.last_time;
        let dt = del_time.as_micros() as f32 / 1_000_000.0;

        self.start_frame();

        // First, check if we got any RenderViewport messages.
        // We do this first thing to unlock GTK, since the rest of the update
        // will take longer.
        if let Some((doc, _, _, _, _)) = &self.last_viewport_unlock {
            for renderer in &self.renderers {
                if renderer.is_for_document(doc) {
                    send.send(RenderResponse::RenderComplete(
                        renderer.document().upgrade().unwrap(),
                        renderer.viewport_copy(),
                    ))
                    .unwrap();
                }
            }
        }

        #[cfg(feature = "timing")]
        self.lap("Copy to main thread");

        for renderer in self.renderers.iter_mut() {
            renderer.update(dt);
        }

        #[cfg(feature = "timing")]
        self.lap("Update");

        let last_viewport_unlock = self.last_viewport_unlock.take();

        if let Some((document, texture, center_x, center_y, scale)) = last_viewport_unlock {
            self.viewport_change(document, texture, center_x, center_y, scale);
        }

        #[cfg(feature = "timing")]
        self.lap("Viewport alloc");

        for renderer in self.renderers.iter_mut() {
            if let Err(e) = renderer.render() {
                eprintln!("Renderer error: {}", e);
            }
        }

        #[cfg(feature = "timing")]
        self.lap("Render");

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
                    if let Some((texture, offset, extent)) = renderer.artboard_texture() {
                        plugin.update_stream_image(
                            renderer.document().upgrade().unwrap(),
                            self.wgpu_adapter.as_ref().unwrap(),
                            self.extended_device.as_ref().unwrap(),
                            queue,
                            texture,
                            offset,
                            extent,
                        );
                    }
                }
            }
        }

        #[cfg(feature = "timing")]
        self.lap("Plugin update");

        self.end_frame();

        self.last_time = cur_time;

        send.send(RenderResponse::DidFrameUpdate).unwrap();
    }

    /// Main loop for off-canvas rendering.
    fn main(&mut self, recv: Receiver<RenderMessage>, send: Sender<RenderResponse>) {
        loop {
            match recv.try_recv() {
                Ok(RenderMessage::UseResources(adapter, resources, extended_device, queue)) => {
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
                }
                Ok(RenderMessage::RegisterDocument(document)) => {
                    if self.wgpu_adapter.is_none() || self.wgpu_resources.is_none() {
                        self.unregistered_documents.push(document.downgrade());
                    } else {
                        self.register_document(document);
                    }
                }
                Ok(RenderMessage::RenderViewport {
                    document,
                    texture,
                    center_x,
                    center_y,
                    scale,
                }) => {
                    //TODO: This won't work if we have multiple documents open.
                    self.last_viewport_unlock =
                        Some((document, texture, center_x, center_y, scale));
                }
                Err(TryRecvError::Empty) => {
                    // The channel is empty. We are idle. Run an update.
                    self.do_update(&send);
                }
                Ok(RenderMessage::UnregisterDocument(document)) => {
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
pub fn render_start() -> (Sender<RenderMessage>, Receiver<RenderResponse>) {
    let (send_message, recv_message) = channel();
    let (send_response, recv_response) = channel();

    spawn(|| {
        RenderThread::new().main(recv_message, send_response);
    });

    (send_message, recv_response)
}
