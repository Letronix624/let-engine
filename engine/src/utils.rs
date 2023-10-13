//! General utitities used throughout the engine.

use crate::camera::CameraScaling;
use color_art::Color;
use core::f32::consts::FRAC_1_SQRT_2;
use glam::{Mat4, Vec2};

/// Makes an orthographic projection matrix with the given information.
pub fn ortho_maker(mode: CameraScaling, position: Vec2, zoom: f32, dimensions: (f32, f32)) -> Mat4 {
    let (width, height) = scale(mode, dimensions);
    Mat4::orthographic_rh(
        position.x - zoom * width,
        position.x + zoom * width,
        position.y - zoom * height,
        position.y + zoom * height,
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
pub fn scale(mode: CameraScaling, dimensions: (f32, f32)) -> (f32, f32) {
    match mode {
        CameraScaling::Stretch => (1.0, 1.0),
        CameraScaling::Linear => (
            0.5 / (dimensions.1 / (dimensions.0 + dimensions.1)),
            0.5 / (dimensions.0 / (dimensions.0 + dimensions.1)),
        ),
        CameraScaling::Circle => (
            1.0 / (dimensions.1.atan2(dimensions.0).sin() / FRAC_1_SQRT_2),
            1.0 / (dimensions.1.atan2(dimensions.0).cos() / FRAC_1_SQRT_2),
        ),
        CameraScaling::Limited => (
            1.0 / (dimensions.1 / dimensions.0.clamp(0.0, dimensions.1)),
            1.0 / (dimensions.0 / dimensions.1.clamp(0.0, dimensions.0)),
        ),
        CameraScaling::Expand => (dimensions.0 * 0.001, dimensions.1 * 0.001),
    }
}

pub fn color_art_to_array(color: Color) -> [f32; 4] {
    [
        color.red() as f32 / 255.0,
        color.green() as f32 / 255.0,
        color.blue() as f32 / 255.0,
        color.alpha() as f32,
    ]
}
