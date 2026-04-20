use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use ningyo_render_wgpu::WgpuResources;

use crate::document::{Document, WeakDocument};
use crate::render::{RenderMessage, RenderResponse, render_start};

#[derive(Clone)]
pub struct DocumentManager(Rc<RefCell<DocumentManagerInner>>);
struct DocumentManagerInner {
    documents: Vec<WeakDocument>,

    send: Sender<RenderMessage<()>>,

    recv: Receiver<RenderResponse<()>>,
}

impl DocumentManager {
    pub fn new() -> Self {
        let (send, recv) = render_start();

        DocumentManager(Rc::new(RefCell::new(DocumentManagerInner {
            send,
            recv,
            documents: vec![],
        })))
    }

    pub fn register_document(&mut self, document: Document) {
        let state = &mut *self.0.borrow_mut();

        state.documents.push(document.downgrade());

        state
            .send
            .send(RenderMessage::RegisterDocument((), document))
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
            .send(RenderMessage::UnregisterDocument((), document))
            .unwrap();
    }

    pub fn use_resources(&self, adapter: wgpu::Adapter, resources: Arc<Mutex<WgpuResources>>) {
        self.0
            .borrow()
            .send
            .send(RenderMessage::UseResources((), adapter, resources))
            .unwrap();
    }

    pub fn update(&mut self, dt: f32) {
        let state = &mut *self.0.borrow_mut();
        let mut garbage = vec![];

        for (index, document) in state.documents.iter().enumerate() {
            if let Some(mut document) = document.upgrade() {
                document.stage_mut().update(dt);
            } else {
                garbage.push(index);
            }
        }

        state.send.send(RenderMessage::DidFrameUpdate(()));
    }
}
