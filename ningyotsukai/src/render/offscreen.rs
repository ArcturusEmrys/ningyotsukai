use std::collections::HashMap;
use std::sync::Arc;

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
    resources: Arc<WgpuResources>,

    device: wgpu::Device,

    queue: wgpu::Queue,

    /// All renderers for the puppets on this document's stage.
    artboard_puppet_renderers: HashMap<Index, WgpuRenderer<'static>>,

    /// The stage texture to render to.
    artboard_texture: Option<(wgpu::Texture, wgpu::TextureView)>,

    /// All renderers for rendering the puppets in the user's viewport.
    viewport_puppet_renderers: HashMap<Index, WgpuRenderer<'static>>,

    /// The last known viewport parameters.
    ///
    /// The texture contained in this variable is good for copying ONLY.
    viewport_parameters: Option<(wgpu::Texture, f32, f32, f32)>,

    /// The current viewport buffer texture.
    viewport_buffer: Option<wgpu::Texture>,
}

impl OffscreenRender {
    pub fn new(
        document: Document,
        resources: Arc<WgpuResources>,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Self {
        OffscreenRender {
            document: document.downgrade(),
            resources,
            device,
            queue,
            artboard_puppet_renderers: HashMap::new(),
            artboard_texture: None,
            viewport_puppet_renderers: HashMap::new(),
            viewport_parameters: None,
            viewport_buffer: None,
        }
    }

    /// Remove renderers for puppets that are no longer in the document.
    pub fn collect_garbage(&mut self) {
        if let Some(document) = self.document.upgrade() {
            document.collect_garbage(&mut self.artboard_puppet_renderers);
            document.collect_garbage(&mut self.viewport_puppet_renderers);
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
        &self.artboard_texture.as_ref().unwrap().0
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.artboard_texture.as_ref().unwrap().1
    }

    /// Allocate a new texture to render on.
    ///
    /// This should be called if you plan to use the texture in this for other
    /// non-rendering purposes.
    pub fn alloc_texture(&mut self) {
        if let Some(document) = self.document.upgrade() {
            let required_size = document.stage().size();
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
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
            self.artboard_texture = Some((texture, view));
        }
    }

    pub fn render(&mut self) -> Option<wgpu::SubmissionIndex> {
        let mut submission = None;

        if let Some(document) = self.document.upgrade() {
            let required_size = document.stage().size();
            let current_size = self
                .artboard_texture
                .as_ref()
                .map(|(t, _)| (t.width(), t.height()));

            if current_size != Some((required_size.x as u32, required_size.y as u32)) {
                self.alloc_texture();
            }

            //TODO: This probably should also clear the viewport.
            //TODO: Are we sure we want to use a render pass here?
            let mut encoder = self
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

            submission = Some(self.queue.submit(std::iter::once(encoder.finish())));

            let texture = self.texture().clone();

            for (index, puppet) in document.stage().iter() {
                let renderer_exists = self.artboard_puppet_renderers.contains_key(&index);
                if !renderer_exists {
                    self.artboard_puppet_renderers.insert(
                        index,
                        WgpuRenderer::new_headless_with_resources(
                            self.resources.clone(),
                            &puppet.model(),
                        )
                        .unwrap(),
                    );
                }

                let renderer = self.artboard_puppet_renderers.get_mut(&index).unwrap();
                renderer.set_render_target(texture.clone()).unwrap();

                renderer.camera.position.x =
                    puppet.position().x / puppet.scale() - (required_size.x / 2.0 / puppet.scale());
                renderer.camera.position.y =
                    puppet.position().y / puppet.scale() - (required_size.y / 2.0 / puppet.scale());
                renderer.camera.scale.x = puppet.scale();
                renderer.camera.scale.y = puppet.scale();

                renderer.draw(&puppet.model().puppet).unwrap();

                if let Some(index) = renderer.last_submission_index() {
                    submission = Some(index);
                }
            }
        }

        submission
    }

    pub fn render_viewport(&mut self) -> Option<wgpu::SubmissionIndex> {
        let mut submission = None;
        if let Some(buffer) = &self.viewport_buffer {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Screen clear"),
                });

            encoder.clear_texture(
                buffer,
                &wgpu::ImageSubresourceRange {
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    mip_level_count: None,
                    base_array_layer: 0,
                    array_layer_count: None,
                },
            );

            submission = Some(self.queue.submit(std::iter::once(encoder.finish())));
        }

        if let Some(document) = self.document.upgrade() {
            for (index, puppet) in document.stage().iter() {
                let viewport_renderer_exists = self.viewport_puppet_renderers.contains_key(&index);
                if !viewport_renderer_exists {
                    self.viewport_puppet_renderers.insert(
                        index,
                        // TODO: This should share resources with any artboard
                        // renderer that might exist for the puppet
                        WgpuRenderer::new_headless_with_resources(
                            self.resources.clone(),
                            &puppet.model(),
                        )
                        .unwrap(),
                    );
                }

                self.apply_viewport_to_renderer(index);

                let renderer = self.viewport_puppet_renderers.get_mut(&index).unwrap();
                renderer.draw(&puppet.model().puppet).unwrap();
                if let Some(index) = renderer.last_submission_index() {
                    submission = Some(index);
                }
            }
        }

        submission
    }

    pub fn viewport_copy(&self) -> wgpu::SubmissionIndex {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Output copy"),
            });

        if let Some(buffer) = &self.viewport_buffer {
            if let Some((texture, _, _, _)) = &self.viewport_parameters {
                encoder.copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: buffer,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyTextureInfo {
                        texture: texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d {
                        width: texture.width(),
                        height: texture.height(),
                        depth_or_array_layers: 1,
                    },
                );
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()))
    }

    pub fn update(&mut self, dt: f32) {
        if let Some(mut document) = self.document.upgrade() {
            document.stage_mut().update(dt);
        }
    }

    pub fn apply_viewport_to_renderer(&mut self, index: Index) {
        let renderer = self.viewport_puppet_renderers.get_mut(&index).unwrap();
        if let Some((texture, center_x, center_y, zoom)) = &self.viewport_parameters {
            if let Some(buffer) = &self.viewport_buffer {
                renderer.set_render_target(buffer.clone()).unwrap();
            }

            if let Some(document) = self.document.upgrade() {
                if let Some(puppet) = document.stage().puppet(index) {
                    let mut x = 0.0;
                    let mut y = 0.0;

                    //Cancel out the center coordinate offset Inox uses
                    x -= texture.width() as f32 / 2.0 / puppet.scale();
                    y -= texture.height() as f32 / 2.0 / puppet.scale();

                    x += puppet.position().x / puppet.scale();
                    y += puppet.position().y / puppet.scale();

                    renderer.camera.position.x = x;
                    renderer.camera.position.y = y;
                    renderer.camera.scale.x = puppet.scale();
                    renderer.camera.scale.y = puppet.scale();

                    let camera = renderer.viewport_camera_mut(0).unwrap();

                    camera.position.x = *center_x;
                    camera.position.y = *center_y;
                    camera.scale.x = *zoom;
                    camera.scale.y = *zoom;
                }
            }
        }
    }

    pub fn viewport_change(
        &mut self,
        texture: wgpu::Texture,
        center_x: f32,
        center_y: f32,
        zoom: f32,
    ) {
        self.viewport_parameters = Some((texture.clone(), center_x, center_y, zoom));

        let viewport_buffer_needs_alloc = if let Some(viewport_buffer) = &self.viewport_buffer {
            viewport_buffer.width() != texture.width()
                || viewport_buffer.height() != texture.height()
        } else {
            true
        };

        if viewport_buffer_needs_alloc {
            self.viewport_buffer = Some(self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Viewport Texture Buffer"),
                dimension: wgpu::TextureDimension::D2,
                size: wgpu::Extent3d {
                    width: texture.width(),
                    height: texture.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                format: texture.format(),
                usage: WgpuRenderer::required_render_target_uses()
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            }));
        }

        let indexes: Vec<_> = self.viewport_puppet_renderers.keys().map(|i| *i).collect();
        for index in indexes {
            self.apply_viewport_to_renderer(index);
        }
    }
}
