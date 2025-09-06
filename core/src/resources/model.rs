use bytemuck::AnyBitPattern;
pub use engine_macros::Vertex;
use foldhash::{HashMap, HashMapExt};

use glam::{Vec2, Vec3, Vec4, vec2};
pub use let_engine_core::{circle, model};

use super::{Format, buffer::BufferAccess};

/// Vertex and optional index data for the appearance and shape of objects.
/// Has 3 simple presets.
///
/// Empty, Square and Triangle.
///
/// The maximum size symbolizes the biggest size of vertices and if indexed, the biggest
/// size of indices the model is allowed to have.
///
/// up is -y, down is +y.
/// right is +x und left is -x.
#[derive(Debug, Clone, PartialEq)]
pub struct Model<V: Vertex> {
    vertices: Vec<V>,
    indices: Vec<u32>,
    max_vertices: usize,
    max_indices: usize,
    buffer_access: BufferAccess,
}

impl<V: Vertex> Default for Model<V> {
    fn default() -> Self {
        Self {
            vertices: vec![],
            indices: vec![],
            max_vertices: 1,
            max_indices: 0,
            buffer_access: BufferAccess::default(),
        }
    }
}

/// Constructors
impl<V: Vertex> Model<V> {
    /// Creates a new model with vertices and an empty index buffer and the maximum
    /// size set to the length of the provided vertex buffer.
    ///
    /// In case the length of vertices equals `0`, the default maximum vertex length of `1` will be
    /// used. This is not that useful or intended most of the time, so if this model is meant
    /// to be dynamically sized, use [`new_maxed`](Self::new_maxed) instead.
    pub fn new(vertices: Vec<V>, buffer_access: BufferAccess) -> Self {
        Self::new_maxed(vertices, 1, buffer_access)
    }

    pub fn with_access(buffer_access: BufferAccess) -> Self {
        Self {
            buffer_access,
            ..Default::default()
        }
    }

    /// Creates a new model with vertices, no index buffer and the given
    /// maximum vertex buffer size.
    ///
    /// The actual maximum vertices length is the maximum between `max_size` and `vertices.len()`
    ///
    /// # Panics
    ///
    /// This function panics if argument `max_vertices` is `0`.
    /// Accepted sizes are `1` or higher.
    pub fn new_maxed(vertices: Vec<V>, max_vertices: usize, buffer_access: BufferAccess) -> Self {
        if max_vertices == 0 {
            panic!("A models size can not equal 0.");
        }

        let max_vertices = vertices.len().max(max_vertices);

        Self {
            vertices,
            indices: vec![],
            max_vertices,
            max_indices: 0,
            buffer_access,
        }
    }

    /// Creates a new model with vertices and indices.
    ///
    /// If indices is empty, this object will be non indexed. If this object is meant to be indexed, where the actual
    /// index data comes in later, use [`new_indexed_maxed`](Self::new_indexed_maxed) instead.
    ///
    /// In case the length of vertices equals `0`, the default maximum vertex length of `1` will be
    /// used. This is not that useful or intended most of the time, especially when indexing,
    /// so if this model is meant to be dynamically sized, use [`new_indexed_maxed`](Self::new_indexed_maxed) instead.
    pub fn new_indexed(vertices: Vec<V>, indices: Vec<u32>, buffer_access: BufferAccess) -> Self {
        Self::new_indexed_maxed(vertices, indices, 1, 0, buffer_access)
    }

    /// Creates a new model with vertices and indices.
    ///
    /// The actual maximum vertices length is the maximum between `max_size` and `vertices.len()`
    ///
    /// If indices is empty and `max_indices` equals `0`, this object will be non indexed.
    ///
    /// # Panics
    ///
    /// This function panics if argument `max_vertices` is `0`.
    /// Accepted sizes are `1` or higher.
    pub fn new_indexed_maxed(
        vertices: Vec<V>,
        indices: Vec<u32>,
        max_vertices: usize,
        max_indices: usize,
        buffer_access: BufferAccess,
    ) -> Self {
        if max_vertices == 0 {
            panic!("A models size can not equal 0.");
        }

        let max_vertices = vertices.len().max(max_vertices);
        let max_indices = indices.len().max(max_indices);

        Self {
            vertices,
            indices,
            max_vertices,
            max_indices,
            buffer_access,
        }
    }
}

/// A macro for easy construction of [`Model`]'s.
///
/// This macro simplifies the creation of a [`Model`] instance.
///
/// # Usage
///
/// ## Default models using vertex type [`Vert`](let_engine::resources::data::Vert)
///
/// - `model!()` → Creates an empty fixed dummy [`Model`] with max vertex size 1.
/// - `model!(triangle)` → Creates a simple 2D triangle using 3 vertices and no indices.
/// - `model!(square)` → Creates a simple 2D square spanning `(-1.0, 1.0)` to `(1.0, -1.0)` using 6 vertices and no indices.
///
/// ## Creating Models with Custom Vertices
///
/// - `model!(vertices)` → Creates a model with the given vertex buffer and default [`BufferAccess`].
/// - `model!(vertices, buffer_access)` → Crates a model with the given vertex buffer and buffer access mode.
///
/// ## Creating Indexed Models
///
/// - `model!(vertices, indices)` → Creates an indexed model with the given vertex and index buffers.
/// - `model!(vertices, indices, buffer_access)` → Creates an indexed model with the given vertex and index buffers with the specified [`BufferAccess`].
///
/// ## Creating Models with a Maximum Vertex Buffer Size
///
/// - `model!(vertices, max_vertices)` → Creates a model with a given maximum vertex buffer size.
/// - `model!(vertices, max_vertices, buffer_access)` → Crates a model with a given maximum vertex buffer size and buffer access mode.
///
/// ## Creating Indexed Models with Maximum Buffer Sizes
///
/// - `model!(vertices, indices, max_vertices, max_indices)` → Creates an indexed model with maximum vertex and index buffer sizes.
/// - `model!(vertices, indices, max_vertices, max_indices, buffer_access)` → Same as above, but with specified buffer access mode.
///
/// ## Creating Empty Models with Maximum Buffer Sizes
///
/// - `model!(max_vertices: usize)` → Creates a model with an empty vertex buffer and a defined maximum vertex buffer size.
/// - `model!(max_vertices: usize, max_indices: usize)` → Creates an indexed model with empty buffers and defined maximum sizes.
/// - `model!(max_vertices: usize, buffer_access: BufferAccess)` → Creates a model with a specified buffer access mode.
/// - `model!(max_vertices: usize, max_indices: usize, buffer_access: BufferAccess)` → Creates an indexed model with empty buffers, defined sizes, and a specified buffer access mode.
///
/// # Notes
///
/// - If only `vertices` are provided, the model is **non-indexed**.
/// - If `indices` are provided, the model is **indexed**.
/// - If `max_vertices` or `max_indices` are provided, they define the buffer size constraints.
/// - If `BufferAccess` is omitted, it defaults to `Fixed`.
///
/// [`Model`]: struct@let_engine::resources::model::Model
/// [`BufferAccess`]: enum@let_engine::resources::buffer::BufferAccess
#[macro_export]
macro_rules! model {
    // default
    () => {
        let_engine::resources::model::Model::default()
    };

    // triangle
    (triangle) => {
        let_engine::resources::model::Model::triangle()
    };

    // square
    (square) => {
        let_engine::resources::model::Model::square()
    };

    // only vertices
    ($vertices:expr) => {
        let_engine::resources::model::Model::new(
            $vertices,
            let_engine::resources::buffer::BufferAccess::default(),
        )
    };

    // vertices and indices
    ($vertices:expr, $indices:expr) => {
        let_engine::resources::model::Model::new_indexed(
            $vertices,
            $indices,
            let_engine::resources::buffer::BufferAccess::default(),
        )
    };

    // vertices and max vertices
    ($vertices:expr, $max_vertices:expr) => {
        let_engine::resources::model::Model::new_maxed(
            $vertices,
            $max_vertices,
            let_engine::resources::buffer::BufferAccess::default(),
        )
    };

    // only max vertices
    ($max_vertices:expr) => {
        let_engine::resources::model::Model::new_maxed(
            Vec::new(),
            $max_vertices,
            let_engine::resources::buffer::BufferAccess::default(),
        )
    };

    // vertices, indices, max vertices and max indices
    ($vertices:expr, $indices:expr, $max_vertices:expr, $max_indices:expr) => {
        let_engine::resources::model::Model::new_indexed_maxed(
            $vertices,
            $indices,
            $max_vertices,
            $max_indices,
            let_engine::resources::buffer::BufferAccess::default(),
        )
    };

    // max vertices and max indices
    ($max_vertices:expr, $max_indices:expr) => {
        let_engine::resources::model::Model::new_indexed_maxed(
            Vec::new(),
            Vec::new(),
            $max_vertices,
            $max_indices,
            let_engine::resources::buffer::BufferAccess::default(),
        )
    };

    // vertices and buffer access
    ($vertices:expr, $buffer_access:expr) => {
        let_engine::resources::model::Model::new($vertices, $buffer_access)
    };

    // vertices, indices and buffer access
    ($vertices:expr, $indices:expr, $buffer_access:expr) => {
        let_engine::resources::model::Model::new_indexed($vertices, $indices, $buffer_access)
    };

    // vertices, max vertices and buffer access
    ($vertices:expr, $max_vertices:expr, $buffer_access:expr) => {
        let_engine::resources::model::Model::new_maxed($vertices, $max_vertices, $buffer_access)
    };

    // max vertices and buffer access
    ($max_vertices:expr, $buffer_access:expr) => {
        let_engine::resources::model::Model::new_maxed(Vec::new(), $max_vertices, $buffer_access)
    };

    // vertices, indices, max vertices, max indices and buffer access
    ($vertices:expr, $indices:expr, $max_vertices:expr, $max_indices:expr, $buffer_access:expr) => {
        let_engine::resources::model::Model::new_indexed_maxed(
            $vertices,
            $indices,
            $max_vertices,
            $max_indices,
            $buffer_access,
        )
    };

    // max vertices, max indices and buffer access
    ($max_vertices:expr, $max_indices:expr, $buffer_access:expr) => {
        let_engine::resources::model::Model::new_indexed_maxed(
            Vec::new(),
            Vec::new(),
            $max_vertices,
            $max_indices,
            $buffer_access,
        )
    };
}

/// Getting and setting
impl<V: Vertex> Model<V> {
    /// Returns a slice to the vertices of this model.
    pub fn vertices(&self) -> &[V] {
        &self.vertices
    }

    /// Returns a mutable slice of the vertices of this model.
    pub fn vertices_mut(&mut self) -> &mut [V] {
        &mut self.vertices
    }

    /// Sets the vertices of this buffer to the given buffer.
    ///
    /// This method raises the `max_vertices` bar of this model in case
    /// `vertices.len()` is bigger than `max_vertices`
    pub fn set_vertices(&mut self, vertices: Vec<V>) {
        self.max_vertices = self.max_vertices.max(vertices.len());
        self.vertices = vertices;
    }

    /// Sets the maximum amount of vertices for this buffer.
    ///
    /// `max_vertices` will be set to the max of the length of the current vertex
    /// buffer and the provided `max_vertices`.
    ///
    /// # Panics
    ///
    /// This function panics if argument `max_vertices` is `0`.
    /// Accepted sizes are `1` or higher.
    pub fn set_max_vertices(&mut self, max_vertices: usize) {
        if max_vertices == 0 {
            panic!("A models size can not equal 0.");
        }

        self.max_vertices = self.max_vertices.max(max_vertices);
    }

    /// Sets the indices of this buffer to the given buffer.
    ///
    /// This method raises the `max_indices` bar of this model in case
    /// `indices.len()` is bigger than `max_indices`
    pub fn set_indices(&mut self, indices: Vec<u32>) {
        self.max_indices = indices.len().max(self.max_indices);
        self.indices = indices;
    }

    /// Sets the maximum amount of indices for this buffer.
    ///
    /// `max_indices` will be set to the max of the length of the current index
    /// buffer and the provided `max_indices`.
    pub fn set_max_indices(&mut self, max_indices: usize) {
        self.max_indices = self.max_indices.max(max_indices);
    }

    /// Returns a slice of the indices of this model.
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// Returns a mutable slice of the indices of this model.
    pub fn indices_mut(&mut self) -> &mut [u32] {
        &mut self.indices
    }

    /// Returns if the data of the vertices is empty.
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    /// Checks whether the model is indexed.
    pub fn is_indexed(&self) -> bool {
        self.max_indices != 0
    }

    /// Returns the number of elements present in the vertex buffer.
    pub fn vertex_len(&self) -> usize {
        self.vertices.len()
    }

    /// Returns the number of elements present in the index buffer.
    ///
    /// Returns 0 when this model is not indexed.
    pub fn index_len(&self) -> usize {
        self.indices.len()
    }

    /// Returns the maximum amount of vertices allowed in this model.
    pub fn max_vertices(&self) -> usize {
        self.max_vertices
    }

    /// Returns the maximum amount of indices allowed in this model.
    pub fn max_indices(&self) -> usize {
        self.max_indices
    }

    /// Returns the buffer access method used for this model.
    pub fn buffer_access(&self) -> &BufferAccess {
        &self.buffer_access
    }
}

pub trait LoadedModel<V: Vertex>: Send + Sync {
    type Error: std::error::Error + Send + Sync;

    fn read_vertices<R: FnOnce(&[V])>(&self, f: R) -> Result<(), Self::Error>;
    fn read_indices<R: FnOnce(&[u32])>(&self, f: R) -> Result<(), Self::Error>;

    fn write_vertices<W: FnOnce(&mut [V])>(&self, f: W, new_size: usize)
    -> Result<(), Self::Error>;
    fn write_indices<W: FnOnce(&mut [u32])>(
        &self,
        f: W,
        new_size: usize,
    ) -> Result<(), Self::Error>;

    fn vertex_count(&self) -> usize;
    fn max_vertices(&self) -> usize;
    fn index_count(&self) -> usize;
    fn max_indices(&self) -> usize;
}

impl<V: Vertex> LoadedModel<V> for () {
    type Error = std::io::Error;

    fn read_vertices<R>(&self, _f: R) -> Result<(), Self::Error> {
        Ok(())
    }

    fn read_indices<R>(&self, _f: R) -> Result<(), Self::Error> {
        Ok(())
    }

    fn write_vertices<W>(&self, _f: W, _new_vertex_size: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    fn write_indices<W>(&self, _f: W, _new_index_size: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    fn vertex_count(&self) -> usize {
        0
    }

    fn max_vertices(&self) -> usize {
        0
    }

    fn index_count(&self) -> usize {
        0
    }

    fn max_indices(&self) -> usize {
        0
    }
}

/// # Safety
/// This trait should not be implemented by the user, but always using the derive macro of `Vertex`.
pub unsafe trait Vertex: AnyBitPattern + Sized + Send + Sync {
    fn description() -> VertexBufferDescription;
}

use paste::paste;
macro_rules! impl_vertex {
    ($ty:ty, $format:expr) => {
        unsafe impl Vertex for $ty {
            fn description() -> VertexBufferDescription {
                let mut members = HashMap::with_capacity(1);

                paste! {
                    let stride = std::mem::size_of::<[<$ty>]>() as u32;

                    members.insert(
                        "position".to_string(),
                        VertexMemberInfo {
                            offset: 0,
                            format: Format::$format,
                            num_elements: 1,
                            stride,
                        },
                    );
                }

                VertexBufferDescription { members, stride }
            }
        }
    };
}

use glam::{I8Vec4, U8Vec4};
impl_vertex!(u8, R8Uint);
impl_vertex!(i8, R8Sint);
impl_vertex!(U8Vec4, Rgba8Uint);
impl_vertex!(I8Vec4, Rgba8Sint);

use half::f16;
impl_vertex!(f16, R16Float);

impl_vertex!(f32, R32Float);
impl_vertex!(Vec2, Rg32Float);
impl_vertex!(Vec3, Rgb32Float);
impl_vertex!(Vec4, Rgba32Float);

/// Describes the contents of a VertexBuffer.
#[derive(Clone, Debug)]
pub struct VertexBufferDescription {
    /// List of member names with their detailed information.
    pub members: HashMap<String, VertexMemberInfo>,
    /// Stride of the vertex type in a buffer.
    pub stride: u32,
}

/// Information about a member of a vertex struct.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VertexMemberInfo {
    /// The offset of the member in bytes from the start of the struct.
    pub offset: u32,

    /// The attribute format of the member. Implicitly provides the number of components.
    pub format: Format,

    /// The number of consecutive array elements or matrix columns using `format`.
    /// The corresponding number of locations might differ depending on the size of the format.
    pub num_elements: u32,

    /// If `num_elements` is greater than 1, the stride in bytes between the start of consecutive
    /// elements.
    pub stride: u32,
}

impl VertexMemberInfo {
    #[inline]
    pub fn num_components(&self) -> u32 {
        self.format
            .components()
            .iter()
            .filter(|&bits| *bits > 0)
            .count() as u32
    }
}

impl Model<Vec2> {
    pub fn triangle() -> Self {
        Self::new(TRIANGLE_2D.to_vec(), BufferAccess::Fixed)
    }
    pub fn square() -> Self {
        Self::new(SQUARE_2D.to_vec(), BufferAccess::Fixed)
    }
}

const TRIANGLE_2D: [Vec2; 3] = [vec2(0.0, -1.1547), vec2(-1.0, 0.5774), vec2(1.0, 0.5774)];

const SQUARE_2D: [Vec2; 6] = [
    vec2(1.0, 1.0),
    vec2(1.0, -1.0),
    vec2(-1.0, 1.0),
    vec2(-1.0, 1.0),
    vec2(1.0, -1.0),
    vec2(-1.0, -1.0),
];

/// A macro that makes it easy to create circles.
///
/// Returns a `Model<Vec2>` with vertices and indices.
///
/// ### $corners
/// Using this with a `u32` makes a circle fan with as many corners as given.
///
/// ### $percent
/// Using this with a `f64` makes a circle fan that looks like a pie with the given percentage missing.
///
/// ### $access
/// Ending with a [`BufferAccess`] in this macro results in this access method being used.
///
/// ## Usage
/// ```rust
/// use let_engine::prelude::*;
///
/// let hexagon: Model<Vec2> = circle!(6); // Makes a hexagon.
///
/// // Makes a pie circle fan with 20 edges with the top right part missing a quarter piece.
/// let pie: Model<Vec2> = circle!(20, 0.75);
/// ```
///
/// ## Tip
/// The amount of needed corners is similar to a logarithmical function.
/// The more corners you use, the more resources you waste, and the less you notice any change.
/// Try to use as little corners as possible for maximum resource efficiency.
#[macro_export]
macro_rules! circle {
    (0) => {
        compile_error!("Number of corners must be greater than zero")
    };

    // Default access
    ($corners:expr) => {
        let_engine::prelude::circle!($corners, let_engine::prelude::BufferAccess::Fixed)
    };
    // Full circle fan
    ($corners:expr, $access:expr) => {{
        use let_engine::{glam::vec2, resources::model::Model};

        let corners: u32 = $corners;

        let mut vertices: Vec<Vec2> = vec![];
        let mut indices: Vec<u32> = vec![];
        use core::f64::consts::TAU;

        // first point in the middle
        vertices.push(vec2(0.0, 0.0));

        // Generate vertices
        for i in 0..corners {
            let angle = TAU * ((i as f64) / corners as f64);
            vertices.push(vec2(angle.cos() as f32, angle.sin() as f32));
        }

        // Generate indices
        for i in 0..corners - 1 {
            // -1 so the last index doesn't go above the total amounts of indices.
            indices.extend([0, i + 1, i + 2]);
        }
        indices.extend([0, corners, 1]);

        Model::new_indexed(vertices, indices, $access)
    }};

    // Default access pie
    ($corners:expr, $percent:expr) => {
        let_engine::prelude::circle!($corners, $percent, let_engine::prelude::BufferAccess::Fixed)
    };
    // Pie circle
    ($corners:expr, $percent:expr, $access:expr) => {{
        use let_engine::{
            glam::{Vec2, vec2},
            resources::model::Model,
        };

        let corners: u32 = $corners;
        if corners == 0 {
            panic!("Number of corners must be greater than zero")
        }

        let percent: f64 = ($percent as f64).clamp(0.0, 1.0);
        let mut vertices: Vec<Vec2> = vec![];
        let mut indices: Vec<u32> = vec![];
        use core::f64::consts::TAU;

        let angle_limit = TAU * percent;

        vertices.push(vec2(0.0, 0.0));

        // Generate vertices
        for i in 0..corners + 1 {
            let angle = angle_limit * (i as f64 / corners as f64);
            vertices.push(vec2(angle.cos() as f32, angle.sin() as f32));
        }

        // Generate indices
        for i in 0..corners {
            indices.extend([0, i + 1, i + 2]);
        }

        Model::new_indexed(vertices, indices, $access)
    }};
}
