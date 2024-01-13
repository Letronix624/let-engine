extern crate image;
extern crate vulkano;
use anyhow::{Context, Error};
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
) -> anyhow::Result<(Arc<Swapchain>, Vec<Arc<Image>>)> {
    let surface_capabilities = device
        .physical_device()
        .surface_capabilities(surface, Default::default())?;
    let image_format = device
        .physical_device()
        .surface_formats(surface, Default::default())?[0]
        .0;
    let innersize = surface
        .object()
        .ok_or(Error::msg("Failed to cast the surface to a window."))?
        .downcast_ref::<Window>()
        .ok_or(Error::msg("Failed to cast the surface to a window."))?
        .inner_size()
        .into();
    let present_mode = physical_device
        .surface_present_modes(surface, SurfaceInfo::default())?
        .min_by_key(|compare| match compare {
            PresentMode::Mailbox => 0,
            PresentMode::Immediate => 1,
            PresentMode::Fifo => 2,
            _ => 3,
        })
        .ok_or(Error::msg(
            "Failed to get any presentation mode on this device.",
        ))?;
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
            .ok_or(Error::msg(
                "Failed to find a supported compositor on this device.",
            ))?,
        ..Default::default()
    };
    Swapchain::new(device.clone(), surface.clone(), create_info)
        .context("Failed to create a swapchain.")
}
