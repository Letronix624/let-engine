use anyhow::{Context, Error, Result};
use std::sync::Arc;
use vulkano::device::Device;
use vulkano::pipeline::graphics::{
    color_blend::{AttachmentBlend, ColorBlendAttachmentState, ColorBlendState},
    multisample::MultisampleState,
    rasterization::{PolygonMode, RasterizationState},
    vertex_input::VertexDefinition,
    GraphicsPipelineCreateInfo,
    {input_assembly::InputAssemblyState, viewport::ViewportState},
};

use vulkano::pipeline::{
    layout::PipelineDescriptorSetLayoutCreateInfo, DynamicState, GraphicsPipeline, PipelineLayout,
    PipelineShaderStageCreateInfo,
};
use vulkano::render_pass::Subpass;
use vulkano::shader::ShaderModule;

/// Creates the graphics pipeline.
pub fn create_pipeline(
    device: &Arc<Device>,
    vs: &Arc<ShaderModule>,
    fs: &Arc<ShaderModule>,
    subpass: Subpass,
    vertex_buffer_description: impl VertexDefinition,
) -> Result<Arc<GraphicsPipeline>> {
    let vertex = vs.entry_point("main").ok_or(Error::msg(
        "There is no `main` entry point for the default shaders.",
    ))?;
    let fragment = fs.entry_point("main").ok_or(Error::msg(
        "There is no `main` entry point for the default shaders.",
    ))?;
    let input_assembly = InputAssemblyState::default();
    let stages = [
        PipelineShaderStageCreateInfo::new(vertex.clone()),
        PipelineShaderStageCreateInfo::new(fragment.clone()),
    ];
    let layout = PipelineLayout::new(
        device.clone(),
        PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
            .into_pipeline_layout_create_info(device.clone())?,
    )?;

    let vertex_input_state =
        vertex_buffer_description.definition(&vertex.info().input_interface)?;
    GraphicsPipeline::new(
        device.clone(),
        None,
        GraphicsPipelineCreateInfo {
            stages: stages.into_iter().collect(),
            vertex_input_state: Some(vertex_input_state),
            input_assembly_state: Some(input_assembly),
            viewport_state: Some(ViewportState::default()),
            rasterization_state: Some(RasterizationState {
                polygon_mode: PolygonMode::Fill,
                // line_stipple,
                ..RasterizationState::default()
            }),
            multisample_state: Some(MultisampleState::default()),
            color_blend_state: Some(ColorBlendState::with_attachment_states(
                subpass.num_color_attachments(),
                ColorBlendAttachmentState {
                    blend: Some(AttachmentBlend::alpha()),
                    ..Default::default()
                },
            )),
            dynamic_state: [DynamicState::Viewport].into_iter().collect(),
            subpass: Some(subpass.into()),
            ..GraphicsPipelineCreateInfo::layout(layout)
        },
    )
    .context("Could not create a graphics pipeline.")
}
