//! Holds model related data structures like Vertices and premade models as well as a circle maker macro.

use super::model::Vertex;
use bytemuck::AnyBitPattern;
use glam::f32::{vec2, Vec2};

pub trait Data: AnyBitPattern + Send + Sync {}

impl<T> Data for T where T: AnyBitPattern + Send + Sync {}

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
