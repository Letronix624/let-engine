use std::sync::atomic::{AtomicUsize, Ordering};
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
    general: Vec<Arc<Queue>>,
    graphics: Vec<Arc<Queue>>,
    compute: Vec<Arc<Queue>>,
    transfer: Vec<Arc<Queue>>,

    general_id: AtomicUsize,
    graphics_id: AtomicUsize,
    compute_id: AtomicUsize,
    transfer_id: AtomicUsize,
}

impl Queues {
    fn new(
        general: Vec<Arc<Queue>>,
        graphics: Vec<Arc<Queue>>,
        compute: Vec<Arc<Queue>>,
        transfer: Vec<Arc<Queue>>,
    ) -> Self {
        // If there are no general queues, assure each specialized queue is present.
        assert!(
            !general.is_empty()
                || !(graphics.is_empty() || compute.is_empty() || transfer.is_empty())
        );

        Self {
            general,
            graphics,
            compute,
            transfer,

            general_id: 0.into(),
            graphics_id: 0.into(),
            compute_id: 0.into(),
            transfer_id: 0.into(),
        }
    }

    fn get_general(&self) -> &Arc<Queue> {
        let id = self.general_id.fetch_add(1, Ordering::Relaxed) % self.general.len();
        &self.general[id]
    }

    pub fn get_graphics(&self) -> &Arc<Queue> {
        if self.graphics.is_empty() {
            return self.get_general();
        };

        let id = self.graphics_id.fetch_add(1, Ordering::Relaxed) % self.graphics.len();
        &self.graphics[id]
    }

    pub fn graphics_id(&self) -> u32 {
        if let Some(graphics_queue) = self.graphics.first() {
            graphics_queue.queue_family_index()
        } else {
            self.general[0].queue_family_index()
        }
    }

    pub fn get_compute(&self) -> &Arc<Queue> {
        if self.compute.is_empty() {
            return self.get_general();
        };

        let id = self.compute_id.fetch_add(1, Ordering::Relaxed) % self.compute.len();
        &self.compute[id]
    }

    pub fn compute_id(&self) -> u32 {
        if let Some(compute_queue) = self.compute.first() {
            compute_queue.queue_family_index()
        } else {
            self.general[0].queue_family_index()
        }
    }

    pub fn get_transfer(&self) -> &Arc<Queue> {
        if self.transfer.is_empty() {
            return self.get_general();
        };

        let id = self.transfer_id.fetch_add(1, Ordering::Relaxed) % self.transfer.len();
        &self.transfer[id]
    }

    pub fn transfer_id(&self) -> u32 {
        if let Some(transfer_queue) = self.transfer.first() {
            transfer_queue.queue_family_index()
        } else {
            self.general[0].queue_family_index()
        }
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
    let layers: Vec<String> = vec![];
    #[cfg(feature = "vulkan_debug")]
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
    vulkano::instance::Instance::new(library, game_info).map_err(|e| match e.unwrap() {
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

// Selects the physical device by prioritizing preferred device types and evaluating their queue family capabilities.
fn choose_physical_device(
    instance: &Arc<Instance>,
    device_extensions: &DeviceExtensions,
    features: &DeviceFeatures,
    handle: &impl HasDisplayHandle,
) -> Result<(Arc<PhysicalDevice>, [Option<(usize, u32)>; 4]), DefaultGraphicsBackendError> {
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

// Extracts queue family information from a physical device.
// Returns Some(queue_families) if the device meets the requirements,
// otherwise returns None.
fn find_queue_families(
    physical_device: &Arc<PhysicalDevice>,
    handle: &impl HasDisplayHandle,
) -> Option<[Option<(usize, u32)>; 4]> {
    let mut queue_families = [None; 4];
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

        if family
            .queue_flags
            .contains(QueueFlags::GRAPHICS | QueueFlags::COMPUTE | QueueFlags::TRANSFER)
        {
            queue_families[0] = Some((id, family.queue_count));
        } else if family.queue_flags.intersects(QueueFlags::GRAPHICS) {
            queue_families[1] = Some((id, family.queue_count));
        } else if family.queue_flags.intersects(QueueFlags::COMPUTE) {
            queue_families[2] = Some((id, family.queue_count));
        } else if family.queue_flags.intersects(
            QueueFlags::VIDEO_DECODE | QueueFlags::VIDEO_ENCODE | QueueFlags::OPTICAL_FLOW,
        ) {
            // Ignore unneeded queues
            continue;
        } else if family.queue_flags.intersects(QueueFlags::TRANSFER) {
            queue_families[3] = Some((id, family.queue_count));
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
    features: DeviceFeatures,
    handle: &impl HasDisplayHandle,
) -> Result<(Arc<Device>, Arc<Queues>), DefaultGraphicsBackendError> {
    let device_extensions = DeviceExtensions {
        khr_swapchain: true,
        // ext_line_rasterization: true,
        ..DeviceExtensions::empty()
    };

    // selects the physical device to be used using this order of preferred devices as well as a list of queue families.
    let (physical_device, queue_families) =
        choose_physical_device(instance, &device_extensions, &features, handle)?;

    // Create create infos for each queue family with all queues with priority 0.5
    let queue_create_infos: Vec<QueueCreateInfo> = queue_families
        .iter()
        .filter_map(|x| {
            if let Some((queue_family_index, queues)) = x {
                Some(QueueCreateInfo {
                    queue_family_index: *queue_family_index as u32,
                    queues: vec![0.5; *queues as usize],
                    ..Default::default()
                })
            } else {
                None
            }
        })
        .collect();

    let (device, queues) = Device::new(
        physical_device.clone(),
        DeviceCreateInfo {
            enabled_extensions: device_extensions,
            enabled_features: features,
            queue_create_infos,

            ..Default::default()
        },
    )
    .map_err(|e| match e.unwrap() {
        vulkano::VulkanError::FeatureNotPresent => Unsupported(
            "Your device does not support all features required to run this application.",
        ),
        e => DefaultGraphicsBackendError::Vulkan(e.into()),
    })?;

    let queues: Vec<Arc<Queue>> = queues.collect();

    // Determine which range of the queues vec belongs to which specialisation
    let r: [usize; 4] = {
        let mut last = 0;
        queue_families.map(|x| {
            if let Some((_, queues)) = x {
                last += queues as usize;
                last
            } else {
                last
            }
        })
    };

    // Create specialized queues.
    let queues = Arc::new(Queues::new(
        queues[0..r[0]].to_vec(),
        queues[r[0]..r[1]].to_vec(),
        queues[r[1]..r[2]].to_vec(),
        queues[r[2]..r[3]].to_vec(),
    ));

    Ok((device, queues))
}
