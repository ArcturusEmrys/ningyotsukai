use std::hash::Hash;
use wgpu;

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

pub trait UniformBlock {
    fn static_size() -> usize;

    fn required_size(&self) -> usize;

    fn write_buffer(&self, out: &mut [u8]);
}
