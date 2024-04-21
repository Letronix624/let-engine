mod instance;
pub mod pipeline;
pub mod shaders;
pub use shaders::*;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use winit::event_loop::EventLoop;
#[cfg(feature = "vulkan_debug_utils")]
mod debug;
pub mod swapchain;
pub(crate) mod window;

use crate::draw::VIEWPORT;
use crate::resources::data::Vertex as GameVertex;
use anyhow::{Context, Error, Result};
use vulkano::{
    device::{Device, DeviceFeatures, Queue},
    image::{view::ImageView, Image},
    pipeline::{
        graphics::{
            input_assembly::InputAssemblyState,
            vertex_input::{Vertex, VertexDefinition},
            viewport::Viewport,
        },
        GraphicsPipeline,
    },
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
};

use std::sync::Arc;

use super::data::InstanceData;
use super::materials::{Material, Shaders};

/// Just a holder of general immutable information about Vulkan.
#[derive(Clone)]
pub struct Vulkan {
    pub instance: Arc<vulkano::instance::Instance>,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub render_pass: Arc<RenderPass>,
    pub subpass: Subpass,

    pub default_shaders: Shaders,
    pub default_instance_shaders: Shaders,
    pub default_material: Material,
    pub textured_material: Material,
    pub texture_array_material: Material,
    pub default_instance_material: Material,
    pub textured_instance_material: Material,
    pub texture_array_instance_material: Material,
}

impl Vulkan {
    pub fn init(event_loop: &EventLoop<()>) -> Result<(Vec<Arc<GraphicsPipeline>>, Self)> {
        let instance = instance::create_instance(event_loop)?;

        #[cfg(feature = "vulkan_debug_utils")]
        std::mem::forget(debug::make_debug(&instance)?);

        let (surface, window) =
            window::create_window(event_loop, &instance, crate::window::WindowBuilder::new())?;

        VIEWPORT.write().extent = window.inner_size().into();

        let device_extensions = instance::create_device_extensions();
        let features = DeviceFeatures {
            fill_mode_non_solid: true,
            wide_lines: true,
            ..DeviceFeatures::empty()
        };
        let (physical_device, queue_family_index) =
            instance::create_physical_device(&instance, device_extensions, features, &surface)?;
        let (device, queue) = instance::create_device_and_queues(
            &physical_device,
            &device_extensions,
            features,
            queue_family_index,
        )?;

        let render_pass = vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    format: device.physical_device().surface_formats(&surface, Default::default())?[0].0,
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {}
            }
        )?;

        let subpass = Subpass::from(render_pass.clone(), 0).ok_or(Error::msg(
            "There was a problem making a subpass from the last render pass.",
        ))?;

        //Materials
        let vs = vertex_shader(device.clone())?;
        let fs = fragment_shader(device.clone())?;
        let default_shaders = Shaders::from_modules(vs.clone(), fs.clone(), "main");

        let tfs = textured_fragment_shader(device.clone())?;
        let default_textured_shaders = Shaders::from_modules(vs.clone(), tfs.clone(), "main");

        let tafs = texture_array_fragment_shader(device.clone())?;
        let default_texture_array_shaders = Shaders::from_modules(vs.clone(), tafs.clone(), "main");

        let instance_vert = instanced_vertex_shader(device.clone())?;
        let instance_frag = instanced_fragment_shader(device.clone())?;
        let default_instance_shaders =
            Shaders::from_modules(instance_vert.clone(), instance_frag.clone(), "main");

        let textured_instance_frag = instanced_textured_fragment_shader(device.clone())?;
        let default_textured_instance_shaders = Shaders::from_modules(
            instance_vert.clone(),
            textured_instance_frag.clone(),
            "main",
        );

        let texture_array_instance_frag = instanced_texture_array_fragment_shader(device.clone())?;
        let default_texture_array_instance_shaders = Shaders::from_modules(
            instance_vert.clone(),
            texture_array_instance_frag.clone(),
            "main",
        );

        let vertex_buffer_description = [GameVertex::per_vertex(), InstanceData::per_instance()];

        let mut pipelines = vec![];

        let rasterisation_state = RasterizationState::default();

        let vertex = vs
            .entry_point("main")
            .expect("Main function of default vertex shader has no main function.");
        let fragment = fs
            .entry_point("main")
            .expect("Main function of default fragment shader has no main function.");

        let pipeline: Arc<GraphicsPipeline> = pipeline::create_pipeline(
            &device,
            vertex.clone(),
            fragment,
            InputAssemblyState::default(),
            subpass.clone(),
            vertex_buffer_description[0].definition(&vertex)?,
            rasterisation_state.clone(),
            None,
        )?;
        pipelines.push(pipeline.clone());

        let textured_fragment = tfs
            .entry_point("main")
            .expect("Main function not found in default textured fragment shader.");
        let textured_pipeline = pipeline::create_pipeline(
            &device,
            vertex.clone(),
            textured_fragment,
            InputAssemblyState::default(),
            subpass.clone(),
            vertex_buffer_description[0].definition(&vertex)?,
            rasterisation_state.clone(),
            None,
        )?;
        pipelines.push(textured_pipeline.clone());

        let texture_array_fragment = tafs
            .entry_point("main")
            .expect("Main function not found in default texture array shader.");
        let texture_array_pipeline = pipeline::create_pipeline(
            &device,
            vertex.clone(),
            texture_array_fragment,
            InputAssemblyState::default(),
            subpass.clone(),
            vertex_buffer_description[0].definition(&vertex)?,
            rasterisation_state.clone(),
            None,
        )?;
        pipelines.push(texture_array_pipeline.clone());

        let instance_vertex = instance_vert
            .entry_point("main")
            .expect("Main function not found in default instanced vertex shader.");
        let instance_fragment = instance_frag
            .entry_point("main")
            .expect("Main function not found in default instanced fragment shader.");
        let instance_pipeline = pipeline::create_pipeline(
            &device,
            instance_vertex.clone(),
            instance_fragment,
            InputAssemblyState::default(),
            subpass.clone(),
            vertex_buffer_description.definition(&instance_vertex)?,
            rasterisation_state.clone(),
            None,
        )?;
        pipelines.push(instance_pipeline.clone());

        let textured_instance_fragment = textured_instance_frag
            .entry_point("main")
            .expect("Main function not found in default textured instanced fragment shader.");
        let textured_instance_pipeline = pipeline::create_pipeline(
            &device,
            instance_vertex.clone(),
            textured_instance_fragment,
            InputAssemblyState::default(),
            subpass.clone(),
            vertex_buffer_description.definition(&instance_vertex)?,
            rasterisation_state.clone(),
            None,
        )?;
        pipelines.push(textured_instance_pipeline.clone());

        let texture_array_instance_fragment = texture_array_instance_frag
            .entry_point("main")
            .expect("Main function not found in default texture array instance fragment shader.");
        let texture_array_instance_pipeline = pipeline::create_pipeline(
            &device,
            instance_vertex.clone(),
            texture_array_instance_fragment,
            InputAssemblyState::default(),
            subpass.clone(),
            vertex_buffer_description.definition(&instance_vertex)?,
            rasterisation_state,
            None,
        )?;
        pipelines.push(texture_array_instance_pipeline.clone());

        let default_material = Material::from_pipeline(&pipeline, false, default_shaders.clone());
        let textured_material =
            Material::from_pipeline(&textured_pipeline, false, default_textured_shaders.clone());
        let texture_array_material = Material::from_pipeline(
            &texture_array_pipeline,
            false,
            default_texture_array_shaders.clone(),
        );
        let default_instance_material =
            Material::from_pipeline(&instance_pipeline, true, default_instance_shaders.clone());

        let textured_instance_material = Material::from_pipeline(
            &textured_instance_pipeline,
            true,
            default_textured_instance_shaders.clone(),
        );

        let texture_array_instance_material = Material::from_pipeline(
            &texture_array_instance_pipeline,
            true,
            default_texture_array_instance_shaders.clone(),
        );

        Ok((
            pipelines,
            Self {
                instance,
                device,
                queue,
                render_pass,
                subpass,
                default_shaders,
                default_instance_shaders,
                default_material,
                textured_material,
                texture_array_material,
                textured_instance_material,
                texture_array_instance_material,
                default_instance_material,
            },
        ))
    }
}

/// Sets the dynamic viewport up to work with the newly set resolution of the window.
//  For games make the viewport less dynamic.
pub fn window_size_dependent_setup(
    images: &[Arc<Image>],
    render_pass: Arc<RenderPass>,
    viewport: &mut Viewport,
) -> Result<Vec<Arc<Framebuffer>>> {
    let dimensions = images[0].extent();
    viewport.extent = [dimensions[0] as f32, dimensions[1] as f32];

    let framebuffers: Vec<Arc<Framebuffer>> = images
        .iter()
        .map(|image| {
            let view = ImageView::new_default(image.clone())
                .context("Could not make a frame texture.")
                .unwrap();
            Framebuffer::new(
                render_pass.clone(),
                FramebufferCreateInfo {
                    attachments: vec![view],
                    ..Default::default()
                },
            )
            .context("Could not make a framebuffer to present to the window.")
            .unwrap()
        })
        .collect();

    Ok(framebuffers)
}
