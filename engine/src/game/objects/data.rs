use glam::f32::{vec2, Mat4, Vec2};
use vulkano::{buffer::BufferContents, pipeline::graphics::vertex_input::Vertex as VTX};

#[repr(C)]
#[derive(BufferContents, VTX, Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    #[format(R32G32_SFLOAT)]
    pub position: Vec2,
    #[format(R32G32_SFLOAT)]
    pub tex_position: Vec2,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, BufferContents)]
pub struct ModelViewProj {
    //sepparate to vertex and fragment
    pub model: Mat4,
    pub view: Mat4,
    pub proj: Mat4,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, BufferContents)]
pub struct ObjectFrag {
    pub color: [f32; 4],
    pub texture_id: u32,
}

impl Default for ObjectFrag {
    fn default() -> Self {
        Self {
            color: [0.0, 0.0, 0.0, 0.0],
            texture_id: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, BufferContents)]
pub struct PushConstant {
    pub resolution: [f32; 2],
}

/// The 4 Camera scaling modes determine how far you can see when the window changes scale.
/// For 2D games those are a problem because there will always be someone with a monitor or window with a weird aspect ratio that can see much more than others when it's not on stretch mode.
/// Those are the options in this game engine:
///
/// 1: Stretch - goes from -1 to 1 in both x and y. So the camera view stretches when the window is not square.
///
/// 2: Linear - Tries to be fair with window scaling and tries to have the same width\*height surface all the time. But when Making the window really thin or something like that you can still see the same height\*width so you could see really far.
///
/// 3: Circle - Imagine a rope tied to itself to make a circle and imagine trying to fit 4 corners of a rectangle as far away from each other. It's similar to Linear but you can't look that far the tighter the window is.
///
/// 4: Limited - The biggest side is always -1 to 1. Simple and more unfair the tighter your window is.
///
/// 5: Expand - The bigger the window is the more you can see. Good for HUDs, fonts and textures.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CameraScaling {
    Stretch = 1,
    Linear = 2,
    Circle = 3,
    Limited = 4,
    Expand = 5,
}

impl Default for CameraScaling {
    fn default() -> Self {
        Self::Stretch
    }
}

pub const CENTER: [f32; 2] = [0.5; 2];
pub const N: [f32; 2] = [0.5, 0.0];
pub const NO: [f32; 2] = [1.0, 0.0];
pub const O: [f32; 2] = [1.0, 0.5];
pub const SO: [f32; 2] = [1.0; 2];
pub const S: [f32; 2] = [0.5, 1.0];
pub const SW: [f32; 2] = [0.0, 1.0];
pub const W: [f32; 2] = [0.0, 0.5];
pub const NW: [f32; 2] = [0.0; 2];

/// Vertex and index data for the appearance and shape of objects.
/// Has 3 simple presets.
///
/// Empty, Square and Triangle.
#[derive(Debug, Clone, PartialEq)]
pub struct Data {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Data {
    pub fn empty() -> Self {
        Data {
            vertices: vec![],
            indices: vec![],
        }
    }
    pub fn square() -> Self {
        Data {
            vertices: SQUARE.into(),
            indices: SQUARE_ID.into(),
        }
    }
    pub fn triangle() -> Self {
        Data {
            vertices: TRIANGLE.into(),
            indices: TRIANGLE_ID.into(),
        }
    }
}

//struct object with position, size, rotation.

#[allow(dead_code)]
pub const TRIANGLE: [Vertex; 3] = [
    Vertex {
        position: vec2(0.0, -1.0),
        tex_position: vec2(0.0, -1.0),
    },
    Vertex {
        position: vec2(-1.0, 1.0),
        tex_position: vec2(-1.0, 1.0),
    },
    Vertex {
        position: vec2(1.0, 1.0),
        tex_position: vec2(1.0, 1.0),
    },
];
#[allow(dead_code)]
pub const TRIANGLE_ID: [u32; 3] = [0, 1, 2];

#[allow(dead_code)]
pub const SQUARE: [Vertex; 4] = [
    Vertex {
        // 0
        position: vec2(-1.0, -1.0),
        tex_position: vec2(-1.0, -1.0),
    },
    Vertex {
        // 1
        position: vec2(1.0, -1.0),
        tex_position: vec2(1.0, -1.0),
    },
    Vertex {
        // 2
        position: vec2(-1.0, 1.0),
        tex_position: vec2(-1.0, 1.0),
    },
    Vertex {
        // 3
        position: vec2(1.0, 1.0),
        tex_position: vec2(1.0, 1.0),
    },
];
#[allow(dead_code)]
pub const SQUARE_ID: [u32; 6] = [0, 1, 2, 1, 2, 3];

/// A macro that makes it easy to create circles.
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
            vertices.push(Vertex {
                position: vec2(
                    (TAU * ((i as f64) / corners as f64)).cos() as f32,
                    (TAU * ((i as f64) / corners as f64)).sin() as f32,
                ),
                tex_position: vec2(
                    (TAU * ((i as f64) / corners as f64)).cos() as f32,
                    (TAU * ((i as f64) / corners as f64)).sin() as f32,
                ),
            });
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

        vertices.push(Vertex {
            position: vec2(0.0, 0.0),
            tex_position: vec2(0.0, 0.0),
        });
        // Do the same as last time just with +1 iterations, because the last index doesn't go back to the first circle position.
        for i in 0..corners + 1 {
            vertices.push(Vertex {
                position: vec2(
                    (count * ((i as f64) / corners as f64)).cos() as f32,
                    (count * ((i as f64) / corners as f64)).sin() as f32,
                ),
                tex_position: vec2(
                    (count * ((i as f64) / corners as f64)).cos() as f32,
                    (count * ((i as f64) / corners as f64)).sin() as f32,
                ),
            });
        }
        // This time the complete iteration is possible because the last index of the circle is not the first one as in the last.
        for i in 0..corners {
            indices.extend([0, i + 1, i + 2]);
        }

        Data { vertices, indices }
    }};
}
