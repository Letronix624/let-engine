mod instance;
pub mod shaders;
use anyhow::Result;
use crossbeam::channel::{Receiver, Sender};
use foldhash::HashMap;
pub use instance::Queues;
use let_engine_core::resources::material::Topology;
use parking_lot::Mutex;
use vulkano_taskgraph::{
    resource::{AccessTypes, Flight, Resources},
    Id, InvalidSlotError, Ref,
};
use winit::raw_window_handle::HasDisplayHandle;
#[cfg(feature = "vulkan_debug")]
mod debug;
pub mod swapchain;

use vulkano::{
    buffer::Buffer,
    descriptor_set::allocator::{
        StandardDescriptorSetAllocator, StandardDescriptorSetAllocatorCreateInfo,
    },
    device::{Device, DeviceFeatures},
    image::Image,
    pipeline::{
        cache::{PipelineCache, PipelineCacheCreateInfo},
        graphics::input_assembly::PrimitiveTopology,
        GraphicsPipeline,
    },
};

use std::sync::{Arc, OnceLock};

use super::{material::GpuMaterial, DefaultGraphicsBackendError, Graphics};

pub static VK: OnceLock<Vulkan> = OnceLock::new();

/// Just a holder of general immutable information about Vulkan.
pub struct Vulkan {
    pub instance: Arc<vulkano::instance::Instance>,
    pub device: Arc<Device>,
    pub queues: Arc<Queues>,

    pub descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,

    pub resources: Arc<Resources>,
    pub graphics_flight: Id<Flight>,
    pub transfer_flight: Id<Flight>,

    pub access_queue: (Sender<NewResource>, Receiver<NewResource>),
    pub vulkan_pipeline_cache: Arc<PipelineCache>,
    pub pipeline_cache: Mutex<HashMap<GpuMaterial, Arc<GraphicsPipeline>>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum NewResource {
    Add(Resource),
    Remove(Resource),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Resource {
    Buffer {
        id: Id<Buffer>,
        access_types: AccessTypes,
    },
    Image {
        id: Id<Image>,
        access_types: AccessTypes,
    },
}

impl Vulkan {
    pub fn init(
        handle: &impl HasDisplayHandle,
        settings: &Graphics,
    ) -> Result<Self, DefaultGraphicsBackendError> {
        let instance = instance::create_instance(handle, settings.window_handle_retries)?;

        #[cfg(feature = "vulkan_debug")]
        std::mem::forget(debug::make_debug(&instance).unwrap());

        let features = DeviceFeatures {
            fill_mode_non_solid: true,
            wide_lines: true,
            ..DeviceFeatures::empty()
        };
        let (device, queues) = instance::create_device_and_queues(&instance, &features, handle)?;

        let descriptor_set_allocator = StandardDescriptorSetAllocator::new(
            &device,
            &StandardDescriptorSetAllocatorCreateInfo::default(),
        )
        .into();

        let resources = Resources::new(&device, &Default::default())
            .map_err(|e| DefaultGraphicsBackendError::Vulkan(e.unwrap().into()))?;

        let graphics_flight = resources
            .create_flight(settings.max_frames_in_flight as u32)
            .map_err(|e| DefaultGraphicsBackendError::Vulkan(e.into()))?;

        let transfer_flight = resources
            .create_flight(1)
            .map_err(|e| DefaultGraphicsBackendError::Vulkan(e.into()))?;

        let access_queue = crossbeam::channel::unbounded();

        let vulkan_pipeline_cache =
            PipelineCache::new(&device, &PipelineCacheCreateInfo::default())
                .map_err(|e| DefaultGraphicsBackendError::Vulkan(e.unwrap().into()))?;

        let pipeline_cache = Mutex::new(HashMap::default());

        Ok(Self {
            instance,
            device,
            queues,

            descriptor_set_allocator,

            resources,
            graphics_flight,
            transfer_flight,

            access_queue,
            vulkan_pipeline_cache,
            pipeline_cache,
        })
    }

    pub fn graphics_flight(&self) -> Result<Ref<Flight>, InvalidSlotError> {
        self.resources.flight(self.graphics_flight)
    }

    pub fn add_resource(&self, resource: Resource) {
        self.access_queue
            .0
            .send(NewResource::Add(resource))
            .unwrap()
    }

    pub fn remove_resource(&self, resource: Resource) {
        self.access_queue
            .0
            .send(NewResource::Remove(resource))
            .unwrap()
    }

    pub fn transfer_flight(&self) -> Result<Ref<Flight>, InvalidSlotError> {
        self.resources.flight(self.transfer_flight)
    }

    pub fn get_pipeline(&self, material: &GpuMaterial) -> Option<Arc<GraphicsPipeline>> {
        self.pipeline_cache.lock().get(material).cloned()
    }
}

pub fn topology_to_vulkan(topology: Topology) -> PrimitiveTopology {
    match topology {
        Topology::TriangleList => PrimitiveTopology::TriangleList,
        Topology::TriangleStrip => PrimitiveTopology::TriangleStrip,
        Topology::LineList => PrimitiveTopology::LineList,
        Topology::LineStrip => PrimitiveTopology::LineStrip,
        Topology::PointList => PrimitiveTopology::PointList,
    }
}
