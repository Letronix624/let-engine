//! General utitities used throughout the engine.

use crate::camera::CameraScaling;
use glam::{Mat4, Vec2};

/// Makes an orthographic projection matrix with the given information.
pub fn ortho_maker(mode: CameraScaling, position: Vec2, zoom: f32, dimensions: Vec2) -> Mat4 {
    let dimensions = mode.scale(dimensions);
    Mat4::orthographic_rh(
        position.x - zoom * dimensions.x,
        position.x + zoom * dimensions.x,
        position.y - zoom * dimensions.y,
        position.y + zoom * dimensions.y,
        -1.0,
        1.0,
    )
}

/// Converts a Vec<u16> to a Vec<u8> where each number is split into it's high and low bytes.
pub fn u16tou8vec(data: Vec<u16>) -> Vec<u8> {
    // to utils.rs in the future
    data.iter()
        .flat_map(|&u16_value| {
            let high_byte = ((u16_value >> 8) & 0xff) as u8;
            let low_byte = (u16_value & 0xff) as u8;
            vec![high_byte, low_byte]
        })
        .collect()
}
