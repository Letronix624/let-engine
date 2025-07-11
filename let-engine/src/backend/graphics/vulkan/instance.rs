use std::str::FromStr;
use std::sync::Arc;
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{physical::PhysicalDeviceType, DeviceExtensions};
use vulkano::device::{
    Device, DeviceCreateInfo, DeviceFeatures, Queue, QueueCreateInfo, QueueFlags,
};
use vulkano::instance::{Instance, InstanceCreateFlags, InstanceCreateInfo, InstanceExtensions};
use vulkano::swapchain::Surface;
use vulkano::{library::VulkanLibrary, Version};
use winit::raw_window_handle::HasDisplayHandle;

use crate::backend::graphics::{
    DefaultGraphicsBackendError, DefaultGraphicsBackendError::Unsupported,
};

#[derive(Debug)]
pub struct Queues {
    general: Arc<Queue>,
    compute: Option<Arc<Queue>>,
    transfer: Option<Arc<Queue>>,
}

impl Queues {
    fn new(general: Arc<Queue>, compute: Option<Arc<Queue>>, transfer: Option<Arc<Queue>>) -> Self {
        Self {
            general,
            compute,
            transfer,
        }
    }

    pub fn general(&self) -> &Arc<Queue> {
        &self.general
    }

    pub fn compute(&self) -> &Arc<Queue> {
        self.compute.as_ref().unwrap_or(self.general())
    }

    pub fn transfer(&self) -> &Arc<Queue> {
        self.transfer.as_ref().unwrap_or(self.general())
    }
}

/// Initializes a new Vulkan instance.
pub fn create_instance(
    handle: &impl HasDisplayHandle,
    max_retries: usize,
) -> Result<Arc<vulkano::instance::Instance>, DefaultGraphicsBackendError> {
    let library = VulkanLibrary::new().map_err(DefaultGraphicsBackendError::Loading)?;

    let mut required_extensions = None;

    for i in 1..=max_retries {
        match Surface::required_extensions(handle) {
            Err(winit::raw_window_handle::HandleError::Unavailable) => {
                log::error!("Window handle currently unavailable. Retrying... {i}/{max_retries}");
                std::thread::sleep(std::time::Duration::from_millis(500))
            }
            Ok(extensions) => {
                required_extensions = Some(extensions);
                break;
            }
            _ => break,
        }
    }

    let Some(required_extensions) = required_extensions else {
        return Err(Unsupported(
            "The windowing system of this device is not supported by the backend implementation.",
        ));
    };

    let extensions = InstanceExtensions {
        ext_debug_utils: true,
        ..required_extensions
    };

    #[cfg(not(feature = "vulkan_debug"))]
    let layers: Vec<&str> = vec![];
    #[cfg(feature = "vulkan_debug")]
    let layers = vec![
        "VK_LAYER_KHRONOS_validation",
        //"VK_LAYER_VALVE_steam_overlay_64",
    ];

    let game_info = InstanceCreateInfo {
        flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
        enabled_layers: &layers,
        enabled_extensions: &extensions,
        engine_name: Some(env!("CARGO_PKG_NAME")),
        engine_version: Version::from_str(env!("CARGO_PKG_VERSION")).unwrap(),
        ..Default::default()
    };
    vulkano::instance::Instance::new(&library, &game_info).map_err(|e| match e.unwrap() {
        vulkano::VulkanError::InitializationFailed => Unsupported(
            "Initialization of an object could not be completed for Vulkan implementation specific reasons."
        ),
        vulkano::VulkanError::IncompatibleDriver => {
            Unsupported("Incompatible drivers.")
        },
        vulkano::VulkanError::ExtensionNotPresent => {
            Unsupported("Your device does not support all Vulkan extensions required to run this application.")
        }
        e => DefaultGraphicsBackendError::Vulkan(e.into()),
    })
}

/// Selects the physical device by prioritizing preferred device types and evaluating their queue family capabilities.
///
/// the `[Option<usize>; 3]` represents 3 queue family types, graphics, compute and transfer.
/// The usize is the queue family index.
#[allow(clippy::type_complexity)]
fn choose_physical_device(
    instance: &Arc<Instance>,
    device_extensions: &DeviceExtensions,
    features: &DeviceFeatures,
    handle: &impl HasDisplayHandle,
) -> Result<(Arc<PhysicalDevice>, [Option<usize>; 3]), DefaultGraphicsBackendError> {
    let devices = instance.enumerate_physical_devices().map_err(|e| {
        if let vulkano::VulkanError::InitializationFailed = e {
            Unsupported("The Vulkan implementation of your device is incomplete.")
        } else {
            DefaultGraphicsBackendError::Vulkan(e.into())
        }
    })?;

    devices
        .filter(|p| p.supported_extensions().contains(device_extensions))
        .filter(|p| p.supported_features().contains(features))
        .filter_map(|p| find_queue_families(&p, handle).map(|queue_families| (p, queue_families)))
        .min_by_key(|(p, _)| match p.properties().device_type {
            PhysicalDeviceType::DiscreteGpu => 0,
            PhysicalDeviceType::IntegratedGpu => 1,
            PhysicalDeviceType::VirtualGpu => 2,
            PhysicalDeviceType::Cpu => 3,
            PhysicalDeviceType::Other => 4,
            _ => 5,
        })
        .ok_or_else(|| Unsupported("No graphics device suitable for drawing the game found."))
}

/// Extracts queue family information from a physical device.
/// Returns Some(queue_families) if the device meets the requirements,
/// otherwise returns None.
fn find_queue_families(
    physical_device: &Arc<PhysicalDevice>,
    handle: &impl HasDisplayHandle,
) -> Option<[Option<usize>; 3]> {
    let mut queue_families = [None; 3];
    let mut flags = QueueFlags::empty();

    for (id, family) in physical_device.queue_family_properties().iter().enumerate() {
        // If the queue family does not support presentation and supports graphics,
        // skip this family.
        if !physical_device
            .presentation_support(id as u32, handle)
            .unwrap_or(false)
            && family.queue_flags.intersects(QueueFlags::GRAPHICS)
        {
            continue;
        };

        if family.queue_flags.contains(QueueFlags::GRAPHICS) {
            queue_families[0] = Some(id);
        } else if family.queue_flags.intersects(QueueFlags::COMPUTE) {
            queue_families[1] = Some(id);
        } else if family.queue_flags.intersects(
            QueueFlags::VIDEO_DECODE | QueueFlags::VIDEO_ENCODE | QueueFlags::OPTICAL_FLOW,
        ) {
            // Ignore unneeded queues
            continue;
        } else if family.queue_flags.intersects(QueueFlags::TRANSFER) {
            queue_families[2] = Some(id);
        }

        flags |= family.queue_flags;
    }

    // Ensure that at least one queue family provides graphics, compute, and transfer capabilities.
    if flags.contains(QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER) {
        Some(queue_families)
    } else {
        None
    }
}

/// Makes the device and queues.
pub fn create_device_and_queues(
    instance: &Arc<Instance>,
    features: &DeviceFeatures,
    handle: &impl HasDisplayHandle,
) -> Result<(Arc<Device>, Arc<Queues>), DefaultGraphicsBackendError> {
    let device_extensions = DeviceExtensions {
        khr_swapchain: true,
        // ext_line_rasterization: true,
        ..DeviceExtensions::empty()
    };

    // selects the physical device to be used using this order of preferred devices as well as a list of queue families.
    let (physical_device, queue_families) =
        choose_physical_device(instance, &device_extensions, features, handle)?;

    let queue_create_infos: Vec<QueueCreateInfo> = queue_families
        .iter()
        .filter_map(|queue_family_index| {
            queue_family_index.map(|index| QueueCreateInfo {
                queue_family_index: index as u32,
                queues: &[1.0],
                ..Default::default()
            })
        })
        .collect();

    let (device, mut queues) = Device::new(
        &physical_device,
        &DeviceCreateInfo {
            enabled_extensions: &device_extensions,
            enabled_features: features,
            queue_create_infos: &queue_create_infos,

            ..Default::default()
        },
    )
    .map_err(|e| match e.unwrap() {
        vulkano::VulkanError::FeatureNotPresent => Unsupported(
            "Your device does not support all features required to run this application.",
        ),
        e => DefaultGraphicsBackendError::Vulkan(e.into()),
    })?;

    let [general, compute, transfer] = queue_families.map(|x| x.map(|_| queues.next().unwrap()));

    // Create specialized queues.
    let queues = Arc::new(Queues::new(general.unwrap(), compute, transfer));

    Ok((device, queues))
}
