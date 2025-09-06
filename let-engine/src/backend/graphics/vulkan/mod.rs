mod instance;
use anyhow::Result;
use concurrent_slotmap::{SlotId, SlotMap};
use crossbeam::channel::{Receiver, Sender};
use foldhash::HashMap;
pub use instance::Queues;
use let_engine_core::resources::material::Topology;
use parking_lot::Mutex;
use vulkano_taskgraph::{
    Id, InvalidSlotError, Ref,
    resource::{AccessTypes, Flight, Resources},
};
use winit::event_loop::EventLoop;
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
        GraphicsPipeline,
        cache::{PipelineCache, PipelineCacheCreateInfo},
        graphics::input_assembly::PrimitiveTopology,
    },
};

use std::sync::{Arc, OnceLock};

use super::{
    DefaultGraphicsBackendError, Graphics,
    buffer::GpuBuffer,
    material::{GpuMaterial, MaterialId},
    model::GpuModel,
    texture::{GpuTexture, TextureId},
};

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
    pub pipeline_cache: Mutex<HashMap<MaterialId, Arc<GraphicsPipeline>>>,

    pub materials: SlotMap<MaterialId, GpuMaterial>,
    pub buffers: SlotMap<SlotId, GpuBuffer<u8>>,
    pub models: SlotMap<SlotId, GpuModel<u8>>,
    pub textures: SlotMap<TextureId, GpuTexture>,
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
        event_loop: &EventLoop<()>,
        settings: &Graphics,
    ) -> Result<Self, DefaultGraphicsBackendError> {
        let instance = instance::create_instance(event_loop, settings.window_handle_retries)?;

        #[cfg(feature = "vulkan_debug")]
        std::mem::forget(debug::make_debug(&instance).unwrap());

        let features = DeviceFeatures {
            fill_mode_non_solid: true,
            wide_lines: true,
            ..DeviceFeatures::empty()
        };
        let (device, queues) =
            instance::create_device_and_queues(&instance, &features, event_loop)?;

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

            materials: SlotMap::with_key(255),
            buffers: SlotMap::new(255),
            models: SlotMap::new(255),
            textures: SlotMap::with_key(255),
        })
    }

    pub fn graphics_flight(&self) -> Result<Ref<'_, Flight>, InvalidSlotError> {
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

    pub fn transfer_flight(&self) -> Result<Ref<'_, Flight>, InvalidSlotError> {
        self.resources.flight(self.transfer_flight)
    }

    pub fn get_pipeline(&self, material: MaterialId) -> Option<Arc<GraphicsPipeline>> {
        self.pipeline_cache.lock().get(&material).cloned()
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
