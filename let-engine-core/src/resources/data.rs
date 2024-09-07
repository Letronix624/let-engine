//! Holds model related data structures like Vertices and premade models as well as a circle maker macro.

use anyhow::Result;
use glam::f32::{vec2, Mat4, Vec2, Vec4};
use thiserror::Error;
use vulkano::{buffer::BufferContents, pipeline::graphics::vertex_input::Vertex as VTX};

use super::{Loader, ModelData};
use parking_lot::Mutex;
use std::sync::Arc;

/// The model you are trying to load has empty data.
///
/// Use `apperance.set_visible(false)` instead.
#[derive(Debug, Error)]
#[error("The model you are trying to load has empty data.")]
pub struct NoDataError;

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
pub enum Data {
    /// Never changing model data
    Fixed {
        vertices: &'static [Vertex],
        indices: &'static [u32],
    },
    /// Model data that may change
    Dynamic {
        vertices: Vec<Vertex>,
        indices: Vec<u32>,
    },
}

impl Data {
    /// Creates new dynamically resizable modeldata with the given vertices and indices.
    pub const fn new_dynamic(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Self::Dynamic { vertices, indices }
    }

    /// Creates new fixed sized modeldata with the given vertices and indices.
    pub const fn new_fixed(vertices: &'static [Vertex], indices: &'static [u32]) -> Self {
        Self::Fixed { vertices, indices }
    }

    /// Returns the data of a square that goes from `-1.0` to `1.0` in both X and Y.
    pub fn square() -> Self {
        Data::Fixed {
            vertices: &SQUARE,
            indices: &SQUARE_ID,
        }
    }

    /// Returns the data of a triangle with the points `[0.0, -1.0], [-1.0, 1.0], [1.0, 1.0]`.
    pub fn triangle() -> Self {
        Data::Fixed {
            vertices: &TRIANGLE,
            indices: &TRIANGLE_ID,
        }
    }

    /// Returns if the data has an empty field.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Fixed { vertices, indices } => vertices.is_empty() || indices.is_empty(),
            Self::Dynamic { vertices, indices } => vertices.is_empty() || indices.is_empty(),
        }
    }

    /// Returns a slice of bytes of the vertices of this data.
    pub fn vertices(&self) -> &[Vertex] {
        match self {
            Self::Fixed { vertices, .. } => vertices,
            Self::Dynamic { vertices, .. } => vertices,
        }
    }

    /// Returns a slice of bytes of the indices of this data.
    pub fn indices(&self) -> &[u32] {
        match self {
            Self::Fixed { indices, .. } => indices,
            Self::Dynamic { indices, .. } => indices,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BasicShapes {
    pub square: ModelData,
    pub triangle: ModelData,
}

impl BasicShapes {
    pub fn new(loader: &Arc<Mutex<Loader>>) -> Result<Self> {
        Ok(Self {
            square: ModelData::new_from_loader(Data::square(), loader)?,
            triangle: ModelData::new_from_loader(Data::triangle(), loader)?,
        })
    }
}

//struct object with position, size, rotation.

const TRIANGLE: [Vertex; 3] = [vert(0.0, -1.0), vert(-1.0, 1.0), vert(1.0, 1.0)];
const TRIANGLE_ID: [u32; 3] = [0, 1, 2];

const SQUARE: [Vertex; 4] = [
    vert(-1.0, -1.0),
    vert(1.0, -1.0),
    vert(-1.0, 1.0),
    vert(1.0, 1.0),
];
const SQUARE_ID: [u32; 6] = [0, 1, 2, 1, 2, 3];
