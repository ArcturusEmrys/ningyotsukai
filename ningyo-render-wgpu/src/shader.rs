use std::hash::Hash;
use wgpu;

pub trait Shader: Clone {
    fn bindgroup_layout(&self) -> &[Option<wgpu::BindGroupLayout>];

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

impl UniformBlock for wgpu::util::DrawIndexedIndirectArgs {
    fn static_size() -> usize {
        std::mem::size_of::<Self>()
    }

    fn required_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }

    fn write_buffer(&self, out: &mut [u8]) {
        out.copy_from_slice(self.as_bytes());
    }
}
