extern crate image;
extern crate vulkano;
use crate::window::{Window, WindowBuilder};
use std::sync::Arc;
use vulkano::instance::Instance;
use vulkano::swapchain::Surface;
use vulkano_win::VkSurfaceBuild;
use winit::event_loop::EventLoop;

/// Returns the event loop and window surface.
pub fn create_window(
    event_loop: &EventLoop<()>,
    instance: &Arc<Instance>,
    builder: WindowBuilder,
) -> (Arc<Surface>, Window) {
    let clear_color = builder.clear_color;
    let builder: winit::window::WindowBuilder = builder.into();
    let surface = builder
        .build_vk_surface(event_loop, instance.clone())
        .unwrap();

    let window = Window::new(surface.clone(), clear_color);

    (surface, window)
}
