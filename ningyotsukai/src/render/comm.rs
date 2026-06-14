use std::sync::Arc;

use crate::document::Document;
use ningyo_render_wgpu::WgpuResources;
use ningyo_texshare::ExtendedDevice;

#[derive(Debug)]
pub enum RenderMessage {
    UseResources(
        wgpu::Adapter,
        wgpu::Instance,
        Arc<WgpuResources>,
        ExtendedDevice,
        wgpu::Queue,
    ),
    RegisterDocument(Document),
    UnregisterDocument(Document),
    RenderViewport {
        document: Document,
        texture: wgpu::Texture,
        center_x: f32,
        center_y: f32,
        scale: f32,
    },
    Shutdown,
}

#[derive(Debug)]
pub enum RenderResponse {
    DidFrameUpdate,
    RenderComplete(Document, Option<wgpu::SubmissionIndex>),
}
