use glam::UVec2;
use std::sync::Arc;
use vulkano::device::Device;
use vulkano::image::ImageUsage;
use vulkano::swapchain::{PresentMode, Surface, SurfaceInfo, Swapchain, SwapchainCreateInfo};
use vulkano_taskgraph::Id;
use winit::window::Window;

use crate::backend::graphics::{GraphicsInterface, VulkanError};

use super::Vulkan;

// Creates the swapchain.
pub fn create_swapchain(
    device: &Arc<Device>,
    surface: Arc<Surface>,
    interface: &GraphicsInterface,
    vulkan: &Vulkan,
) -> Result<(Id<Swapchain>, UVec2), VulkanError> {
    let surface_capabilities = device
        .physical_device()
        .surface_capabilities(&surface, Default::default())
        .map_err(|e| VulkanError::from(e.unwrap()))?;
    let image_format = device
        .physical_device()
        .surface_formats(&surface, Default::default())
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

    let present_mode = device
        .physical_device()
        .surface_present_modes(&surface, SurfaceInfo::default())
        .map_err(|e| VulkanError::from(e.unwrap()))?
        .into_iter()
        .min_by_key(|compare| match compare {
            PresentMode::Mailbox => 0,
            PresentMode::Immediate => 1,
            PresentMode::Fifo => 2,
            _ => 3,
        })
        .unwrap(); // This has to at least contain `Fifo`

    // Set the present mode of the game engine to this.
    interface.settings.write().present_mode = present_mode.into();

    // Give available present modes
    let mut present_modes: Vec<_> = device
        .physical_device()
        .surface_present_modes(&surface, SurfaceInfo::default())
        .map_err(|e| VulkanError::from(e.unwrap()))?
        .into_iter()
        .map(|x| x.into())
        .collect();
    present_modes.dedup();

    *interface.available_present_modes.write() = present_modes;

    let create_info = SwapchainCreateInfo {
        min_image_count: surface_capabilities.min_image_count,
        image_format,
        image_extent: inner_size.into(),
        image_usage: ImageUsage::COLOR_ATTACHMENT,
        present_mode,
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
            .create_swapchain(vulkan.graphics_flight, surface, create_info)
            .map_err(|e| VulkanError::from(e.unwrap()))?,
        inner_size,
    ))
}
