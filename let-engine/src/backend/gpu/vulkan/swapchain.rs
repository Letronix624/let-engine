use glam::UVec2;
use std::sync::{Arc, OnceLock};
use vulkano::device::Device;
use vulkano::format::Format;
use vulkano::image::ImageUsage;
use vulkano::swapchain::{PresentMode, Surface, SurfaceInfo, Swapchain, SwapchainCreateInfo};
use vulkano_taskgraph::Id;
use winit::window::Window;

use crate::backend::gpu::VulkanError;

use super::Vulkan;

// Creates the swapchain.
pub fn create_swapchain(
    device: &Arc<Device>,
    surface: Arc<Surface>,
    present_modes: &OnceLock<Box<[crate::backend::gpu::PresentMode]>>,
    vulkan: &Vulkan,
) -> Result<(Id<Swapchain>, UVec2, Format), VulkanError> {
    let surface_capabilities = device
        .physical_device()
        .surface_capabilities(&surface, &Default::default())
        .map_err(|e| VulkanError::from(e.unwrap()))?;
    let image_format = device
        .physical_device()
        .surface_formats(&surface, &Default::default())
        .map_err(|e| VulkanError::from(e.unwrap()))?[0]
        .0;

    let inner_size = UVec2::from_array(
        surface
            .object()
            .unwrap()
            .downcast_ref::<Window>()
            .unwrap()
            .inner_size()
            .into(),
    );

    // Give available present modes
    let mut available_present_modes: Vec<_> = device
        .physical_device()
        .surface_present_modes(&surface, &SurfaceInfo::default())
        .map_err(|e| VulkanError::from(e.unwrap()))?
        .into_iter()
        .map(|x| x.into())
        .collect();
    available_present_modes.sort();
    available_present_modes.dedup();

    present_modes
        .set(available_present_modes.into_boxed_slice())
        .unwrap();

    let create_info = SwapchainCreateInfo {
        min_image_count: surface_capabilities.min_image_count,
        image_format,
        image_extent: inner_size.into(),
        image_usage: ImageUsage::COLOR_ATTACHMENT,
        present_mode: PresentMode::Fifo,
        composite_alpha: surface_capabilities
            .supported_composite_alpha
            .into_iter()
            .next()
            .unwrap(),
        ..Default::default()
    };

    Ok((
        vulkan
            .resources
            .create_swapchain(&surface, &create_info)
            .map_err(|e| VulkanError::from(e.unwrap()))?,
        inner_size,
        image_format,
    ))
}
