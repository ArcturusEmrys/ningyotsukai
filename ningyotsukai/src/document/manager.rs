use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::spawn;

use ningyo_render_wgpu::WgpuResources;

use crate::document::Document;
use crate::render::{RenderMessage, RenderResponse, render_start};

#[derive(Clone)]
pub struct DocumentManager(Rc<RefCell<DocumentManagerInner>>);
struct DocumentManagerInner {
    send: Sender<RenderMessage<()>>,

    recv: Receiver<RenderResponse<()>>,
}

impl DocumentManager {
    pub fn new() -> Self {
        let (send, recv) = render_start();

        DocumentManager(Rc::new(RefCell::new(DocumentManagerInner { send, recv })))
    }

    pub fn register_document(&self, document: Document) {
        self.0
            .borrow()
            .send
            .send(RenderMessage::RegisterDocument((), document))
            .unwrap();
    }

    pub fn unregister_document(&self, document: Document) {
        self.0
            .borrow()
            .send
            .send(RenderMessage::UnregisterDocument((), document))
            .unwrap();
    }

    pub fn use_resources(&self, resources: Arc<Mutex<WgpuResources>>) {
        self.0
            .borrow()
            .send
            .send(RenderMessage::UseResources((), resources))
            .unwrap();
    }
}
