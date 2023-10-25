extern crate image;
extern crate vulkano;
use crate::window::{Window, WindowBuilder};
use std::sync::Arc;
use vulkano::instance::Instance;
use vulkano::swapchain::Surface;
use winit::event_loop::EventLoop;

/// Returns the event loop and window surface.
pub fn create_window(
    event_loop: &EventLoop<()>,
    instance: &Arc<Instance>,
    builder: WindowBuilder,
) -> (Arc<Surface>, Window) {
    let clear_color = builder.clear_color;
    let builder: winit::window::WindowBuilder = builder.into();
    let window = Arc::new(builder.build(event_loop).unwrap());

    let surface = Surface::from_window(instance.clone(), window.clone()).unwrap();

    let window = Window::new(window, clear_color);
    (surface, window)
}
