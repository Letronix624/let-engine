//! Holds model related data structures like Vertices and premade models as well as a circle maker macro.

use super::model::Vertex;
use bytemuck::AnyBitPattern;
use glam::f32::{vec2, Vec2, Vec4};

/// A vertex containing a position (xy) and texture position (uv).
#[repr(C)]
#[derive(AnyBitPattern, Vertex, Debug, Clone, Copy, PartialEq)]
pub struct TVert {
    #[format(Rg32Float)]
    pub position: Vec2,
    #[format(Rg32Float)]
    pub tex_position: Vec2,
}

/// Creates a vertex with given x and y coordinates and tx and ty coordinates for the UV texture mapping.
#[inline]
pub const fn tvert(x: f32, y: f32, tx: f32, ty: f32) -> TVert {
    TVert {
        position: vec2(x, y),
        tex_position: vec2(tx, ty),
    }
}

/// A vertex containing just a position (xy) R32G32_SFLOAT.
#[repr(C)]
#[derive(AnyBitPattern, Vertex, Debug, Clone, Copy, PartialEq)]
pub struct Vert {
    #[format(Rg32Float)]
    pub position: Vec2,
}

impl From<Vec2> for Vert {
    fn from(value: Vec2) -> Self {
        Self { position: value }
    }
}

/// Creates a vertex with given x and y coordinates.
#[inline]
pub const fn vert(x: f32, y: f32) -> Vert {
    Vert {
        position: vec2(x, y),
    }
}

// /// MVP matrix.
// #[repr(C)]
// #[derive(Clone, Copy, Debug, PartialEq, BufferContents)]
// pub(crate) struct ModelViewProj {
//     pub model: Mat4,
//     pub view: Mat4,
//     pub proj: Mat4,
// }

// /// Default instance data.
// #[repr(C)]
// #[derive(Clone, Copy, Debug, PartialEq, BufferContents, VTX)]
// pub(crate) struct InstanceData {
//     #[format(R32G32B32A32_SFLOAT)]
//     pub color: Vec4,
//     #[format(R32_UINT)]
//     pub layer: u32,
//     #[format(R32G32B32A32_SFLOAT)]
//     pub model: Mat4,
//     #[format(R32G32B32A32_SFLOAT)]
//     pub view: Mat4,
//     #[format(R32G32B32A32_SFLOAT)]
//     pub proj: Mat4,
// }

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Vertex, AnyBitPattern)]
pub(crate) struct ObjectFrag {
    #[format(Rgba32Float)]
    pub color: Vec4,
    #[format(R32Float)]
    pub texture_id: u32,
}

impl Default for ObjectFrag {
    fn default() -> Self {
        Self {
            color: Vec4::splat(0.0),
            texture_id: 0,
        }
    }
}
