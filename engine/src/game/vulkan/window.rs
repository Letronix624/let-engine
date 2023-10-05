extern crate image;
extern crate vulkano;
use std::sync::Arc;
use vulkano::instance::Instance;
use vulkano::swapchain::Surface;
use vulkano_win::VkSurfaceBuild;
use winit::{event_loop::EventLoop, window::WindowBuilder};

/// Returns the event loop and window surface.
pub fn create_window(
    instance: &Arc<Instance>,
    builder: WindowBuilder,
) -> (EventLoop<()>, Arc<Surface>) {
    let event_loop = winit::event_loop::EventLoopBuilder::new().build();

    let surface = builder
        .build_vk_surface(&event_loop, instance.clone())
        .unwrap();

    (event_loop, surface)
}
