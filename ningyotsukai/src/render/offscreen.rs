use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use generational_arena::Index;
use ningyo_render_wgpu::{WgpuRenderer, WgpuResources};

use inox2d::render::InoxRendererExt;

use crate::document::{Document, WeakDocument};

pub struct OffscreenRender {
    /// The document we want to render.
    ///
    /// This is a weak reference to allow us to know if we should discard
    /// associated resources when the document is closed.
    document: WeakDocument,

    /// All loaded WGPU resources.
    resources: Arc<Mutex<WgpuResources>>,

    /// All renderers for the puppets on this document's stage.
    puppet_renderers: HashMap<Index, WgpuRenderer<'static>>,

    /// The texture to render to.
    texture: Option<(wgpu::Texture, wgpu::TextureView)>,
}

impl OffscreenRender {
    pub fn new(document: Document, resources: Arc<Mutex<WgpuResources>>) -> Self {
        OffscreenRender {
            document: document.downgrade(),
            resources,
            puppet_renderers: HashMap::new(),
            texture: None,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.document.upgrade().is_some()
    }

    pub fn is_for_document(&self, other_document: &Document) -> bool {
        if let Some(my_doc) = self.document.upgrade() {
            &my_doc == other_document
        } else {
            false
        }
    }

    pub fn document(&self) -> WeakDocument {
        self.document.clone()
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture.as_ref().unwrap().0
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.texture.as_ref().unwrap().1
    }

    /// Allocate a new texture to render on.
    ///
    /// This should be called if you plan to use the texture in this for other
    /// non-rendering purposes.
    pub fn alloc_texture(&mut self) {
        if let Some(document) = self.document.upgrade() {
            let required_size = document.stage().size();
            let resources = self.resources.lock().unwrap();
            let texture = resources.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Offscreen Texture Buffer"),
                dimension: wgpu::TextureDimension::D2,
                size: wgpu::Extent3d {
                    width: required_size.x as u32,
                    height: required_size.y as u32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: WgpuRenderer::required_render_target_uses()
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });

            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("Offscreen Texture Buffer View"),
                format: Some(wgpu::TextureFormat::Rgba8Unorm),
                dimension: Some(wgpu::TextureViewDimension::D2),
                usage: Some(texture.usage()),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            });
            self.texture = Some((texture, view));
        }
    }

    pub fn render(&mut self) {
        //TODO: Stage render target management
        if let Some(document) = self.document.upgrade() {
            let required_size = document.stage().size();
            let current_size = self.texture.as_ref().map(|(t, _)| (t.width(), t.height()));

            if current_size != Some((required_size.x as u32, required_size.y as u32)) {
                self.alloc_texture();
            }

            let resources = self.resources.lock().unwrap();

            let mut encoder =
                resources
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Offscreen Render internal buffer clear"),
                    });

            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Offscreen Render Buffer clear"),
                depth_stencil_attachment: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.view(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });

            resources.queue.submit(std::iter::once(encoder.finish()));

            drop(resources);

            let texture = self.texture().clone();

            for (index, puppet) in document.stage().iter() {
                let renderer_exists = self.puppet_renderers.contains_key(&index);
                if !renderer_exists {
                    self.puppet_renderers.insert(
                        index,
                        WgpuRenderer::new_headless_with_resources(
                            self.resources.clone(),
                            &puppet.model(),
                        )
                        .unwrap(),
                    );
                }

                let renderer = self.puppet_renderers.get_mut(&index).unwrap();
                renderer.set_render_target(texture.clone()).unwrap();

                renderer.camera.position.x =
                    puppet.position().x / puppet.scale() - (required_size.x / 2.0 / puppet.scale());
                renderer.camera.position.y =
                    puppet.position().y / puppet.scale() - (required_size.y / 2.0 / puppet.scale());
                renderer.camera.scale.x = puppet.scale();
                renderer.camera.scale.y = puppet.scale();

                renderer.draw(&puppet.model().puppet).unwrap();
            }
        }
    }
}
