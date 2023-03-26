use bytemuck::{Pod, Zeroable};
use vulkano::impl_vertex;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Zeroable, Pod, PartialEq)]
pub struct Vertex {
    pub position: [f32; 2],
    pub tex_position: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Zeroable, Pod, PartialEq)]
pub struct Camera {
    pub position: [f32; 2],
    pub rotation: f32,
    pub zoom: f32,
    pub mode: u32,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            position: [0.0; 2],
            rotation: 0.0,
            zoom: 1.0,
            mode: 0,
        }
    }
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

impl_vertex!(Vertex, position, tex_position);

#[allow(dead_code)]
pub const TRIANGLE: [Vertex; 3] = [
    Vertex {
        position: [0.0, -1.0],
        tex_position: [0.0, -1.0],
    },
    Vertex {
        position: [-1.0, 1.0],
        tex_position: [-1.0, 1.0],
    },
    Vertex {
        position: [1.0, 1.0],
        tex_position: [1.0, 1.0],
    },
];
#[allow(dead_code)]
pub const TRIANGLE_ID: [u32; 3] = [0, 1, 2];

#[allow(dead_code)]
pub const SQUARE: [Vertex; 4] = [
    Vertex { // 0
        position: [-1.0, -1.0],
        tex_position: [-1.0, -1.0],
    },
    Vertex { // 1
        position: [1.0, -1.0],
        tex_position: [1.0, -1.0],
    },
    Vertex { // 2
        position: [-1.0, 1.0],
        tex_position: [-1.0, 1.0],
    },
    Vertex { // 3
        position: [1.0, 1.0],
        tex_position: [1.0, 1.0],
    },
];
#[allow(dead_code)]
pub const SQUARE_ID: [u32; 6] = [0, 1, 2, 1, 2, 3];

/// A macro that makes it easy to create circles.
#[allow(unused)]
#[macro_export]
macro_rules! make_circle {
    ($corners:expr) => {{
        use let_engine::Vertex;
        let corners = $corners;
        let mut vertices: Vec<Vertex> = vec![];
        let mut indices: Vec<u32> = vec![];
        use core::f64::consts::PI;
        vertices.push(Vertex {
            position: [0.0, 0.0],
            tex_position: [0.0, 0.0],
        });
        for i in 0..corners {
            indices.extend([0, i+1, i+2]);
            vertices.push(Vertex {
                position: [
                    (PI * 2.0 * ((i as f64) / corners as f64)).cos() as f32,
                    (PI * 2.0 * ((i as f64) / corners as f64)).sin() as f32,
                ],
                tex_position: [
                    (PI * 2.0 * ((i as f64) / corners as f64)).cos() as f32,
                    (PI * 2.0 * ((i as f64) / corners as f64)).sin() as f32,
                ],
            });
        }

        indices.extend([0, corners, 1]);
        Data { vertices, indices }
    }};
    ($corners:expr, $purrcent:expr) => {{
        use core::f64::consts::PI;
        use let_engine::Vertex;
        let corners = $corners;
        let purrcent = $purrcent as f64;
        let purrcent: f64 = purrcent.clamp(0.0, 1.0);
        let mut vertices: Vec<Vertex> = vec![];
        let mut indices: Vec<u32> = vec![];

        let count = (PI * 2.0) * purrcent;

        vertices.push(Vertex {
            position: [0.0, 0.0],
            tex_position: [0.0, 0.0],
        });

        for i in 0..corners + 1 {
            indices.extend([0, i+1, i+2]);
            vertices.push(Vertex {
                position: [
                    (count * ((i as f64) / corners as f64)).cos() as f32,
                    (count * ((i as f64) / corners as f64)).sin() as f32,
                ],
                tex_position: [
                    (count * ((i as f64) / corners as f64)).cos() as f32,
                    (count * ((i as f64) / corners as f64)).sin() as f32,
                ],
            });
        }

        Data { vertices, indices }
    }};
}

#[allow(unused)]
pub(crate) use make_circle;
