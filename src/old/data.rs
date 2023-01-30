use bytemuck::{Pod, Zeroable};
use vulkano::impl_vertex;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Zeroable, Pod)]
pub struct Vertex {
    pub position: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Zeroable, Pod)]
pub struct TextVertex {
    pub position: [f32; 2],
    pub tex_position: [f32; 2],
}

#[derive(Debug, Clone)]
pub struct Data {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
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

impl_vertex!(Vertex, position);
impl_vertex!(TextVertex, position, tex_position);

#[allow(dead_code)]
pub const BACKGROUND: [Vertex; 12] = [
    Vertex {
        position: [-1.0, 0.0],
    },
    Vertex {
        position: [-1.0, 1.0],
    },
    Vertex {
        position: [1.0, 0.0],
    },
    Vertex {
        position: [-1.0, 1.0],
    },
    Vertex {
        position: [1.0, 1.0],
    },
    Vertex {
        position: [1.0, 0.0],
    },
    Vertex {
        position: [-1.0, -1.0],
    },
    Vertex {
        position: [-1.0, 0.0],
    },
    Vertex {
        position: [1.0, -1.0],
    },
    Vertex {
        position: [-1.0, 0.0],
    },
    Vertex {
        position: [1.0, 0.0],
    },
    Vertex {
        position: [1.0, -1.0],
    },
];
#[allow(dead_code)]
pub const BACKGROUND_ID: [u16; 12] = [0, 1, 2, 1, 3, 2, 4, 0, 5, 0, 2, 5];

#[allow(dead_code)]
pub const TRIANGLE: [Vertex; 3] = [
    Vertex {
        position: [0.0, -1.0],
    },
    Vertex {
        position: [-1.0, 1.0],
    },
    Vertex {
        position: [1.0, 1.0],
    },
];
#[allow(dead_code)]
pub const TRIANGLE_ID: [u16; 3] = [0, 1, 2];

#[allow(dead_code)]
pub const SQUARE: [Vertex; 6] = [
    Vertex {
        position: [-1.0, -1.0],
    },
    Vertex {
        position: [1.0, -1.0],
    },
    Vertex {
        position: [-1.0, 1.0],
    },
    Vertex {
        position: [1.0, -1.0],
    },
    Vertex {
        position: [-1.0, 1.0],
    },
    Vertex {
        position: [1.0, 1.0],
    },
];
#[allow(dead_code)]
pub const SQUARE_ID: [u16; 6] = [0, 1, 2, 1, 2, 3];

macro_rules! make_circle {
    ($corners:expr) => {{
        let corners = $corners;
        let mut vertices: Vec<Vertex> = vec![];
        use core::f64::consts::PI;
        for i in 0..corners {
            vertices.push(Vertex {
                position: [0.0, 0.0],
            });
            vertices.push(Vertex {
                position: [
                    (PI * 2.0 * ((i as f64) / corners as f64)).cos() as f32,
                    (PI * 2.0 * ((i as f64) / corners as f64)).sin() as f32,
                ],
            });
            vertices.push(Vertex {
                position: [
                    (PI * 2.0 * (((i + 1) as f64) / corners as f64)).cos() as f32,
                    (PI * 2.0 * (((i + 1) as f64) / corners as f64)).sin() as f32,
                ],
            })
        }
        let mut indices: Vec<u16> = vec![];
        for i in 1..corners {
            indices.push(0);
            indices.push(i as u16);
            indices.push(i as u16 + 1);
        }
        indices.push(0);
        indices.push(indices.last().cloned().unwrap());
        indices.push(indices[1]);

        Data { vertices, indices }
    }};
    ($corners:expr, $purrcent:expr) => {{
        use core::f64::consts::PI;
        let corners = $corners;
        let purrcent = $purrcent as f64;
        let purrcent: f64 = purrcent.clamp(0.0, 1.0);
        let mut vertices: Vec<Vertex> = vec![];
        let count = (PI * 2.0) * purrcent;
        for i in 0..corners {
            vertices.push(Vertex {
                position: [0.0, 0.0],
            });
            vertices.push(Vertex {
                position: [
                    (count * ((i as f64) / corners as f64)).cos() as f32,
                    (count * ((i as f64) / corners as f64)).sin() as f32,
                ],
            });
            vertices.push(Vertex {
                position: [
                    (count * (((i + 1) as f64) / corners as f64)).cos() as f32,
                    (count * (((i + 1) as f64) / corners as f64)).sin() as f32,
                ],
            });
        }
        let mut indices: Vec<u16> = vec![];
        for i in 1..corners {
            indices.push(0);
            indices.push(i as u16);
            indices.push(i as u16 + 1);
        }
        indices.push(0);
        indices.push(indices.last().cloned().unwrap());
        indices.push(indices[1]);

        Data { vertices, indices }
    }};
}
pub(crate) use make_circle;
