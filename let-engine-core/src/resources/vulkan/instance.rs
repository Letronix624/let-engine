use anyhow::anyhow;
use std::sync::Arc;
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{
    physical::PhysicalDeviceType, DeviceCreateInfo, DeviceExtensions, QueueCreateInfo,
};
use vulkano::device::{Device, DeviceFeatures, Queue, QueueFlags};
use vulkano::instance::{InstanceCreateFlags, InstanceCreateInfo, InstanceExtensions};
use vulkano::swapchain::Surface;
use vulkano::{library::VulkanLibrary, Version};
use winit::event_loop::EventLoop;

use crate::EngineError;

/// Initializes a new Vulkan instance.
pub fn create_instance(
    event_loop: &EventLoop<()>,
) -> Result<Arc<vulkano::instance::Instance>, EngineError> {
    let library = VulkanLibrary::new().map_err(|e| EngineError::RequirementError(e.to_string()))?;
    let required_extensions = Surface::required_extensions(event_loop)
        .map_err(|e| EngineError::RequirementError(e.to_string()))?;

    let extensions = InstanceExtensions {
        ext_debug_utils: true,
        ..required_extensions
    };

    #[cfg(not(feature = "vulkan_debug_utils"))]
    let layers: Vec<String> = vec![];
    #[cfg(feature = "vulkan_debug_utils")]
    let layers = vec![
        "VK_LAYER_KHRONOS_validation".to_owned(),
        //"VK_LAYER_VALVE_steam_overlay_64".to_owned(),
    ];

    let game_info = InstanceCreateInfo {
        flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
        enabled_layers: layers,
        enabled_extensions: extensions,
        engine_name: Some("Let Engine".into()),
        engine_version: Version {
            major: env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
            minor: env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(),
            patch: env!("CARGO_PKG_VERSION_PATCH").parse().unwrap(),
        },
        ..Default::default()
    };
    vulkano::instance::Instance::new(library, game_info)
        .map_err(|e| EngineError::RequirementError(e.to_string()))
}
pub fn create_device_extensions() -> DeviceExtensions {
    DeviceExtensions {
        khr_swapchain: true,
        // ext_line_rasterization: true,
        ..DeviceExtensions::empty()
    }
}

/// Makes a physical device.
pub fn create_physical_device(
    instance: &Arc<vulkano::instance::Instance>,
    device_extensions: DeviceExtensions,
    features: DeviceFeatures,
    surface: &Arc<Surface>,
) -> Result<(Arc<PhysicalDevice>, u32), EngineError> {
    // selects the physical device to be used using this order of preferred devices.
    instance
        .enumerate_physical_devices()
        .map_err(|e| {
            EngineError::RequirementError(format!(
                "There was an error enumerating the physical devices of this instance: {e}"
            ))
        })?
        .filter(|p| p.supported_extensions().contains(&device_extensions))
        .filter(|p| p.supported_features().contains(&features))
        .filter_map(|p| {
            p.queue_family_properties()
                .iter()
                .enumerate()
                .position(|(i, q)| {
                    q.queue_flags.intersects(QueueFlags::GRAPHICS)
                        && p.surface_support(i as u32, surface).unwrap_or(false)
                })
                .map(|i| (p, i as u32))
        })
        .min_by_key(|(p, _)| match p.properties().device_type {
            PhysicalDeviceType::DiscreteGpu => 0,
            PhysicalDeviceType::IntegratedGpu => 1,
            PhysicalDeviceType::VirtualGpu => 2,
            PhysicalDeviceType::Cpu => 3,
            PhysicalDeviceType::Other => 4,
            _ => 5,
        })
        .ok_or(EngineError::RequirementError(
            "No suitable GPU was found.".to_string(),
        ))
}

/// Makes the device and queues.
pub fn create_device_and_queues(
    physical_device: &Arc<PhysicalDevice>,
    device_extensions: &DeviceExtensions,
    features: DeviceFeatures,
    queue_family_index: u32,
) -> Result<(Arc<Device>, Arc<Queue>), EngineError> {
    let (device, mut queues) = Device::new(
        physical_device.clone(),
        DeviceCreateInfo {
            enabled_extensions: *device_extensions,
            enabled_features: features,
            queue_create_infos: vec![QueueCreateInfo {
                queue_family_index,
                ..Default::default()
            }],

            ..Default::default()
        },
    )
    .map_err(|e| EngineError::RequirementError(e.to_string()))?;
    Ok((
        device,
        queues.next().ok_or(EngineError::Other(anyhow!(
            "The graphics queue has no slots.".to_string()
        )))?,
    ))
}
