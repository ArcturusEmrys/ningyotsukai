use glam::Mat4;

use inox2d::math::camera::Camera;

pub trait CameraExt {
    /// Calculate the transform from model coordinates to "artboard coordinates"
    /// or any other matrix that does not include the final transformation into
    /// normalized coordinates.
    fn to_artboard_matrix(&self) -> Mat4;
}

impl CameraExt for Camera {
    fn to_artboard_matrix(&self) -> Mat4 {
        Mat4::from_scale(self.scale.extend(1.0))
            * Mat4::from_rotation_z(self.rotation)
            * Mat4::from_translation(self.position.extend(-(u16::MAX as f32 / 2.0)))
    }
}
