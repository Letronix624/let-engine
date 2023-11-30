extern crate image;
extern crate vulkano;
use std::sync::Arc;
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::Device;
use vulkano::image::{Image, ImageUsage};
use vulkano::swapchain::{PresentMode, Surface, SurfaceInfo, Swapchain, SwapchainCreateInfo};
use winit::window::Window;

// Creates the swapchain.
pub fn create_swapchain_and_images(
    physical_device: &Arc<PhysicalDevice>,
    device: &Arc<Device>,
    surface: &Arc<Surface>,
) -> (Arc<Swapchain>, Vec<Arc<Image>>) {
    let surface_capabilities = device
        .physical_device()
        .surface_capabilities(surface, Default::default())
        .unwrap();
    let image_format = device
        .physical_device()
        .surface_formats(surface, Default::default())
        .unwrap()[0]
        .0;
    let innersize = surface
        .object()
        .unwrap()
        .downcast_ref::<Window>()
        .unwrap()
        .inner_size()
        .into();
    let present_mode = physical_device
        .surface_present_modes(surface, SurfaceInfo::default())
        .unwrap()
        .min_by_key(|compare| match compare {
            PresentMode::Mailbox => 0,
            PresentMode::Immediate => 1,
            PresentMode::Fifo => 2,
            _ => 3,
        })
        .unwrap();
    let create_info = SwapchainCreateInfo {
        min_image_count: surface_capabilities.min_image_count,
        image_format,
        image_extent: innersize,
        image_usage: ImageUsage::COLOR_ATTACHMENT,
        present_mode,
        composite_alpha: surface_capabilities
            .supported_composite_alpha
            .into_iter()
            .next()
            .unwrap(),
        ..Default::default()
    };
    match Swapchain::new(device.clone(), surface.clone(), create_info) {
        Ok(t) => t,
        Err(e) => panic!("{e}"),
    }
}
