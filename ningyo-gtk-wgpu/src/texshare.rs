#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

pub trait TryIntoGdkTexture {
    fn into_gdk_texture(
        self,
        device: &wgpu::Device,
        display: &gdk4::Display,
        old_texture: Option<gdk4::Texture>,
    ) -> Result<gdk4::Texture, Box<dyn std::error::Error>>;
}
