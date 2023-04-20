use crate::game::objects::data::Vertex as GameVertex;
use std::sync::Arc;
use vulkano::device::Device;
use vulkano::pipeline::graphics::color_blend::ColorBlendState;
use vulkano::pipeline::graphics::rasterization::{PolygonMode, RasterizationState};
use vulkano::pipeline::graphics::{
    input_assembly::InputAssemblyState, vertex_input::Vertex, viewport::ViewportState,
};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::render_pass::Subpass;
use vulkano::shader::ShaderModule;

pub fn create_pipeline(
    device: &Arc<Device>,
    vs: &Arc<ShaderModule>,
    fs: &Arc<ShaderModule>,
    subpass: Subpass,
) -> Arc<GraphicsPipeline> {
    GraphicsPipeline::start()
        .vertex_input_state(GameVertex::per_vertex())
        .input_assembly_state(InputAssemblyState::new())
        .vertex_shader(vs.entry_point("main").unwrap(), ())
        .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
        .fragment_shader(fs.entry_point("main").unwrap(), ())
        .color_blend_state(ColorBlendState::new(subpass.num_color_attachments()).blend_alpha())
        .rasterization_state(RasterizationState::new().polygon_mode(PolygonMode::Fill))
        .render_pass(subpass)
        .build(device.clone())
        .unwrap()
}
