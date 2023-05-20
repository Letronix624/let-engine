extern crate image;
extern crate vulkano;
use std::sync::Arc;
use vulkano::device::Device;
use vulkano::image::{ImageUsage, SwapchainImage};
use vulkano::swapchain::{
    PresentMode, Surface, Swapchain, SwapchainCreateInfo, SwapchainCreationError,
};
use winit::window::Window;

pub fn create_swapchain_and_images(
    device: &Arc<Device>,
    surface: &Arc<Surface>,
) -> (Arc<Swapchain>, Vec<Arc<SwapchainImage>>) {
    let surface_capabilities = device
        .physical_device()
        .surface_capabilities(surface, Default::default())
        .unwrap();
    let image_format = Some(
        device
            .physical_device()
            .surface_formats(surface, Default::default())
            .unwrap()[0]
            .0,
    );
    let innersize = surface
        .object()
        .unwrap()
        .downcast_ref::<Window>()
        .unwrap()
        .inner_size()
        .into();
    let mut swapchain = None;
    for present_mode in [
        PresentMode::Mailbox,
        PresentMode::Immediate,
        PresentMode::FifoRelaxed,
        PresentMode::Fifo,
    ] {
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
        swapchain = Some(
            match Swapchain::new(device.clone(), surface.clone(), create_info) {
                Ok(t) => t,
                Err(SwapchainCreationError::PresentModeNotSupported) => continue,
                Err(e) => panic!("{e}"),
            },
        );
        break;
    }
    swapchain.unwrap()
}
