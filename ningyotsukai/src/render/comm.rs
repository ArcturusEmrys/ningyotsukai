use crate::document::Document;

pub enum RenderMessage<C> {
    UseWgpuDevice(C, wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue),
    RegisterDocument(C, Document),
    DoFrameUpdate(C),
    UnregisterDocument(C, Document),
    Shutdown,
}

pub enum RenderResponse<C> {
    Ack(C),
}
