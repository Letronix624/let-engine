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
        .surface_capabilities(&surface, Default::default())
        .unwrap();
    let image_format = Some(
        device
            .physical_device()
            .surface_formats(&surface, Default::default())
            .unwrap()[0]
            .0,
    );
    let image_usage = ImageUsage {
        color_attachment: true,
        ..ImageUsage::empty()
    };
    let innersize = surface
        .object()
        .unwrap()
        .downcast_ref::<Window>()
        .unwrap()
        .inner_size()
        .into();
    let create_info = SwapchainCreateInfo {
        min_image_count: surface_capabilities.min_image_count,
        image_format,
        image_extent: innersize,
        image_usage,
        present_mode: PresentMode::Mailbox,
        composite_alpha: surface_capabilities
            .supported_composite_alpha
            .iter()
            .next()
            .unwrap(),
        ..Default::default()
    };

    let swapchain = match Swapchain::new(device.clone(), surface.clone(), create_info) {
        Ok(t) => t,
        Err(e) => {
            if e == SwapchainCreationError::PresentModeNotSupported {
                let create_info = SwapchainCreateInfo {
                    min_image_count: surface_capabilities.min_image_count,
                    image_format,
                    image_extent: innersize,
                    image_usage,
                    present_mode: PresentMode::Immediate,
                    composite_alpha: surface_capabilities
                        .supported_composite_alpha
                        .iter()
                        .next()
                        .unwrap(),

                    ..Default::default()
                };
                match Swapchain::new(device.clone(), surface.clone(), create_info) {
                    Ok(t) => t,
                    Err(e) => {
                        if e == SwapchainCreationError::PresentModeNotSupported {
                            let create_info = SwapchainCreateInfo {
                                min_image_count: surface_capabilities.min_image_count,
                                image_format,
                                image_extent: innersize,
                                image_usage,
                                present_mode: PresentMode::Fifo,
                                composite_alpha: surface_capabilities
                                    .supported_composite_alpha
                                    .iter()
                                    .next()
                                    .unwrap(),

                                ..Default::default()
                            };
                            Swapchain::new(device.clone(), surface.clone(), create_info).unwrap()
                        } else {
                            panic!("{e}")
                        }
                    }
                }
            } else {
                panic!("{e}")
            }
        }
    };
    println!("{:?}", swapchain.0.present_mode());
    swapchain
}
