use std::{any::type_name, marker::PhantomData};

use wgpu::util::DeviceExt;

use crate::shader::{DefaultArray, UniformBlock};

#[derive(Debug)]
pub struct BufferBuilder<B>
where
    B: UniformBlock,
{
    phantom: PhantomData<B>,
    data: Vec<u8>,
    alignment_requirement: u32,
}

impl<B> BufferBuilder<B>
where
    B: UniformBlock,
{
    /// Create a new Buffer Builder that can generate buffers compliant with
    /// the specified GPU limits.
    pub fn new(limits: wgpu::Limits) -> Self {
        Self {
            phantom: PhantomData::default(),
            data: Vec::new(),
            alignment_requirement: limits.min_uniform_buffer_offset_alignment,
        }
    }

    /// Clear the data in the builder.
    ///
    /// This deliberately keeps the same capacity as before so we can reuse the
    /// allocation.
    pub fn clear(&mut self) {
        self.data.resize(0, 0);
    }

    /// Insert a uniform block into the builder.
    ///
    /// This function returns an index into the builder that will be valid for
    /// the next buffer returned from `commit`.
    pub fn insert(&mut self, uniform: B) -> usize {
        let misalignment = self.alignment_requirement as usize
            - (self.data.len() % self.alignment_requirement as usize);
        let padding = misalignment % self.alignment_requirement as usize;
        if padding != 0 {
            self.data.resize(self.data.len() + padding, 0);
        }

        let start = self.data.len();

        let mut buffer = B::Buffer::new();
        uniform.write_buffer(&mut buffer);

        self.data.extend_from_slice(buffer.as_ref());

        start
    }

    /// Write the built buffer out to the GPU for rendering.
    ///
    /// This also clears the internal buffer for building the next one.
    pub fn commit(&mut self, device: &wgpu::Device) -> wgpu::Buffer {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("BufferBuilder<{}>::commit", type_name::<B>())),
            contents: self.data.as_ref(),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        self.clear();

        buffer
    }
}
