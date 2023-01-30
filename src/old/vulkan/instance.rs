extern crate image;
extern crate vulkano;
use crate::consts::*;
use std::sync::Arc;
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{
    physical::PhysicalDeviceType, DeviceCreateInfo, DeviceExtensions, QueueCreateInfo,
};
use vulkano::device::{Device, Queue};
use vulkano::instance::{debug::*, Instance, InstanceCreateInfo, InstanceExtensions};
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
        application_name: Some(APPNAME.into()),
        application_version: Version {
            major: (0),
            minor: (0),
            patch: (0),
        },
        enabled_extensions: extensions,
        engine_name: Some("Let Engine".into()),
        engine_version: Version {
            major: (0),
            minor: (1),
            patch: (0),
        },
        ..Default::default()
    };
    Instance::new(library, gameinfo).expect("Couldn't start Vulkan.")
}
pub fn setup_debug(instance: &Arc<Instance>) -> Option<DebugUtilsMessenger> {
    unsafe {
        DebugUtilsMessenger::new(
            instance.clone(),
            DebugUtilsMessengerCreateInfo {
                message_severity: DebugUtilsMessageSeverity {
                    error: true,
                    warning: true,
                    information: true,
                    verbose: true,
                    ..DebugUtilsMessageSeverity::empty()
                },
                message_type: DebugUtilsMessageType {
                    general: true,
                    validation: true,
                    performance: true,
                    ..DebugUtilsMessageType::empty()
                },
                ..DebugUtilsMessengerCreateInfo::user_callback(Arc::new(|msg| {
                    let severity = if msg.severity.error {
                        "error"
                    } else if msg.severity.warning {
                        "warning"
                    } else if msg.severity.information {
                        "information"
                    } else if msg.severity.verbose {
                        "verbose"
                    } else {
                        panic!("no-impl");
                    };

                    let ty = if msg.ty.general {
                        "general"
                    } else if msg.ty.validation {
                        "validation"
                    } else if msg.ty.performance {
                        "performance"
                    } else {
                        panic!("no-impl");
                    };
                    if severity != "verbose" {
                        println!(
                            "{} {} {}: {}",
                            msg.layer_prefix.unwrap_or("unknown"),
                            ty,
                            severity,
                            msg.description
                        );
                    }
                }))
            },
        )
        .ok()
    }
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
    surface: &Arc<Surface>,
) -> (Arc<PhysicalDevice>, u32) {
    instance
        .enumerate_physical_devices()
        .unwrap()
        .filter(|p| p.supported_extensions().contains(&device_extensions))
        .filter_map(|p| {
            p.queue_family_properties()
                .iter()
                .enumerate()
                .position(|(i, q)| {
                    q.queue_flags.graphics && p.surface_support(i as u32, surface).unwrap_or(false)
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
    queue_family_index: u32,
) -> (Arc<Device>, Arc<Queue>) {
    let (device, mut queues) = Device::new(
        physical_device.clone(),
        DeviceCreateInfo {
            enabled_extensions: device_extensions.clone(),
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
