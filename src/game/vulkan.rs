mod instance;
mod pipeline;
pub mod shaders;
pub use shaders::*;
pub mod swapchain;
mod window;

use vulkano::{
    device::{physical::PhysicalDevice, Device, Features, Queue},
    image::{view::ImageView, ImageAccess, SwapchainImage},
    instance::Instance,
    pipeline::{graphics::viewport::Viewport, GraphicsPipeline},
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
    swapchain::Surface,
};
use winit::{event_loop::EventLoop, window::WindowBuilder};

use std::sync::Arc;

use super::materials;

#[derive(Clone)]
pub struct Vulkan {
    pub instance: Arc<Instance>,
    pub surface: Arc<Surface>,
    pub physical_device: Arc<PhysicalDevice>,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub render_pass: Arc<RenderPass>,
    pub subpass: Subpass,
    pub default_material: materials::Material,
    pub textured_material: materials::Material,
    pub texture_array_material: materials::Material,
}

impl Vulkan {
    pub fn init(window_builder: WindowBuilder) -> (Self, EventLoop<()>) {
        let instance = instance::create_instance();
        let (event_loop, surface) = window::create_window(&instance, window_builder);

        let device_extensions = instance::create_device_extensions();
        let features = Features {
            fill_mode_non_solid: true,
            wide_lines: true,
            ..Features::empty()
        };
        let (physical_device, queue_family_index) =
            instance::create_physical_and_queue(&instance, device_extensions, features, &surface);
        let (device, queue) = instance::create_device_and_queues(
            &physical_device,
            &device_extensions,
            features,
            queue_family_index,
        );

        let render_pass = vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: device.physical_device().surface_formats(&surface, Default::default()).unwrap()[0].0,
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {}
            }
        )
        .unwrap();

        let subpass = Subpass::from(render_pass.clone(), 0).unwrap();

        //Materials
        let vs = vertexshader::load(device.clone()).unwrap();
        let fs = fragmentshader::load(device.clone()).unwrap();
        let tfs = textured_fragmentshader::load(device.clone()).unwrap();
        let tafs = texture_array_fragmentshader::load(device.clone()).unwrap();

        let pipeline: Arc<GraphicsPipeline> =
            pipeline::create_pipeline(&device, &vs, &fs, subpass.clone());
        let textured_pipeline = pipeline::create_pipeline(&device, &vs, &tfs, subpass.clone());
        let texture_array_pipeline =
            pipeline::create_pipeline(&device, &vs, &tafs, subpass.clone());

        let default_material = materials::Material {
            pipeline,
            descriptor: None,
            texture: None,
            layer: 0,
        };
        let textured_material = materials::Material {
            pipeline: textured_pipeline,
            descriptor: None,
            texture: None,
            layer: 0,
        };
        let texture_array_material = materials::Material {
            pipeline: texture_array_pipeline,
            descriptor: None,
            texture: None,
            layer: 0,
        };
        //

        (
            Self {
                instance,
                surface,
                physical_device,
                device,
                queue,
                render_pass,
                subpass,
                default_material,
                textured_material,
                texture_array_material,
            },
            event_loop,
        )
    }
}

pub fn window_size_dependent_setup(
    images: &[Arc<SwapchainImage>],
    render_pass: Arc<RenderPass>,
    viewport: &mut Viewport,
) -> Vec<Arc<Framebuffer>> {
    let dimensions = images[0].dimensions().width_height();
    viewport.dimensions = [dimensions[0] as f32, dimensions[1] as f32];

    images
        .iter()
        .map(|image| {
            let view = ImageView::new_default(image.clone()).unwrap();
            Framebuffer::new(
                render_pass.clone(),
                FramebufferCreateInfo {
                    attachments: vec![view],
                    ..Default::default()
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>()
}
