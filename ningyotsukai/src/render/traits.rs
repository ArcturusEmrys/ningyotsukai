use crate::document::Document;
use ningyo_texshare::ExtendedDevice;

pub trait SinkPlugin {
    fn publish_stream(
        &mut self,
        document: Document,
        name: String,
        size: glam::Vec2,
        framerate: (u32, u32),
    );

    fn update_stream_image(
        &mut self,
        document: Document,
        adapter: &wgpu::Adapter,
        device: &ExtendedDevice,
        queue: &wgpu::Queue,
        texture: wgpu::Texture,
    );
}
