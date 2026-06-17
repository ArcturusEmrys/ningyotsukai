use std::collections::HashMap;
use std::num::NonZero;

use crate::WgpuResources;

use crate::shader::UniformBlock;
use crate::shaders::basic::{basic_frag, basic_mask_frag, basic_vert, composite_frag};

/// Cache for all of our needed bind groups.
///
/// Ideally, we should never allocate a BindGroup during a rendering call.
#[derive(Debug, Clone, Default)]
pub struct BindingCache<'a> {
    tether: Option<&'a BindingCache<'a>>,

    /// The bind group used for basic_vert.
    ///
    /// The two associated Buffer objects are the buffers bound to the shader
    /// in the given bindgroup. The first one is the per-draw uniforms buffer;
    /// the second is the viewport configuration buffer.
    ///
    /// If either buffer changes, the previous binding is invalidated.
    ///
    /// This stores two BindGroups; the second is shared with and identical
    /// across all frag shader bindings.
    basic_vert_bind: Option<(wgpu::BindGroup, wgpu::BindGroup, wgpu::Buffer, wgpu::Buffer)>,

    /// The cache of bind groups used for basic_frag.
    ///
    /// We permit a different BindGroup for each texture combination; but we
    /// do not permit multiple uniform buffers to live in the cache at the same
    /// time. All bindgroups must reference the same buffer, and if the buffer
    /// changes, the old bind groups are invalidated.
    ///
    /// The bindgroups here go into slots 1 and 3.
    basic_frag_bind: HashMap<Vec<wgpu::TextureView>, (wgpu::BindGroup, wgpu::BindGroup)>,

    /// The last uniform and viewport settings buffer used to access the
    /// basic_frag cache.
    ///
    /// If it changes, the entire cache is erased.
    last_basic_frag_bind_buffer: Option<(wgpu::Buffer, wgpu::Buffer)>,

    /// The cache of bind groups used for basic_mask_frag.
    ///
    /// All bindgroups must reference the same buffer, and if the buffer
    /// changes, the old bind groups are invalidated.
    ///
    /// The bindgroups here go into slots 1 and 3.
    basic_mask_frag_bind: HashMap<Vec<wgpu::TextureView>, (wgpu::BindGroup, wgpu::BindGroup)>,

    /// The last uniform and viewport settings buffer used to access the
    /// basic_mask_frag cache.
    ///
    /// If it changes, the entire cache is erased.
    last_basic_mask_frag_bind_buffer: Option<(wgpu::Buffer, wgpu::Buffer)>,

    /// The bind group used for composite_vert.
    ///
    /// This shader only accepts the viewport configuration, which should be
    /// static across renderers, so we only permit one BindGroup in the cache
    /// at a time.
    composite_vert_bind: Option<(wgpu::BindGroup, wgpu::Buffer)>,

    /// The bind group used for composite_frag.
    ///
    /// This shader accepts the compositing render targets (aka "GBuffer") only
    /// so we only permit one bind group to be cached at any one time.
    ///
    /// The two buffers in the list are the uniform and viewport settings
    /// buffers, in that order.
    ///
    /// The bindgroups here go into slots 1 and 3.
    composite_frag_bind: Option<(
        wgpu::BindGroup,
        wgpu::BindGroup,
        wgpu::TextureView,
        wgpu::TextureView,
        wgpu::TextureView,
        wgpu::Buffer,
        wgpu::Buffer,
    )>,
}

impl<'a> BindingCache<'a> {
    /// Create a subsidiary BindingCache that borrows data from another one.
    ///
    /// Bindings stored in the borrowed binding cache will be accessible in the
    /// returned cache, but any new data will be stored in the subsidiary cache.
    /// You will need to merge the cache items back into the parent in order to
    /// retain them.
    ///
    /// This is primarily intended for multithreaded rendering scenarios; you
    /// can create multiple split binding caches and then merge them later. You
    /// can also use this for immutable access to a single binding cache.
    pub fn split<'b>(&'b self) -> BindingCache<'b>
    where
        'b: 'a,
    {
        Self {
            tether: Some(self),
            basic_vert_bind: None,
            basic_frag_bind: HashMap::new(),
            last_basic_frag_bind_buffer: None,
            basic_mask_frag_bind: HashMap::new(),
            last_basic_mask_frag_bind_buffer: None,
            composite_vert_bind: None,
            composite_frag_bind: None,
        }
    }

    /// Remove connection to the borrowed binding cache and its lifetime.
    pub fn untether(mut self) -> BindingCache<'static> {
        // For reasons I don't totally understand, pulling all the owned
        // variables out of a struct with a lifetime on it does NOT erase the
        // lifetime I'm trying to get rid of, so we have to do this.
        let mut new_cache = BindingCache::default();
        let old_self = std::mem::replace(&mut self, BindingCache::default());

        new_cache.basic_vert_bind = old_self.basic_vert_bind;
        new_cache.basic_frag_bind = old_self.basic_frag_bind;
        new_cache.last_basic_frag_bind_buffer = old_self.last_basic_frag_bind_buffer;
        new_cache.basic_mask_frag_bind = old_self.basic_mask_frag_bind;
        new_cache.last_basic_mask_frag_bind_buffer = old_self.last_basic_mask_frag_bind_buffer;
        new_cache.composite_vert_bind = old_self.composite_vert_bind;
        new_cache.composite_frag_bind = old_self.composite_frag_bind;

        new_cache
    }

    pub fn bind_basic_vert(
        &mut self,
        resources: &WgpuResources,
        buffer: &wgpu::Buffer,
        viewports: &wgpu::Buffer,
    ) -> (wgpu::BindGroup, wgpu::BindGroup) {
        if let Some(tether) = self.tether {
            if let Some((bg0, bg2, my_buffer, my_viewports)) = &tether.basic_vert_bind {
                if buffer == my_buffer && viewports == my_viewports {
                    return (bg0.clone(), bg2.clone());
                }
            }
        }

        if let Some((bg0, bg2, my_buffer, my_viewports)) = &self.basic_vert_bind {
            if buffer == my_buffer && viewports == my_viewports {
                return (bg0.clone(), bg2.clone());
            } else {
                #[cfg(feature = "timing")]
                eprintln!("      (basic_vert buffer changed!)");
            }
        }

        let new_bg0 = resources.part_shader_vert.bind_0(
            &resources.device,
            wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: None,
            },
        );

        let new_bg2 = resources.part_shader_vert.bind_2(
            &resources.device,
            wgpu::BufferBinding {
                buffer: viewports,
                offset: 0,
                size: Some(NonZero::new(basic_vert::Viewport::static_size() as u64).unwrap()),
            },
        );

        self.basic_vert_bind = Some((
            new_bg0.clone(),
            new_bg2.clone(),
            buffer.clone(),
            viewports.clone(),
        ));

        (new_bg0, new_bg2)
    }

    pub fn bind_basic_frag(
        &mut self,
        resources: &WgpuResources,
        model_textures: &[&wgpu::TextureView],
        buffer: &wgpu::Buffer,
        viewports: &wgpu::Buffer,
    ) -> (wgpu::BindGroup, wgpu::BindGroup) {
        let textures_key: Vec<_> = model_textures.iter().map(|c| (*c).clone()).collect();

        if let Some(tether) = self.tether {
            if let Some((bg1, bg3)) = tether.basic_frag_bind.get(&textures_key) {
                // NOTE: Not recording the buffer is an internal logic error,
                // so a panic is appropriate
                let (last_buffer, last_viewports) =
                    tether.last_basic_frag_bind_buffer.as_ref().unwrap();
                if buffer == last_buffer && viewports == last_viewports {
                    return (bg1.clone(), bg3.clone());
                }
            }
        }

        if let Some((bg1, bg3)) = self.basic_frag_bind.get(&textures_key) {
            let (last_buffer, last_viewports) = self.last_basic_frag_bind_buffer.as_ref().unwrap();
            if buffer == last_buffer && viewports == last_viewports {
                return (bg1.clone(), bg3.clone());
            } else {
                #[cfg(feature = "timing")]
                eprintln!("      (basic_frag_bind buffers changed!)");
                self.basic_frag_bind = HashMap::new();
            }
        }

        let new_bg1 = resources.part_shader_frag.bind_1(
            &resources.device,
            model_textures,
            &resources.model_sampler,
            wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: None,
            },
        );
        let new_bg3 = resources.part_shader_frag.bind_3(
            &resources.device,
            wgpu::BufferBinding {
                buffer: viewports,
                offset: 0,
                size: Some(NonZero::new(basic_frag::Viewport::static_size() as u64).unwrap()),
            },
        );

        self.basic_frag_bind
            .insert(textures_key, (new_bg1.clone(), new_bg3.clone()));
        self.last_basic_frag_bind_buffer = Some((buffer.clone(), viewports.clone()));

        (new_bg1, new_bg3)
    }

    pub fn bind_basic_mask_frag(
        &mut self,
        resources: &WgpuResources,
        model_textures: &[&wgpu::TextureView],
        buffer: &wgpu::Buffer,
        viewport: &wgpu::Buffer,
    ) -> (wgpu::BindGroup, wgpu::BindGroup) {
        let textures_key: Vec<_> = model_textures.iter().map(|c| (*c).clone()).collect();

        if let Some(tether) = self.tether {
            if let Some((bg1, bg3)) = tether.basic_mask_frag_bind.get(&textures_key) {
                // NOTE: Not recording the buffer is an internal logic error,
                // so a panic is appropriate
                let (last_buffer, last_viewport) =
                    tether.last_basic_mask_frag_bind_buffer.as_ref().unwrap();
                if buffer == last_buffer && viewport == last_viewport {
                    return (bg1.clone(), bg3.clone());
                }
            }
        }

        if let Some((bg1, bg3)) = self.basic_mask_frag_bind.get(&textures_key) {
            let (last_buffer, last_viewport) =
                self.last_basic_mask_frag_bind_buffer.as_ref().unwrap();
            if buffer == last_buffer && viewport == last_viewport {
                return (bg1.clone(), bg3.clone());
            } else {
                #[cfg(feature = "timing")]
                eprintln!("      (basic_mask_frag_bind buffers changed!)");
                self.basic_mask_frag_bind = HashMap::new();
            }
        }

        let new_bg1 = resources.part_shader_mask_frag.bind_1(
            &resources.device,
            model_textures,
            &resources.model_sampler,
            wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: None,
            },
        );
        let new_bg3 = resources.part_shader_mask_frag.bind_3(
            &resources.device,
            wgpu::BufferBinding {
                buffer: viewport,
                offset: 0,
                size: Some(NonZero::new(basic_mask_frag::Viewport::static_size() as u64).unwrap()),
            },
        );

        self.basic_mask_frag_bind
            .insert(textures_key, (new_bg1.clone(), new_bg3.clone()));
        self.last_basic_mask_frag_bind_buffer = Some((buffer.clone(), viewport.clone()));

        (new_bg1, new_bg3)
    }

    pub fn bind_composite_vert(
        &mut self,
        resources: &WgpuResources,
        viewports: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        if let Some(tether) = self.tether {
            if let Some((bg, my_viewports)) = &tether.composite_vert_bind {
                if viewports == my_viewports {
                    return bg.clone();
                }
            }
        }

        if let Some((bg, my_viewports)) = &self.composite_vert_bind {
            if viewports == my_viewports {
                return bg.clone();
            } else {
                #[cfg(feature = "timing")]
                eprintln!("      (composite_vert buffer changed!)");
            }
        }

        let new_bg = resources.composite_shader_vert.bind_2(
            &resources.device,
            wgpu::BufferBinding {
                buffer: viewports,
                offset: 0,
                size: Some(NonZero::new(basic_frag::Viewport::static_size() as u64).unwrap()),
            },
        );

        self.composite_vert_bind = Some((new_bg.clone(), viewports.clone()));

        new_bg
    }

    pub fn bind_composite_frag(
        &mut self,
        resources: &WgpuResources,
        albedo: &wgpu::TextureView,
        emissive: &wgpu::TextureView,
        bump: &wgpu::TextureView,
        buffer: &wgpu::Buffer,
        viewports: &wgpu::Buffer,
    ) -> (wgpu::BindGroup, wgpu::BindGroup) {
        if let Some(tether) = self.tether {
            if let Some((bg1, bg3, my_albedo, my_emissive, my_bump, my_buffer, my_viewports)) =
                &tether.composite_frag_bind
            {
                if buffer == my_buffer
                    && albedo == my_albedo
                    && emissive == my_emissive
                    && bump == my_bump
                    && viewports == my_viewports
                {
                    return (bg1.clone(), bg3.clone());
                }
            }
        }

        if let Some((bg1, bg3, my_albedo, my_emissive, my_bump, my_buffer, my_viewports)) =
            &self.composite_frag_bind
        {
            if buffer == my_buffer
                && albedo == my_albedo
                && emissive == my_emissive
                && bump == my_bump
                && viewports == my_viewports
            {
                return (bg1.clone(), bg3.clone());
            } else {
                #[cfg(feature = "timing")]
                eprintln!("      (composite_frag buffer or textures changed!)");
            }
        }

        let new_bg1 = resources.composite_shader_frag.bind_1(
            &resources.device,
            albedo,
            emissive,
            bump,
            &resources.model_sampler,
            wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: Some(NonZero::new(composite_frag::Input::static_size() as u64).unwrap()),
            },
        );
        let new_bg3 = resources.composite_shader_frag.bind_3(
            &resources.device,
            wgpu::BufferBinding {
                buffer: viewports,
                offset: 0,
                size: Some(NonZero::new(basic_mask_frag::Viewport::static_size() as u64).unwrap()),
            },
        );

        self.composite_frag_bind = Some((
            new_bg1.clone(),
            new_bg3.clone(),
            albedo.clone(),
            emissive.clone(),
            bump.clone(),
            buffer.clone(),
            viewports.clone(),
        ));

        (new_bg1, new_bg3)
    }

    /// Merge in another binding cache into this one.
    ///
    /// This allows multithreaded access to the binding cache to account for
    /// new objects that were created during processing.
    pub fn merge(&mut self, other: BindingCache<'_>) {
        if let Some(other_bvb) = other.basic_vert_bind {
            self.basic_vert_bind = Some(other_bvb);
        }

        if other.last_basic_frag_bind_buffer.is_some() {
            if other.last_basic_frag_bind_buffer == self.last_basic_frag_bind_buffer {
                for (kv, bg) in other.basic_frag_bind {
                    self.basic_frag_bind.insert(kv, bg);
                }
            } else {
                self.basic_frag_bind = other.basic_frag_bind;
                self.last_basic_frag_bind_buffer = other.last_basic_frag_bind_buffer;
            }
        }

        if other.last_basic_mask_frag_bind_buffer.is_some() {
            if other.last_basic_mask_frag_bind_buffer == self.last_basic_mask_frag_bind_buffer {
                for (k, bg) in other.basic_mask_frag_bind {
                    self.basic_mask_frag_bind.insert(k, bg);
                }
            } else {
                self.basic_mask_frag_bind = other.basic_mask_frag_bind;
                self.last_basic_mask_frag_bind_buffer = other.last_basic_mask_frag_bind_buffer;
            }
        }

        if let Some(other_cvb) = other.composite_vert_bind {
            self.composite_vert_bind = Some(other_cvb);
        }

        if let Some(other_cfb) = other.composite_frag_bind {
            self.composite_frag_bind = Some(other_cfb);
        }
    }

    /// Measure how many BindGroup creations this BindingCache had to process.
    ///
    /// This isn't particularly meaningful outside of split operation.
    pub fn delta_len(&self) -> (usize, usize, usize, usize, usize) {
        (
            self.basic_vert_bind.iter().len(),
            self.basic_frag_bind.len(),
            self.basic_mask_frag_bind.len(),
            self.composite_vert_bind.iter().len(),
            self.composite_frag_bind.iter().len(),
        )
    }
}
