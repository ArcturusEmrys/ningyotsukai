use crate::document::Document;
use ningyo_texshare::ExtendedDevice;

pub trait SinkPlugin {
    /// Publish a new stream.
    ///
    /// Streams are identified by their Document. Further sink plugin messages
    /// will use the same Document to identify the same stream.
    fn publish_stream(
        &mut self,
        document: Document,
        name: String,
        size: glam::Vec2,
        framerate: (u32, u32),
    );

    /// Indicate that one or more of the given stream parameters has changed.
    ///
    /// The document given will match a previously published stream.
    fn stream_parameters_changed(
        &mut self,
        document: Document,
        name: String,
        size: glam::Vec2,
        framerate: (u32, u32),
    );

    /// Update an existing stream's image.
    ///
    /// The document given will match a previously published stream with the
    /// same name. The texture, origin, and extent combination refers
    /// to a portion of the texture that we wish to publish to the stream.
    fn update_stream_image(
        &mut self,
        document: Document,
        adapter: &wgpu::Adapter,
        device: &ExtendedDevice,
        queue: &wgpu::Queue,
        texture: wgpu::Texture,
        origin: wgpu::Origin3d,
        extent: wgpu::Extent3d,
    );
}
