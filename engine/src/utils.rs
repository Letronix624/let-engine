use crate::camera::CameraScaling;
use glam::{Mat4, Vec2};

pub fn ortho_maker(mode: CameraScaling, position: Vec2, zoom: f32, dimensions: (f32, f32)) -> Mat4 {
    let (width, height) = crate::game::objects::scale(mode, dimensions);
    Mat4::orthographic_rh(
        position.x - zoom * width,
        position.x + zoom * width,
        position.y - zoom * height,
        position.y + zoom * height,
        -1.0,
        1.0,
    )
}

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
