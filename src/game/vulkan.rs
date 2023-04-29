mod instance;
mod pipeline;
pub mod shaders;
pub use shaders::*;
mod swapchain;
mod window;

use vulkano::{
    device::{physical::PhysicalDevice, Device, DeviceExtensions, Features, Queue},
    image::{view::ImageView, ImageAccess, SwapchainImage},
    instance::Instance,
    pipeline::{graphics::viewport::Viewport, GraphicsPipeline},
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
    swapchain::{Surface, Swapchain},
};
use winit::{event_loop::EventLoop, window::WindowBuilder};

use std::sync::Arc;

use super::{materials, AppInfo};

pub struct Vulkan {
    pub instance: Arc<Instance>,
    pub surface: Arc<Surface>,
    pub device_extensions: DeviceExtensions,
    pub features: Features,
    pub physical_device: Arc<PhysicalDevice>,
    pub queue_family_index: u32,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub swapchain: Arc<Swapchain>,
    pub images: Vec<Arc<SwapchainImage>>,
    pub render_pass: Arc<RenderPass>,
    pub default_material: materials::Material,
    pub textured_material: materials::Material,
    pub texture_array_material: materials::Material,
    pub viewport: Viewport,
    pub framebuffers: Vec<Arc<Framebuffer>>,
}

impl Vulkan {
    pub fn init(
        window_builder: WindowBuilder,
        app_info: AppInfo,
    ) -> (Self, EventLoop<()>) {
        let instance = instance::create_instance(app_info.app_name.to_string());
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

        let (swapchain, images) = swapchain::create_swapchain_and_images(&device, &surface);

        let render_pass = vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: swapchain.image_format(),
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
        let texture_array_pipeline = pipeline::create_pipeline(&device, &vs, &tafs, subpass.clone());

        let default_material = materials::Material {
            pipeline: pipeline,
            descriptor: None,
            texture: None,
            layer: 0
        };
        let textured_material = materials::Material {
            pipeline: textured_pipeline,
            descriptor: None,
            texture: None,
            layer: 0
        };
        let texture_array_material = materials::Material {
            pipeline: texture_array_pipeline,
            descriptor: None,
            texture: None,
            layer: 0
        };
        //

        let mut viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [0.0, 0.0],
            depth_range: 0.0..1.0,
        };

        let framebuffers = window_size_dependent_setup(&images, render_pass.clone(), &mut viewport);

        (
            Self {
                instance,
                surface,
                device_extensions,
                features,
                physical_device,
                queue_family_index,
                device,
                queue,
                swapchain,
                images,
                render_pass,
                default_material,
                textured_material,
                texture_array_material,
                viewport,
                framebuffers,
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
