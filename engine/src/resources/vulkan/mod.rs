mod instance;
mod pipeline;
pub mod shaders;
pub use shaders::*;
#[cfg(feature = "vulkan_debug_utils")]
mod debug;
pub mod swapchain;
pub(crate) mod window;

use crate::prelude::*;
use crate::resources::data::Vertex as GameVertex;
use anyhow::{Context, Error, Result};
#[cfg(feature = "vulkan_debug_utils")]
use vulkano::instance::debug::DebugUtilsMessenger;
use vulkano::{
    device::{Device, DeviceFeatures, Queue},
    image::{view::ImageView, Image},
    pipeline::{
        graphics::{vertex_input::Vertex, viewport::Viewport},
        GraphicsPipeline,
    },
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
};

use std::sync::Arc;

/// Just a holder of general immutable information about Vulkan.
#[derive(Clone)]
pub(crate) struct Vulkan {
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

    #[cfg(feature = "vulkan_debug_utils")]
    _debug: Arc<DebugUtilsMessenger>,
}

impl Vulkan {
    pub fn init() -> Result<Self> {
        EVENT_LOOP.with_borrow(|event_loop| {
        let instance = instance::create_instance(event_loop.get().ok_or(Error::msg("There was a problem getting the event loop."))?)?;
        #[cfg(feature = "vulkan_debug_utils")]
        let _debug = Arc::new(debug::make_debug(&instance)?);
        let (surface, _window) =
            window::create_window(event_loop.get().ok_or(Error::msg("There was a problem getting the event loop."))?, &instance, WindowBuilder::new())?;

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

        let subpass = Subpass::from(render_pass.clone(), 0).ok_or(Error::msg("There was a problem making a subpass from the last render pass."))?;

        //Materials
        let vs = vertex_shader(device.clone())?;
        let fs = fragment_shader(device.clone())?;
        let default_shaders = Shaders::from_modules(vs.clone(), fs.clone(), "main");

        let tfs = textured_fragment_shader(device.clone())?;
        let tafs = texture_array_fragment_shader(device.clone())?;

        let instance_vert = instanced_vertex_shader(device.clone())?;
        let instance_frag = instanced_fragment_shader(device.clone())?;
        let textured_instance_frag = instanced_textured_fragment_shader(device.clone())?;
        let textue_array_instance_frag = instanced_texture_array_fragment_shader(device.clone())?;

        let default_instance_shaders =
            Shaders::from_modules(instance_vert.clone(), instance_frag.clone(), "main");

        let vertex_buffer_description = [GameVertex::per_vertex(), InstanceData::per_instance()];

        let pipeline: Arc<GraphicsPipeline> = pipeline::create_pipeline(
            &device,
            &vs,
            &fs,
            subpass.clone(),
            vertex_buffer_description[0].clone(),
        )?;
        let textured_pipeline = pipeline::create_pipeline(
            &device,
            &vs,
            &tfs,
            subpass.clone(),
            vertex_buffer_description[0].clone(),
        )?;
        let texture_array_pipeline = pipeline::create_pipeline(
            &device,
            &vs,
            &tafs,
            subpass.clone(),
            vertex_buffer_description[0].clone(),
        )?;
        let instance_pipeline = pipeline::create_pipeline(
            &device,
            &instance_vert,
            &instance_frag,
            subpass.clone(),
            vertex_buffer_description.clone(),
        )?;
        let textured_instance_pipeline = pipeline::create_pipeline(
            &device,
            &instance_vert,
            &textured_instance_frag,
            subpass.clone(),
            vertex_buffer_description.clone(),
        )?;
        let texture_array_instance_pipeline = pipeline::create_pipeline(
            &device,
            &instance_vert,
            &textue_array_instance_frag,
            subpass.clone(),
            vertex_buffer_description.clone(),
        )?;

        let default_material = Material {
            pipeline,
            descriptor: None,
            texture: None,
            layer: 0,
            instanced: false,
        };
        let textured_material = Material {
            pipeline: textured_pipeline,
            descriptor: None,
            texture: None,
            layer: 0,
            instanced: false,
        };
        let texture_array_material = Material {
            pipeline: texture_array_pipeline,
            descriptor: None,
            texture: None,
            layer: 0,
            instanced: false,
        };
        let default_instance_material = Material {
            pipeline: instance_pipeline,
            descriptor: None,
            texture: None,
            layer: 0,
            instanced: true,
        };

        let textured_instance_material = Material {
            pipeline: textured_instance_pipeline,
            descriptor: None,
            texture: None,
            layer: 0,
            instanced: true,
        };

        let texture_array_instance_material = Material {
            pipeline: texture_array_instance_pipeline,
            descriptor: None,
            texture: None,
            layer: 0,
            instanced: true,
        };

        Ok(Self {
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
            #[cfg(feature = "vulkan_debug_utils")]
            _debug,
        })
        })
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

    images
        .iter()
        .map(|image| {
            let view =
                ImageView::new_default(image.clone()).context("Could not make a frame texture.")?;
            Framebuffer::new(
                render_pass.clone(),
                FramebufferCreateInfo {
                    attachments: vec![view],
                    ..Default::default()
                },
            )
            .context("Could not make a framebuffer to present to the window.")
        })
        .collect()
}
