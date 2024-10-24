use crate::window::{Window, WindowBuilder};
use anyhow::Result;
use std::sync::Arc;
use vulkano::instance::Instance;
use vulkano::swapchain::Surface;
use winit::event_loop::EventLoop;

/// Returns the event loop and window surface.
pub fn create_window(
    event_loop: &EventLoop<()>,
    instance: &Arc<Instance>,
    builder: WindowBuilder,
) -> Result<(Arc<Surface>, Arc<Window>)> {
    let clear_color = builder.clear_color;
    let visible = builder.visible;
    let attributes: winit::window::WindowAttributes = builder.into();
    let window: Arc<winit::window::Window> = event_loop.create_window(attributes)?.into();

    let surface = Surface::from_window(instance.clone(), window.clone())?;

    let window: Arc<Window> = Arc::new((window, visible).into());
    window.set_clear_color(clear_color);
    Ok((surface, window))
}
