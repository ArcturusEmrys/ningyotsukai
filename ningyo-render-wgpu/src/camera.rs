use glam::{Mat4, Vec2};

use inox2d::math::camera::Camera;

pub trait CameraExt {
    /// Calculate the transform from model coordinates to "artboard coordinates"
    /// or any other matrix that does not include the final transformation into
    /// normalized coordinates.
    fn to_artboard_matrix(&self) -> Mat4;

    /// Calculate the transform from "artboard coordinates" to the final range
    /// of normalized device coordinates.
    ///
    /// The viewport size should be the width and height of the target render
    /// texture, *not* including any subdivision of such a texture.
    fn to_view_proj_matrix(&self, viewport: Vec2) -> Mat4;
}

impl CameraExt for Camera {
    fn to_artboard_matrix(&self) -> Mat4 {
        Mat4::from_scale(self.scale.extend(1.0))
            * Mat4::from_rotation_z(self.rotation)
            * Mat4::from_translation(self.position.extend(-(u16::MAX as f32 / 2.0)))
    }

    fn to_view_proj_matrix(&self, viewport: Vec2) -> Mat4 {
        let pos = self.position.extend(-(u16::MAX as f32 / 2.0));

        // Return camera ortho matrix
        Mat4::orthographic_lh(0.0, viewport.x, viewport.y, 0.0, 0.0, u16::MAX as f32)
            * Mat4::from_scale(self.scale.extend(1.0))
            * Mat4::from_rotation_z(self.rotation)
            * Mat4::from_translation(pos)
    }
}
