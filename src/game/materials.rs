use super::{objects::data::Vertex as GameVertex, Vulkan};
use derive_builder::Builder;
use std::sync::Arc;
use vulkano::pipeline::{
    graphics::{
        color_blend::ColorBlendState,
        input_assembly::{InputAssemblyState, PrimitiveTopology},
        rasterization::RasterizationState,
        vertex_input::Vertex,
        viewport::ViewportState,
    },
    GraphicsPipeline, StateMode,
};
use vulkano::render_pass::Subpass;
use vulkano::shader::ShaderModule;

#[derive(Clone, Copy)]
pub enum Topology {
    TriangleList,
    TriangleStrip,
    LineList,
    LineStrip,
    PointList,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub shaders: Shaders,
    pipeline: Arc<GraphicsPipeline>,
}

impl Material {
    pub fn new(settings: MaterialSettings, vulkan: &Vulkan, subpass: Subpass) -> Self {
        let vs = &settings.shaders.vertex;
        let fs = &settings.shaders.fragment;

        let topology: PrimitiveTopology = match settings.topology {
            Topology::TriangleList => PrimitiveTopology::TriangleList,
            Topology::TriangleStrip => PrimitiveTopology::TriangleStrip,
            Topology::LineList => PrimitiveTopology::LineList,
            Topology::LineStrip => PrimitiveTopology::LineStrip,
            Topology::PointList => PrimitiveTopology::PointList,
        };

        let pipeline: Arc<GraphicsPipeline> = GraphicsPipeline::start()
            .vertex_input_state(GameVertex::per_vertex())
            .input_assembly_state(InputAssemblyState::new().topology(topology))
            .rasterization_state(RasterizationState {
                line_width: StateMode::Fixed(settings.line_width),
                ..RasterizationState::new()
            })
            .vertex_shader(vs.entry_point("main").unwrap(), ())
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .fragment_shader(fs.entry_point("main").unwrap(), ())
            .color_blend_state(ColorBlendState::new(subpass.num_color_attachments()).blend_alpha())
            .render_pass(subpass)
            .build(vulkan.device.clone())
            .unwrap();
        Self {
            shaders: settings.shaders,
            pipeline,
        }
    }
    pub fn pipeline(&self) -> &Arc<GraphicsPipeline> {
        &self.pipeline
    }
}

/// Vertex and fragment shaders of a material
/// as well as the topology and line width, if the topology is set to LineList or LineStrip.
#[derive(Builder)]
pub struct MaterialSettings {
    #[builder(setter(into))]
    pub shaders: Shaders,
    #[builder(setter(into))]
    pub topology: Topology,
    #[builder(setter(into))]
    pub line_width: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shaders {
    pub vertex: Arc<ShaderModule>,
    pub fragment: Arc<ShaderModule>,
}

impl Shaders {
    pub unsafe fn from_bytes(vertex_bytes: &[u8], fragment_bytes: &[u8], vulkan: &Vulkan) -> Self {
        let vertex: Arc<ShaderModule> =
            unsafe { ShaderModule::from_bytes(vulkan.device.clone(), vertex_bytes).unwrap() };
        let fragment: Arc<ShaderModule> =
            unsafe { ShaderModule::from_bytes(vulkan.device.clone(), fragment_bytes).unwrap() };
        Self { vertex, fragment }
    }
}

impl Default for MaterialSettings {
    fn default() -> Self {
        Self {
            shaders: Shaders::default(),
            topology: Topology::TriangleList,
            line_width: 1.,
        }
    }
}

impl Default for Shaders {
    fn default() -> Self {
        todo!();
    }
}
