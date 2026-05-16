use wgpu;
use wgpu::util::DeviceExt;

use std::any::type_name;
use std::hash::Hash;

pub trait Shader: Clone {
    fn bindgroup_layout(&self) -> &wgpu::BindGroupLayout;

    fn label(&self) -> &str;
}

pub trait VertexShader: Shader {
    fn as_vertex_state<'a>(&'a self) -> wgpu::VertexState<'a>;
}

pub trait FragmentShader: Shader {
    type TargetArray<T: Eq + Hash + Clone>: IntoIterator<Item = T>
        + Eq
        + Hash
        + Clone
        + AsRef<[T]>
        + AsMut<[T]>;

    fn preferred_color_targets(&self) -> Self::TargetArray<Option<wgpu::ColorTargetState>>;

    fn as_fragment_state<'a>(
        &'a self,
        color_targets: &'a [Option<wgpu::ColorTargetState>],
    ) -> wgpu::FragmentState<'a>;
}

/// Stupid workaround for the fact that Rust STILL doesn't support Default on
/// large array types
pub trait DefaultArray {
    fn new() -> Self;
}

impl<const N: usize> DefaultArray for [u8; N] {
    fn new() -> Self {
        [0; N]
    }
}

pub trait UniformBlock {
    type Buffer: DefaultArray
        + AsRef<[u8]>
        + AsMut<[u8]>
        + for<'a> TryFrom<&'a [u8]>
        + for<'a> TryFrom<&'a mut [u8]>
        + std::fmt::Debug;

    fn write_buffer(&self, out: &mut Self::Buffer);
}
