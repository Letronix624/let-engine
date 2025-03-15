//! General utitities used throughout the engine.

use crate::camera::CameraScaling;
use glam::{Mat4, Vec2};

/// Makes an orthographic projection matrix with the given information.
pub fn ortho_maker(mode: CameraScaling, position: Vec2, zoom: Vec2, dimensions: Vec2) -> Mat4 {
    let dimensions = mode.scale(dimensions);
    Mat4::orthographic_rh(
        position.x - zoom.x * dimensions.x,
        position.x + zoom.x * dimensions.x,
        position.y - zoom.y * dimensions.y,
        position.y + zoom.y * dimensions.y,
        -1.0,
        1.0,
    )
}
