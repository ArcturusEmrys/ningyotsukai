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
	type TargetArray<T: Eq + Hash + Clone>: IntoIterator<Item = T> + Eq + Hash + Clone + AsRef<[T]> + AsMut<[T]>;

	fn preferred_color_targets(&self) -> Self::TargetArray<Option<wgpu::ColorTargetState>>;

	fn as_fragment_state<'a>(&'a self, color_targets: &'a [Option<wgpu::ColorTargetState>]) -> wgpu::FragmentState<'a>;
}

pub trait UniformBlock<const SIZE: usize> {
	fn write_buffer(&self, out: &mut [u8; SIZE]);

	fn into_buffer(&self, device: &wgpu::Device) -> wgpu::Buffer {
		let mut contents = [0; SIZE];
		self.write_buffer(&mut contents);

		device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some(&format!("{}::into_buffer", type_name::<Self>())), //TODO: Can we get a type name in here?
			contents: &contents,
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		})
	}
}
