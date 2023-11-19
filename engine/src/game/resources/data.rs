//! Holds model related data structures like Vertices and premade models as well as a circle maker macro.

use glam::f32::{vec2, Mat4, Vec2, Vec4};
use vulkano::{buffer::BufferContents, pipeline::graphics::vertex_input::Vertex as VTX};

use super::{ModelData, Resources};

/// A vertex containing it's position (xy) and texture position (uv).
#[repr(C)]
#[derive(BufferContents, VTX, Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    #[format(R32G32_SFLOAT)]
    pub position: Vec2,
    #[format(R32G32_SFLOAT)]
    pub tex_position: Vec2,
}

// vert2d in the future
/// Creates a vertex with given x and y coordinates for both position and texture position.
#[inline]
pub const fn vert(x: f32, y: f32) -> Vertex {
    Vertex {
        position: vec2(x, y),
        tex_position: vec2(x, y),
    }
}
// tvert2d
/// Creates a vertex with given x and y coordinates for position and given tx and ty coordinates for the UV texture mapping for those points.
#[inline]
pub const fn tvert(x: f32, y: f32, tx: f32, ty: f32) -> Vertex {
    Vertex {
        position: vec2(x, y),
        tex_position: vec2(tx, ty),
    }
}

/// MVP matrix.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, BufferContents)]
pub(crate) struct ModelViewProj {
    pub model: Mat4,
    pub view: Mat4,
    pub proj: Mat4,
}

/// Default instance data.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, BufferContents, VTX)]
pub(crate) struct InstanceData {
    #[format(R32G32B32A32_SFLOAT)]
    pub color: Vec4,
    #[format(R32_UINT)]
    pub layer: u32,
    #[format(R32G32B32A32_SFLOAT)]
    pub model: Mat4,
    #[format(R32G32B32A32_SFLOAT)]
    pub view: Mat4,
    #[format(R32G32B32A32_SFLOAT)]
    pub proj: Mat4,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, BufferContents)]
pub(crate) struct ObjectFrag {
    pub color: Vec4,
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

/// Vertex and index data for the appearance and shape of objects.
/// Has 3 simple presets.
///
/// Empty, Square and Triangle.
///
/// Right now it only supports 2d model data.
///
/// The models must have vertices and indices.
///
/// up is -y, down is +y.
/// right is +x und left is -x.
#[derive(Debug, Clone, PartialEq)]
pub struct Data {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Data {
    pub const fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }
    /// Returns the data of a square that goes from `-1.0` to `1.0` in both X and Y.
    pub fn square() -> Self {
        Data {
            vertices: SQUARE.into(),
            indices: SQUARE_ID.into(),
        }
    }
    /// Returns the data of a triangle with the points `[0.0, -1.0], [-1.0, 1.0], [1.0, 1.0]`.
    pub fn triangle() -> Self {
        Data {
            vertices: TRIANGLE.into(),
            indices: TRIANGLE_ID.into(),
        }
    }
    /// Returns if the data has an empty field.
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() || self.indices.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BasicShapes {
    pub square: ModelData,
    pub triangle: ModelData,
}

impl BasicShapes {
    pub fn new(resources: &Resources) -> Self {
        Self {
            square: ModelData::new(Data::square(), resources).unwrap(),
            triangle: ModelData::new(Data::triangle(), resources).unwrap(),
        }
    }
}

//struct object with position, size, rotation.

#[allow(dead_code)]
const TRIANGLE: [Vertex; 3] = [vert(0.0, -1.0), vert(-1.0, 1.0), vert(1.0, 1.0)];
#[allow(dead_code)]
const TRIANGLE_ID: [u32; 3] = [0, 1, 2];

#[allow(dead_code)]
const SQUARE: [Vertex; 4] = [
    vert(-1.0, -1.0),
    vert(1.0, -1.0),
    vert(-1.0, 1.0),
    vert(1.0, 1.0),
];
#[allow(dead_code)]
const SQUARE_ID: [u32; 6] = [0, 1, 2, 1, 2, 3];

/// A macro that makes it easy to create circles.
///
/// Returns [Data] with vertices and indices.
///
/// Using this with a `u32` makes a circle fan with as many corners as given.
///
/// Using this with a `u32` and a `f64` makes a circle fan that looks like a pie with the given percentage missing.
///
/// ## usage:
/// ```rust
/// use let_engine::prelude::*;
///
/// let hexagon: Data = make_circle!(6); // Makes a hexagon.
///
/// let pie: Data = make_circle!(20, 0.75); // Makes a pie circle fan with 20 edges with the top right part missing a quarter piece.
/// ```
#[macro_export]
macro_rules! make_circle {
    ($corners:expr) => {{ // Make a full circle fan with variable edges.
        use let_engine::{vec2, Vertex};
        let corners = $corners;
        let mut vertices: Vec<Vertex> = vec![];
        let mut indices: Vec<u32> = vec![];
        use core::f64::consts::TAU;
        // first point in the middle
        vertices.push(Vertex {
            position: vec2(0.0, 0.0),
            tex_position: vec2(0.0, 0.0),
        });
        // Going through the number of steps and pushing the % of one complete TAU circle to the vertices.
        for i in 0..corners {
            vertices.push(vert(
                    (TAU * ((i as f64) / corners as f64)).cos() as f32,
                    (TAU * ((i as f64) / corners as f64)).sin() as f32,
                ));
        }
        // Adding the indices adding the middle point, index and index after this one.
        for i in 0..corners - 1 { // -1 so the last index doesn't go above the total amounts of indices.
            indices.extend([0, i + 1, i + 2]);
        }
        // Completing the indices by setting the last 2 indices to the last point and the first point of the circle.
        indices.extend([0, corners, 1]);
        Data { vertices, indices }
    }};
    ($corners:expr, $percent:expr) => {{ // Make a pie circle fan with the amount of edges and completeness of the circle.
        use core::f64::consts::TAU;
        use let_engine::{vec2, Vertex};
        let corners = $corners;
        let percent = $percent as f64;
        let percent: f64 = percent.clamp(0.0, 1.0);
        let mut vertices: Vec<Vertex> = vec![];
        let mut indices: Vec<u32> = vec![];

        let count = TAU * percent;

        vertices.push(vert(0.0, 0.0));
        // Do the same as last time just with +1 iterations, because the last index doesn't go back to the first circle position.
        for i in 0..corners + 1 {
            vertices.push(vert(
                    (count * ((i as f64) / corners as f64)).cos() as f32,
                    (count * ((i as f64) / corners as f64)).sin() as f32,
                ));
        }
        // This time the complete iteration is possible because the last index of the circle is not the first one as in the last.
        for i in 0..corners {
            indices.extend([0, i + 1, i + 2]);
        }

        Data { vertices, indices }
    }};
}
