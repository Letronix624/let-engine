//! Holds model related data structures like Vertices and premade models as well as a circle maker macro.

use super::model::Vertex;
use bytemuck::AnyBitPattern;
use glam::{
    Vec3,
    f32::{Vec2, vec2},
};

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

impl TVert {
    #[inline]
    pub const fn new(position: Vec2, tex_position: Vec2) -> Self {
        Self {
            position,
            tex_position,
        }
    }
}

/// Creates a vertex with given x and y coordinates and tx and ty coordinates for the UV texture mapping.
#[inline]
pub const fn tvert(x: f32, y: f32, tx: f32, ty: f32) -> TVert {
    TVert::new(vec2(x, y), vec2(tx, ty))
}

#[repr(C)]
#[derive(AnyBitPattern, Vertex, Debug, Clone, Copy, PartialEq)]
pub struct Vert3D {
    #[format(Rgb32Float)]
    pub position: Vec3,
    #[format(Rg32Float)]
    pub tex_position: Vec2,
    #[format(Rgb32Float)]
    pub normal: Vec3,
}

impl Vert3D {
    #[inline]
    pub const fn new(position: Vec3, tex_position: Vec2, normal: Vec3) -> Self {
        Vert3D {
            position,
            tex_position,
            normal,
        }
    }
}
