use crate::document::Document;

pub trait SinkPlugin {
    fn publish_stream(
        &mut self,
        document: Document,
        name: String,
        size: glam::Vec2,
        framerate: (u32, u32),
    );

    fn update_stream_image(&mut self, document: Document, texture: wgpu::Texture);
}
