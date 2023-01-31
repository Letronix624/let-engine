mod instance;
mod pipeline;
pub mod shaders;
use shaders::*;
mod swapchain;
mod window;

use vulkano::{
    device::{physical::PhysicalDevice, Device, DeviceExtensions, Queue},
    image::{view::ImageView, ImageAccess, SwapchainImage},
    instance::{debug::*, Instance},
    pipeline::{graphics::viewport::Viewport, GraphicsPipeline},
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
    shader::ShaderModule,
    swapchain::{Surface, Swapchain},
};
use winit::{event_loop::EventLoop, window::WindowBuilder};

use std::sync::Arc;

use super::AppInfo;

pub struct Vulkan {
    pub instance: Arc<Instance>,
    pub debugmessenger: Option<DebugUtilsMessenger>,
    pub surface: Arc<Surface>,
    pub device_extensions: DeviceExtensions,
    pub physical_device: Arc<PhysicalDevice>,
    pub queue_family_index: u32,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub swapchain: Arc<Swapchain>,
    pub images: Vec<Arc<SwapchainImage>>,
    pub vs: Arc<ShaderModule>,
    pub fs: Arc<ShaderModule>,
    pub render_pass: Arc<RenderPass>,
    pub pipeline: Arc<GraphicsPipeline>,
    pub viewport: Viewport,
    pub framebuffers: Vec<Arc<Framebuffer>>,
}

impl Vulkan {
    pub fn init(window_builder: WindowBuilder, app_info: AppInfo) -> (Self, EventLoop<()>) {
        let instance = instance::create_instance(app_info.app_name.to_string());
        let (event_loop, surface) = window::Window::create_window(&instance, window_builder);
        let debugmessenger = instance::setup_debug(&instance);
        let device_extensions = instance::create_device_extensions();
        let (physical_device, queue_family_index) =
            instance::create_physical_and_queue(&instance, device_extensions, &surface);
        let (device, queue) = instance::create_device_and_queues(
            &physical_device,
            &device_extensions,
            queue_family_index,
        );

        let (swapchain, images) = swapchain::create_swapchain_and_images(&device, &surface);

        let vs = vertexshader::load(device.clone()).unwrap();
        let fs = fragmentshader::load(device.clone()).unwrap();

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
        let pipeline: Arc<GraphicsPipeline> =
            pipeline::create_pipeline(&device, &vs, &fs, subpass.clone());

        let mut viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [0.0, 0.0],
            depth_range: 0.0..1.0,
        };

        let framebuffers = window_size_dependent_setup(&images, render_pass.clone(), &mut viewport);

        (
            Self {
                instance,
                debugmessenger,
                surface,
                device_extensions,
                physical_device,
                queue_family_index,
                device,
                queue,
                swapchain,
                images,
                vs,
                fs,
                render_pass,
                pipeline,
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
