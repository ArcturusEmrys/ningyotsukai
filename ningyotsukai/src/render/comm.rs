use std::sync::Arc;

use crate::document::Document;
use ningyo_render_wgpu::WgpuResources;
use ningyo_texshare::ExtendedDevice;

pub enum RenderMessage<C> {
    UseResources(
        C,
        wgpu::Adapter,
        Arc<WgpuResources>,
        ExtendedDevice,
        wgpu::Queue,
    ),
    RegisterDocument(C, Document),
    UnregisterDocument(C, Document),
    RenderViewport {
        cookie: C,
        document: Document,
        texture: wgpu::Texture,
        center_x: f32,
        center_y: f32,
        scale: f32,
    },
    Shutdown,
}

pub enum RenderResponse<C> {
    Ack(C),
    DidFrameUpdate,
    RenderComplete(Document, Option<wgpu::SubmissionIndex>),
}
