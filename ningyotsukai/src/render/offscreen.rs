use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use generational_arena::Index;
use ningyo_render_wgpu::{RenderTarget, WgpuRenderer, WgpuRendererError, WgpuResources};

use inox2d::render::InoxRendererExt;

use crate::document::{Document, WeakDocument};
use crate::render::SinkPlugin;

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

    /// All renderers for rendering the puppets in the document.
    puppet_renderers: HashMap<Index, WgpuRenderer<'static>>,

    /// The last known viewport parameters.
    ///
    /// The texture contained in this variable is good for copying ONLY.
    viewport_parameters: Option<(wgpu::Texture, f32, f32, f32)>,

    /// The current set of render target(s).
    render_target: Arc<Mutex<RenderTarget<'static>>>,

    /// The last recorded ViewportChanged message.
    ///
    /// We can't immediately process these as we may have multiples of them in
    /// flight. Instead, we queue this up here, and then after all messages are
    /// processed, we do an update where this is copied over to
    /// viewport_parameters.
    last_viewport_unlock: Option<(wgpu::Texture, f32, f32, f32)>,

    /// The last known size of the artboard.
    last_artboard_size: Option<glam::Vec2>,
}

impl OffscreenRender {
    pub fn new(
        document: Document,
        resources: Arc<WgpuResources>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        sink_plugins: &mut [Box<dyn SinkPlugin>],
    ) -> Self {
        let size = document.stage().size();

        for plugin in sink_plugins {
            plugin.publish_stream(
                document.clone(),
                "Ningyotsukai Document".to_string(),
                size,
                (60, 1),
            );
        }

        OffscreenRender {
            document: document.downgrade(),
            resources,
            device,
            queue,
            puppet_renderers: HashMap::new(),
            viewport_parameters: None,
            render_target: Arc::new(Mutex::new(RenderTarget::new_texture_target())),
            last_viewport_unlock: None,
            last_artboard_size: None,
        }
    }

    /// Remove renderers for puppets that are no longer in the document.
    pub fn collect_garbage(&mut self) {
        if let Some(document) = self.document.upgrade() {
            document.collect_garbage(&mut self.puppet_renderers);
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

    pub fn artboard_texture(&self) -> Option<(wgpu::Texture, wgpu::Origin3d, wgpu::Extent3d)> {
        let render_target = self.render_target.lock().unwrap();
        let (origin, extent) = render_target.viewport_origin_and_extent(1)?;

        Some((render_target.color_target().ok()?, origin, extent))
    }

    pub fn render(&mut self) -> Result<wgpu::SubmissionIndex, WgpuRendererError> {
        let mut submission = self
            .render_target
            .lock()
            .unwrap()
            .clear(&self.device, &self.queue)?;

        if let Some(document) = self.document.upgrade() {
            for (index, puppet) in document.stage().iter() {
                let viewport_renderer_exists = self.puppet_renderers.contains_key(&index);
                if !viewport_renderer_exists {
                    self.puppet_renderers.insert(
                        index,
                        // TODO: This should share resources with any artboard
                        // renderer that might exist for the puppet
                        WgpuRenderer::new_headless_with_resources(
                            self.resources.clone(),
                            &puppet.model(),
                            self.render_target.clone(),
                        )
                        .unwrap(),
                    );
                }

                self.apply_viewport_to_renderer(index);

                let renderer = self.puppet_renderers.get_mut(&index).unwrap();
                renderer.draw(&puppet.model().puppet).unwrap();
                if let Some(index) = renderer.last_submission_index() {
                    submission = index;
                }
            }
        }

        Ok(submission)
    }

    pub fn viewport_copy(&self) -> Option<wgpu::SubmissionIndex> {
        let render_target = self.render_target.lock().unwrap();
        if let Some((texture, _, _, _)) = &self.viewport_parameters {
            return Some(
                render_target
                    .copy(&self.device, &self.queue, texture, 0)
                    .unwrap(),
            );
        }

        None
    }

    pub fn update(&mut self, dt: f32) {
        if let Some(mut document) = self.document.upgrade() {
            document.stage_mut().update(dt);
        }
    }

    pub fn apply_viewport_to_renderer(&mut self, index: Index) {
        let renderer = self.puppet_renderers.get_mut(&index).unwrap();
        if let Some(document) = self.document.upgrade() {
            if let Some(puppet) = document.stage().puppet(index) {
                let mut x = 0.0;
                let mut y = 0.0;

                //Cancel out the center coordinate offset Inox uses
                x += puppet.position().x;
                y += puppet.position().y;

                renderer.camera.position.x = x / puppet.scale();
                renderer.camera.position.y = y / puppet.scale();
                renderer.camera.scale.x = puppet.scale();
                renderer.camera.scale.y = puppet.scale();
            }
        }
    }

    pub fn take_last_viewport_message(&mut self) -> Option<(wgpu::Texture, f32, f32, f32)> {
        self.last_viewport_unlock.take()
    }

    pub fn set_last_viewport_message(&mut self, message: (wgpu::Texture, f32, f32, f32)) {
        self.last_viewport_unlock = Some(message);
    }

    pub fn viewport_change(
        &mut self,
        texture: wgpu::Texture,
        center_x: f32,
        center_y: f32,
        zoom: f32,
        sink_plugins: &mut [Box<dyn SinkPlugin>],
    ) -> Result<(), WgpuRendererError> {
        // TODO: Do we need to apply the puppet position at this time?
        let indexes: Vec<_> = self.puppet_renderers.keys().map(|i| *i).collect();
        for index in indexes {
            self.apply_viewport_to_renderer(index);
        }

        let mut render_target = self.render_target.lock().unwrap();

        render_target.resize(texture.width(), texture.height(), 0)?;

        if let Some(document) = self.document.upgrade() {
            let artboard_size = document.stage().size();
            render_target.resize(artboard_size.x as u32, artboard_size.y as u32, 1)?;

            if self.last_artboard_size != Some(artboard_size) {
                // We only send a resize notification if the plugin already
                // knows about the old size.
                if self.last_artboard_size.is_some() {
                    for plugin in sink_plugins {
                        plugin.stream_parameters_changed(
                            document.clone(),
                            "Ningyotsukai Document".to_string(),
                            artboard_size,
                            (60, 1),
                        );
                    }
                }

                self.last_artboard_size = Some(artboard_size);
            }
        }

        render_target.set_color_target_format(texture.format());
        render_target.apply(&self.device, &self.queue).unwrap();

        if let Some(camera) = render_target.viewport_camera_mut(0) {
            camera.position.x = center_x / zoom;
            camera.position.y = center_y / zoom;
            camera.scale.x = zoom;
            camera.scale.y = zoom;
        }

        if let Some(camera) = render_target.viewport_camera_mut(1) {
            camera.position.x = 0.0;
            camera.position.y = 0.0;
            camera.scale.x = 1.0;
            camera.scale.y = 1.0;
        }

        self.viewport_parameters = Some((texture.clone(), center_x, center_y, zoom));

        Ok(())
    }
}
