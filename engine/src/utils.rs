//! General utitities used throughout the engine.

use crate::camera::CameraScaling;
use core::f32::consts::FRAC_1_SQRT_2;
use glam::{Mat4, Vec2, vec2};

/// Makes an orthographic projection matrix with the given information.
pub fn ortho_maker(mode: CameraScaling, position: Vec2, zoom: f32, dimensions: Vec2) -> Mat4 {
    let dimensions = scale(mode, dimensions);
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

/// Scales the given dimensions using the given scaling algorithm.
pub fn scale(mode: CameraScaling, dimensions: Vec2) -> Vec2 {
    match mode {
        CameraScaling::Stretch => vec2(1.0, 1.0),
        CameraScaling::Linear => vec2(
            0.5 / (dimensions.y / (dimensions.x + dimensions.y)),
            0.5 / (dimensions.x / (dimensions.x + dimensions.y)),
        ),
        CameraScaling::Circle => vec2(
            1.0 / (dimensions.y.atan2(dimensions.x).sin() / FRAC_1_SQRT_2),
            1.0 / (dimensions.y.atan2(dimensions.x).cos() / FRAC_1_SQRT_2),
        ),
        CameraScaling::Limited => vec2(
            1.0 / (dimensions.y / dimensions.x.clamp(0.0, dimensions.y)),
            1.0 / (dimensions.x / dimensions.y.clamp(0.0, dimensions.x)),
        ),
        CameraScaling::Expand => vec2(dimensions.x * 0.001, dimensions.y * 0.001),
    }
}
