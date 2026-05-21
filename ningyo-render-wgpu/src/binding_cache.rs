use std::collections::HashMap;
use std::num::NonZero;

use crate::WgpuResources;

use crate::shader::UniformBlock;
use crate::shaders::basic::{basic_frag, basic_mask_frag, basic_vert, composite_frag};

/// Cache for all of our needed bind groups.
///
/// Ideally, we should never allocate a BindGroup during a rendering call.
#[derive(Debug, Clone, Default)]
pub struct BindingCache {
    /// The bind group used for basic_vert.
    ///
    /// The associated Buffer object is the buffer bound to the shader in the
    /// given bindgroup. If the buffer changes, the previous binding is
    /// invalidated.
    basic_vert_bind: Option<(wgpu::BindGroup, wgpu::Buffer)>,

    /// The cache of bind groups used for basic_frag.
    ///
    /// We permit a different BindGroup for each texture combination; but we
    /// do not permit multiple uniform buffers to live in the cache at the same
    /// time. All bindgroups must reference the same buffer, and if the buffer
    /// changes, the old bind groups are invalidated.
    basic_frag_bind:
        HashMap<(wgpu::TextureView, wgpu::TextureView, wgpu::TextureView), wgpu::BindGroup>,

    /// The last buffer used to access the basic_frag cache.
    ///
    /// If it changes, the entire cache is erased.
    last_basic_frag_bind_buffer: Option<wgpu::Buffer>,

    /// The cache of bind groups used for basic_mask_frag.
    ///
    /// All bindgroups must reference the same buffer, and if the buffer
    /// changes, the old bind groups are invalidated.
    basic_mask_frag_bind: HashMap<wgpu::TextureView, wgpu::BindGroup>,

    /// The last buffer used to access the basic_mask_frag cache.
    ///
    /// If it changes, the entire cache is erased.
    last_basic_mask_frag_bind_buffer: Option<wgpu::Buffer>,

    /// The bind group used for composite_frag.
    ///
    /// This shader accepts the compositing render targets (aka "GBuffer") only
    /// so we only permit one bind group to be cached at any one time.
    composite_frag_bind: Option<(
        wgpu::BindGroup,
        wgpu::TextureView,
        wgpu::TextureView,
        wgpu::TextureView,
        wgpu::Buffer,
    )>,
}

impl BindingCache {
    pub fn bind_basic_vert(
        &mut self,
        resources: &WgpuResources,
        buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        if let Some((bg, my_buffer)) = &self.basic_vert_bind {
            if buffer == my_buffer {
                return bg.clone();
            }
        }

        let new_bg = resources.part_shader_vert.bind(
            &resources.device,
            wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: Some(
                    NonZero::new(size_of::<<basic_vert::Input as UniformBlock>::Buffer>() as u64)
                        .unwrap(),
                ),
            },
        );

        self.basic_vert_bind = Some((new_bg.clone(), buffer.clone()));

        new_bg
    }

    pub fn bind_basic_frag(
        &mut self,
        resources: &WgpuResources,
        albedo: &wgpu::TextureView,
        emissive: &wgpu::TextureView,
        bumpmap: &wgpu::TextureView,
        buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        // Fun wrinkle of the Rust HashMap API is that I have to bump the
        // reference count every time I want to query these texture views
        let key = (albedo.clone(), emissive.clone(), bumpmap.clone());
        if let Some(bg) = self.basic_frag_bind.get(&key) {
            // NOTE: Not recording the buffer is an internal logic error,
            // so a panic is appropriate
            if buffer == self.last_basic_frag_bind_buffer.as_ref().unwrap() {
                return bg.clone();
            } else {
                self.basic_frag_bind = HashMap::new();
            }
        }

        let new_bg = resources.part_shader_frag.bind(
            &resources.device,
            albedo,
            emissive,
            bumpmap,
            &resources.model_sampler,
            wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: Some(
                    NonZero::new(size_of::<<basic_frag::Input as UniformBlock>::Buffer>() as u64)
                        .unwrap(),
                ),
            },
        );

        self.basic_frag_bind.insert(key, new_bg.clone());
        self.last_basic_frag_bind_buffer = Some(buffer.clone());

        new_bg
    }

    pub fn bind_basic_mask_frag(
        &mut self,
        resources: &WgpuResources,
        albedo: &wgpu::TextureView,
        buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        if let Some(bg) = self.basic_mask_frag_bind.get(albedo) {
            // NOTE: Not recording the buffer is an internal logic error,
            // so a panic is appropriate
            if buffer == self.last_basic_mask_frag_bind_buffer.as_ref().unwrap() {
                return bg.clone();
            } else {
                self.basic_mask_frag_bind = HashMap::new();
            }
        }

        let new_bg = resources.part_shader_mask_frag.bind(
            &resources.device,
            albedo,
            &resources.model_sampler,
            wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: Some(
                    NonZero::new(
                        size_of::<<basic_mask_frag::Input as UniformBlock>::Buffer>() as u64,
                    )
                    .unwrap(),
                ),
            },
        );

        self.basic_mask_frag_bind
            .insert(albedo.clone(), new_bg.clone());
        self.last_basic_mask_frag_bind_buffer = Some(buffer.clone());

        new_bg
    }

    pub fn bind_composite_frag(
        &mut self,
        resources: &WgpuResources,
        albedo: &wgpu::TextureView,
        emissive: &wgpu::TextureView,
        bump: &wgpu::TextureView,
        buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        if let Some((bg, my_albedo, my_emissive, my_bump, my_buffer)) = &self.composite_frag_bind {
            if buffer == my_buffer
                && albedo == my_albedo
                && emissive == my_emissive
                && bump == my_bump
            {
                return bg.clone();
            }
        }

        let new_bg = resources.composite_shader_frag.bind(
            &resources.device,
            albedo,
            emissive,
            bump,
            &resources.model_sampler,
            wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: Some(
                    NonZero::new(
                        size_of::<<composite_frag::Input as UniformBlock>::Buffer>() as u64
                    )
                    .unwrap(),
                ),
            },
        );

        self.basic_vert_bind = Some((new_bg.clone(), buffer.clone()));

        new_bg
    }
}
