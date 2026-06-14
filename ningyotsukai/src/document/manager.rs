use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use ningyo_render_wgpu::WgpuResources;
use ningyo_texshare::ExtendedDevice;

use crate::document::{Document, WeakDocument};
use crate::render::{RenderMessage, RenderResponse, render_start};

type UpdateCallback = Box<dyn Fn()>;
type RenderCallback = Box<dyn Fn(Document, Option<wgpu::SubmissionIndex>)>;

#[derive(Clone)]
pub struct DocumentManager(Rc<RefCell<DocumentManagerInner>>);
struct DocumentManagerInner {
    documents: Vec<WeakDocument>,

    /// Callbacks fired whenever the offcanvas thread has completed an update.
    callbacks: Vec<UpdateCallback>,

    /// Callbacks fired whenever the offcanvas thread has completed rendering.
    render_callbacks: Vec<RenderCallback>,

    send: Sender<RenderMessage>,

    recv: Receiver<RenderResponse>,
}

impl DocumentManager {
    pub fn new() -> Self {
        let (send, recv) = render_start();

        let me = DocumentManager(Rc::new(RefCell::new(DocumentManagerInner {
            send,
            recv,
            documents: vec![],
            callbacks: vec![],
            render_callbacks: vec![],
        })));

        glib::idle_add_local({
            let idle_me = Rc::downgrade(&me.0);
            move || {
                if let Some(me) = idle_me.upgrade() {
                    DocumentManager(me).tick();
                    glib::ControlFlow::Continue
                } else {
                    glib::ControlFlow::Break
                }
            }
        });

        me
    }

    pub fn register_document(&mut self, document: Document) {
        let state = &mut *self.0.borrow_mut();

        state.documents.push(document.downgrade());

        state
            .send
            .send(RenderMessage::RegisterDocument(document))
            .unwrap();
    }

    pub fn unregister_document(&mut self, document: Document) {
        let state = &mut *self.0.borrow_mut();

        if let Some((index, _)) = state
            .documents
            .iter()
            .enumerate()
            .find(|(_, wd)| wd.ptr_eq(&document.downgrade()))
        {
            state.documents.remove(index);
        }

        self.0
            .borrow()
            .send
            .send(RenderMessage::UnregisterDocument(document))
            .unwrap();
    }

    pub fn use_resources(
        &self,
        adapter: wgpu::Adapter,
        instance: wgpu::Instance,
        resources: Arc<WgpuResources>,
        extended_device: ExtendedDevice,
        queue: wgpu::Queue,
    ) {
        self.0
            .borrow()
            .send
            .send(RenderMessage::UseResources(
                adapter,
                instance,
                resources,
                extended_device,
                queue,
            ))
            .unwrap();
    }

    pub fn render_viewport(
        &self,
        document: Document,
        texture: wgpu::Texture,
        center_x: f32,
        center_y: f32,
        scale: f32,
    ) {
        self.0
            .borrow()
            .send
            .send(RenderMessage::RenderViewport {
                document,
                texture,
                center_x,
                center_y,
                scale,
            })
            .unwrap();
    }

    pub fn add_update_callback<F>(&mut self, callback: F)
    where
        F: Fn() + 'static,
    {
        self.0.borrow_mut().callbacks.push(Box::new(callback));
    }

    pub fn add_render_callback<F>(&mut self, callback: F)
    where
        F: Fn(Document, Option<wgpu::SubmissionIndex>) + 'static,
    {
        self.0
            .borrow_mut()
            .render_callbacks
            .push(Box::new(callback));
    }

    pub fn tick(&mut self) {
        let state = &mut *self.0.borrow_mut();
        let mut garbage = vec![];

        for (index, document) in state.documents.iter().enumerate().rev() {
            if document.upgrade().is_none() {
                garbage.push(index);
            }
        }

        for index in garbage {
            state.documents.remove(index);
        }

        while let Ok(e) = state.recv.try_recv() {
            match e {
                RenderResponse::DidFrameUpdate => {
                    for callback in state.callbacks.iter() {
                        callback();
                    }
                }
                RenderResponse::RenderComplete(doc, index) => {
                    for callback in state.render_callbacks.iter() {
                        callback(doc.clone(), index.clone());
                    }
                }
            }
        }
    }

    pub fn shutdown(&mut self) {
        self.0
            .borrow_mut()
            .send
            .send(RenderMessage::Shutdown)
            .unwrap();
    }
}
