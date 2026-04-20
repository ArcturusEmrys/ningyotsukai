use std::sync::{Arc, Mutex};

use crate::document::Document;
use ningyo_render_wgpu::WgpuResources;

pub enum RenderMessage<C> {
    UseResources(C, wgpu::Adapter, Arc<Mutex<WgpuResources>>),
    RegisterDocument(C, Document),
    DidFrameUpdate(C),
    UnregisterDocument(C, Document),
    Shutdown,
}

pub enum RenderResponse<C> {
    Ack(C),
}
