use anyhow::{Context, Result};
use std::sync::Arc;
use vulkano::device::Device;
use vulkano::pipeline::cache::PipelineCache;
use vulkano::pipeline::graphics::vertex_input::VertexInputState;
use vulkano::pipeline::graphics::{
    color_blend::{AttachmentBlend, ColorBlendAttachmentState, ColorBlendState},
    multisample::MultisampleState,
    rasterization::RasterizationState,
    GraphicsPipelineCreateInfo,
    {input_assembly::InputAssemblyState, viewport::ViewportState},
};

use vulkano::pipeline::{
    layout::PipelineDescriptorSetLayoutCreateInfo, GraphicsPipeline, PipelineLayout,
    PipelineShaderStageCreateInfo,
};
use vulkano::render_pass::Subpass;
use vulkano::shader::EntryPoint;

use crate::draw::VIEWPORT;

/// Creates the graphics pipeline.
#[allow(clippy::too_many_arguments)]
pub fn create_pipeline(
    device: &Arc<Device>,
    vertex: EntryPoint,
    fragment: EntryPoint,
    input_assembly: InputAssemblyState,
    subpass: Subpass,
    vertex_input_state: VertexInputState,
    rasterisaion_state: RasterizationState,
    cache: Option<Arc<PipelineCache>>,
) -> Result<Arc<GraphicsPipeline>> {
    let stages = [
        PipelineShaderStageCreateInfo::new(vertex.clone()),
        PipelineShaderStageCreateInfo::new(fragment.clone()),
    ];
    let layout = PipelineLayout::new(
        device.clone(),
        PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
            .into_pipeline_layout_create_info(device.clone())?,
    )?;

    GraphicsPipeline::new(
        device.clone(),
        cache,
        GraphicsPipelineCreateInfo {
            stages: stages.into_iter().collect(),
            vertex_input_state: Some(vertex_input_state),
            input_assembly_state: Some(input_assembly),
            viewport_state: Some(ViewportState {
                viewports: [VIEWPORT.read().clone()].into_iter().collect(),
                ..Default::default()
            }),
            rasterization_state: Some(rasterisaion_state),
            multisample_state: Some(MultisampleState::default()),
            color_blend_state: Some(ColorBlendState::with_attachment_states(
                subpass.num_color_attachments(),
                ColorBlendAttachmentState {
                    blend: Some(AttachmentBlend::alpha()),
                    ..Default::default()
                },
            )),
            subpass: Some(subpass.into()),
            ..GraphicsPipelineCreateInfo::layout(layout)
        },
    )
    .context("Could not create a graphics pipeline.")
}
