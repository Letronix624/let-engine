extern crate image;
extern crate vulkano;
use crate::data::*;
use std::sync::Arc;
use vulkano::device::Device;
use vulkano::pipeline::graphics::color_blend::ColorBlendState;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::pipeline::graphics::{
    input_assembly::InputAssemblyState, vertex_input::BuffersDefinition, viewport::ViewportState,
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
        .vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
        .input_assembly_state(InputAssemblyState::new())
        .vertex_shader(vs.entry_point("main").unwrap(), ())
        .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
        .fragment_shader(fs.entry_point("main").unwrap(), ())
        .color_blend_state(ColorBlendState::new(subpass.num_color_attachments()).blend_alpha())
        .render_pass(subpass)
        .build(device.clone())
        .unwrap()
}

pub fn create_font_pipeline(
    device: &Arc<Device>,
    vs: &Arc<ShaderModule>,
    fs: &Arc<ShaderModule>,
    subpass: Subpass,
    dimension: [f32; 2],
) -> Arc<GraphicsPipeline> {
    GraphicsPipeline::start()
        .vertex_input_state(BuffersDefinition::new().vertex::<TextVertex>())
        .vertex_shader(vs.entry_point("main").unwrap(), ())
        .input_assembly_state(InputAssemblyState::new())
        .viewport_state(ViewportState::viewport_fixed_scissor_irrelevant([
            Viewport {
                origin: [0.0, 0.0],
                depth_range: 0.0..1.0,
                dimensions: [dimension[0] as f32, dimension[1] as f32],
            },
        ]))
        .fragment_shader(fs.entry_point("main").unwrap(), ())
        .color_blend_state(ColorBlendState::new(subpass.num_color_attachments()).blend_alpha())
        .render_pass(subpass)
        .build(device.clone())
        .unwrap()
}
