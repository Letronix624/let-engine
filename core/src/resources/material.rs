use derive_builder::Builder;

/// Represents a material that defines how an object is rendered, including its shaders, textures, and buffers.
///
/// A `Material` consists of several components that control the appearance of an object in the rendering pipeline.
/// It includes shader programs, texture data, and buffers that define the object's properties, such as colors,
/// transformations, and other GPU resources.
pub struct Material {
    /// The material's settings that define rendering configuration, such as pipeline state or other parameters.
    pub settings: MaterialSettings,

    /// The shaders that define how the material interacts with the GPU during rendering.
    pub graphics_shaders: GraphicsShaders,
}

impl Material {
    pub fn new(settings: MaterialSettings, graphics_shaders: GraphicsShaders) -> Self {
        Self {
            settings,
            graphics_shaders,
        }
    }
}

/// Represents different ways in which an object is drawn using its vertices and indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Topology {
    /// Uses every three vertices to form an independent triangle.
    TriangleList,

    /// Forms the first triangle using three vertices, then each subsequent triangle shares the previous two vertices.
    TriangleStrip,

    /// Uses pairs of two vertices to create independent line segments.
    LineList,

    /// Draws a continuous line by connecting each vertex to the next.
    ///
    /// When the material setting `primitive_restart` is true and an index buffer is used,
    /// this line will be split for every index, which has the value of `u32::MAX`.
    LineStrip,

    /// Draws a single point for every vertex.
    PointList,
}

impl Topology {
    /// Returns true if this topology is a "strip" variant (`TriangleStrip` or `LineStrip`).
    pub fn is_strip(&self) -> bool {
        matches!(self, Topology::TriangleStrip | Topology::LineStrip)
    }

    /// Returns true if this topology is a "list" variant (`TriangleList`, `LineList`, `PointList`).
    pub fn is_list(&self) -> bool {
        matches!(
            self,
            Topology::TriangleList | Topology::LineList | Topology::PointList
        )
    }
}

/// Vertex and fragment shaders of a material
/// as well as the topology and line width, if the topology is set to LineList or LineStrip.
#[derive(Builder, Clone, Debug, PartialEq)]
pub struct MaterialSettings {
    /// The usage way of the vertices and indices given in the model.
    #[builder(setter(into), default = "Topology::TriangleList")]
    pub topology: Topology,

    /// If this is true when drawing with an index buffer in a "strip" topology,
    /// using a special index with the maximum index of `u32::MAX` will tell the GPU that
    /// it's the end of a primitive.
    ///
    /// You can only use this with the "strip" topologies.
    #[builder(setter(into), default = "false")]
    pub primitive_restart: bool,

    /// The width of the line in case the topology was set to something with lines.
    #[builder(setter(into), default = "1.0")]
    pub line_width: f32,
}

impl Eq for MaterialSettings {}

impl std::hash::Hash for MaterialSettings {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.topology.hash(state);
        self.primitive_restart.hash(state);
        self.line_width.to_bits().hash(state);
    }
}

impl Default for MaterialSettings {
    fn default() -> Self {
        Self {
            topology: Topology::TriangleList,
            primitive_restart: false,
            line_width: 1.0,
        }
    }
}

/// Shader data that can be loaded by the graphics backend inside a material.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GraphicsShaders {
    pub vertex_bytes: Vec<u8>,
    pub fragment_bytes: Option<Vec<u8>>,
    pub vertex_entry_point: String,
    pub fragment_entry_point: Option<String>,
}

impl GraphicsShaders {
    pub fn new(
        vertex_bytes: Vec<u8>,
        vertex_entry_point: String,
        fragment_bytes: Vec<u8>,
        fragment_entry_point: String,
    ) -> Self {
        Self {
            vertex_bytes,
            vertex_entry_point,
            fragment_bytes: Some(fragment_bytes),
            fragment_entry_point: Some(fragment_entry_point),
        }
    }

    pub fn new_no_fragment(vertex_bytes: Vec<u8>, vertex_entry_point: String) -> Self {
        Self {
            vertex_bytes,
            vertex_entry_point,
            fragment_bytes: None,
            fragment_entry_point: None,
        }
    }
}
