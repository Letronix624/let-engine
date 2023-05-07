use std::sync::Arc;
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{
    physical::PhysicalDeviceType, DeviceCreateInfo, DeviceExtensions, QueueCreateInfo,
};
use vulkano::device::{Device, Features, Queue, QueueFlags};
use vulkano::instance::{Instance, InstanceCreateInfo, InstanceExtensions};
use vulkano::swapchain::Surface;
use vulkano::{library::VulkanLibrary, Version};

pub fn create_instance() -> Arc<Instance> {
    let library = match VulkanLibrary::new() {
        Err(e) => {
            println!(
                "Your PC does not support the required Vulkan libraries to run this program.\n{e}"
            );
            std::process::exit(0);
        }
        Ok(a) => a,
    };
    let required_extensions = vulkano_win::required_extensions(&library);
    let extensions = InstanceExtensions {
        ext_debug_utils: true,
        ..required_extensions
    };

    let layers = vec![
        //"VK_LAYER_KHRONOS_validation".to_owned(),
        //"VK_LAYER_VALVE_steam_overlay_64".to_owned(),
    ];

    let gameinfo = InstanceCreateInfo {
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
    Instance::new(library, gameinfo).expect("Couldn't start Vulkan.")
}
pub fn create_device_extensions() -> DeviceExtensions {
    DeviceExtensions {
        khr_swapchain: true,
        ..DeviceExtensions::empty()
    }
}
pub fn create_physical_and_queue(
    instance: &Arc<Instance>,
    device_extensions: DeviceExtensions,
    features: Features,
    surface: &Arc<Surface>,
) -> (Arc<PhysicalDevice>, u32) {
    instance
        .enumerate_physical_devices()
        .unwrap()
        .filter(|p| p.supported_extensions().contains(&device_extensions))
        .filter(|p| p.supported_features().contains(&features))
        .filter_map(|p| {
            p.queue_family_properties()
                .iter()
                .enumerate()
                .position(|(i, q)| {
                    q.queue_flags.intersects(QueueFlags::GRAPHICS)
                        && p.surface_support(i as u32, &surface).unwrap_or(false)
                    //q.queue_flags.graphics && p.surface_support(i as u32, surface).unwrap_or(false)
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
        .expect("No suitable physical device found")
}
pub fn create_device_and_queues(
    physical_device: &Arc<PhysicalDevice>,
    device_extensions: &DeviceExtensions,
    features: Features,
    queue_family_index: u32,
) -> (Arc<Device>, Arc<Queue>) {
    let (device, mut queues) = Device::new(
        physical_device.clone(),
        DeviceCreateInfo {
            enabled_extensions: device_extensions.clone(),
            enabled_features: features,
            queue_create_infos: vec![QueueCreateInfo {
                queue_family_index,
                ..Default::default()
            }],

            ..Default::default()
        },
    )
    .unwrap();
    (device, queues.next().unwrap())
}
