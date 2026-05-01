use std::sync::{Arc, Mutex};

use crate::document::Document;
use ningyo_render_wgpu::WgpuResources;
use ningyo_texshare::ExtendedDevice;

pub enum RenderMessage<C> {
    UseResources(C, wgpu::Adapter, Arc<Mutex<WgpuResources>>, ExtendedDevice),
    RegisterDocument(C, Document),
    DidFrameUpdate(C),
    UnregisterDocument(C, Document),
    Shutdown,
}

pub enum RenderResponse<C> {
    Ack(C),
}
