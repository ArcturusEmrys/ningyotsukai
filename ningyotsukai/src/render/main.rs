use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::spawn;

use ningyo_render_wgpu::WgpuResources;
use ningyo_texshare::ExtendedDevice;

use crate::document::{Document, WeakDocument};
use crate::render::SinkPlugin;
use crate::render::comm::{RenderMessage, RenderResponse};
use crate::render::offscreen::OffscreenRender;

struct RenderThread {
    wgpu_resources: Option<Arc<Mutex<WgpuResources>>>,
    wgpu_adapter: Option<wgpu::Adapter>,
    extended_device: Option<ExtendedDevice>,

    renderers: Vec<OffscreenRender>,
    unregistered_documents: Vec<WeakDocument>,

    plugins: Vec<Box<dyn SinkPlugin>>,

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

        RenderThread {
            wgpu_resources,
            wgpu_adapter,
            extended_device: None,
            renderers,
            unregistered_documents,
            plugins,

            #[cfg(feature = "renderdoc")]
            doc: None,
        }
    }

    fn register_document(&mut self, document: Document) {
        let size = document.stage().size();

        self.renderers.push(OffscreenRender::new(
            document.clone(),
            self.wgpu_resources.clone().unwrap(),
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

    /// Main loop for off-canvas rendering.
    fn main<C>(&mut self, recv: Receiver<RenderMessage<C>>, send: Sender<RenderResponse<C>>) {
        loop {
            match recv.recv() {
                Ok(RenderMessage::UseResources(c, adapter, resources, extended_device)) => {
                    self.wgpu_resources = Some(resources);
                    self.wgpu_adapter = Some(adapter);
                    self.extended_device = Some(extended_device);

                    #[cfg(feature = "pipewire")]
                    {
                        let resources = self.wgpu_resources.as_ref().unwrap();
                        let resources = resources.lock().unwrap();
                        let adapter = self.wgpu_adapter.clone().unwrap();

                        self.plugins
                            .push(crate::render::pipewire::PipewirePlugin::new(
                                adapter,
                                resources.device.clone(),
                                resources.queue.clone(),
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
                Ok(RenderMessage::DidFrameUpdate(c)) => {
                    #[cfg(feature = "renderdoc")]
                    {
                        use std::ptr::null;
                        if self.doc.is_none() {
                            self.doc = renderdoc::RenderDoc::new().ok();
                        }

                        //TODO: Can I get native window handles out of GTK?
                        if self.doc.is_some() {
                            self.doc
                                .as_mut()
                                .unwrap()
                                .start_frame_capture(null(), null());
                        }
                    }

                    for renderer in self.renderers.iter_mut() {
                        //TODO: Force an allocation every frame so that plugins
                        //don't ever see intermediate results.
                        //Ideally, this should be a ring buffer.
                        renderer.alloc_texture();
                        renderer.render();
                    }

                    if let Some(resources) = self.wgpu_resources.as_ref() {
                        resources
                            .lock()
                            .unwrap()
                            .device
                            .poll(wgpu::PollType::Wait {
                                submission_index: None,
                                timeout: None,
                            })
                            .unwrap();

                        let queue = self
                            .wgpu_resources
                            .as_ref()
                            .unwrap()
                            .lock()
                            .unwrap()
                            .queue
                            .clone();

                        for plugin in &mut self.plugins {
                            for renderer in &mut self.renderers {
                                plugin.update_stream_image(
                                    renderer.document().upgrade().unwrap(),
                                    self.wgpu_adapter.as_ref().unwrap(),
                                    self.extended_device.as_ref().unwrap(),
                                    &queue,
                                    renderer.texture().clone(),
                                );
                            }
                        }
                    }

                    #[cfg(feature = "renderdoc")]
                    {
                        use std::ptr::null;
                        if self.doc.is_some() {
                            self.doc.as_mut().unwrap().end_frame_capture(null(), null());
                        }
                    }

                    send.send(RenderResponse::Ack(c)).unwrap();
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
