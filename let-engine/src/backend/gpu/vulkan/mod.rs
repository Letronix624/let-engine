mod instance;
use anyhow::{Result, anyhow};
use concurrent_slotmap::{
    Key, SlotId, SlotMap,
    hyaline::{CollectorHandle, Guard},
};
use foldhash::HashMap;
pub use instance::Queues;
use let_engine_core::resources::{data::Data, material::Topology, model::Vertex};
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

use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, AtomicU64},
};

use crate::backend::gpu::{buffer::BufferId, model::ModelId, texture::TextureId};

use super::{
    DefaultGpuBackendError, GpuSettings,
    buffer::GpuBuffer,
    material::{GpuMaterial, MaterialId},
    model::GpuModel,
    texture::GpuTexture,
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

    resources_dirty: AtomicBool,

    pub vulkan_pipeline_cache: Arc<PipelineCache>,
    pub pipeline_cache: Mutex<HashMap<MaterialId, Arc<GraphicsPipeline>>>,

    pub collector: CollectorHandle,
    resource_map: SlotMap<SlotId, Resource>,
    virtual_ids: SlotMap<SlotId, AtomicSlotId>,
}

pub const VIRTUAL_TAG_BIT: u32 = 1 << 7;

pub enum Resource {
    Material(GpuMaterial),
    Buffer(GpuBuffer<u8>),
    Model(GpuModel<u8>),
    Texture(GpuTexture),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ResourceAccess {
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
        settings: GpuSettings,
    ) -> Result<Self, DefaultGpuBackendError> {
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
            .map_err(|e| DefaultGpuBackendError::Vulkan(e.unwrap().into()))?;

        let graphics_flight = resources
            .create_flight(settings.max_frames_in_flight as u32)
            .map_err(|e| DefaultGpuBackendError::Vulkan(e.into()))?;

        let transfer_flight = resources
            .create_flight(1)
            .map_err(|e| DefaultGpuBackendError::Vulkan(e.into()))?;

        let vulkan_pipeline_cache =
            PipelineCache::new(&device, &PipelineCacheCreateInfo::default())
                .map_err(|e| DefaultGpuBackendError::Vulkan(e.unwrap().into()))?;

        let pipeline_cache = Mutex::new(HashMap::default());

        let collector = CollectorHandle::new();
        // TODO: settings for capacity
        let resource_map = unsafe { SlotMap::with_collector(1024, collector.clone()) };
        let virtual_ids = unsafe { SlotMap::with_collector(1024, collector.clone()) };

        Ok(Self {
            instance,
            device,
            queues,

            descriptor_set_allocator,

            resources,
            resources_dirty: false.into(),
            graphics_flight,
            transfer_flight,

            vulkan_pipeline_cache,
            pipeline_cache,

            collector,
            resource_map,
            virtual_ids,
        })
    }

    pub fn graphics_flight(&self) -> Result<Ref<'_, Flight>, InvalidSlotError> {
        self.resources.flight(self.graphics_flight)
    }

    /// Call when resources are modified
    pub fn flag_taskgraph_to_be_rebuilt(&self) {
        self.resources_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Call each frame to check if task graph has to be rebuilt
    pub fn clean_resources(&self) -> bool {
        self.resources_dirty
            .swap(false, std::sync::atomic::Ordering::Relaxed)
    }

    /// Wait for transfer operations to complete
    pub fn wait_transfer(&self) {
        self.resources
            .flight(self.transfer_flight)
            .unwrap()
            .wait_idle()
            .unwrap()
    }

    pub fn get_pipeline(&self, material: MaterialId) -> Option<Arc<GraphicsPipeline>> {
        self.pipeline_cache.lock().get(&material).cloned()
    }
}

impl Vulkan {
    pub fn material<'a>(&'a self, id: MaterialId, guard: &'a Guard<'_>) -> Option<&'a GpuMaterial> {
        if let Some(Resource::Material(material)) = self.resource(id.as_id(), guard) {
            Some(material)
        } else {
            None
        }
    }

    pub fn buffer<'a, B: Data>(
        &'a self,
        id: BufferId<B>,
        guard: &'a Guard<'_>,
    ) -> Option<&'a GpuBuffer<B>> {
        if let Some(Resource::Buffer(buffer)) = self.resource(id.as_id(), guard) {
            // SAFETY: transmute is safe here, because the generic is not present in the byte representation and drop logic.
            Some(unsafe { std::mem::transmute::<&GpuBuffer<u8>, &GpuBuffer<B>>(buffer) })
        } else {
            None
        }
    }

    pub fn model<'a, V: Vertex>(
        &'a self,
        id: ModelId<V>,
        guard: &'a Guard<'_>,
    ) -> Option<&'a GpuModel<V>> {
        if let Some(Resource::Model(model)) = self.resource(id.as_id(), guard) {
            // SAFETY: transmute is safe here, because the generic is not present in the byte representation and drop logic.
            //         The vertex type might mismatch to the original format, but this is only possible if the user used unsafe
            //         logic to reinterpret the vertex type of an ID to a non-compatible type, which is totally on them.
            Some(unsafe { std::mem::transmute::<&GpuModel<u8>, &GpuModel<V>>(model) })
        } else {
            None
        }
    }

    pub fn texture<'a>(&'a self, id: TextureId, guard: &'a Guard<'_>) -> Option<&'a GpuTexture> {
        if let Some(Resource::Texture(texture)) = self.resource(id.as_id(), guard) {
            Some(texture)
        } else {
            None
        }
    }

    pub fn add_material(&self, material: GpuMaterial, guard: &Guard<'_>) -> MaterialId {
        MaterialId::from_id(self.resource_map.insert_with_tag(
            Resource::Material(material),
            MaterialId::TAG_BIT,
            guard,
        ))
    }

    pub fn add_buffer<B: Data>(&self, buffer: GpuBuffer<B>, guard: &Guard<'_>) -> BufferId<B> {
        BufferId::from_id(self.resource_map.insert_with_tag(
            Resource::Buffer(unsafe { std::mem::transmute::<GpuBuffer<B>, GpuBuffer<u8>>(buffer) }),
            BufferId::<B>::TAG_BIT,
            guard,
        ))
    }

    pub fn add_model<V: Vertex>(&self, model: GpuModel<V>, guard: &Guard<'_>) -> ModelId<V> {
        ModelId::from_id(self.resource_map.insert_with_tag(
            Resource::Model(unsafe { std::mem::transmute::<GpuModel<V>, GpuModel<u8>>(model) }),
            ModelId::<V>::TAG_BIT,
            guard,
        ))
    }

    pub fn add_texture(&self, texture: GpuTexture, guard: &Guard<'_>) -> TextureId {
        TextureId::from_id(self.resource_map.insert_with_tag(
            Resource::Texture(texture),
            TextureId::TAG_BIT,
            guard,
        ))
    }

    fn resource<'a>(&'a self, id: SlotId, guard: &'a Guard<'_>) -> Option<&'a Resource> {
        if id.tag() & VIRTUAL_TAG_BIT != 0 {
            if let Some(id) = self.virtual_ids.get(id, guard) {
                self.resource(id.load(), guard)
            } else {
                None
            }
        } else {
            self.resource_map.get(id, guard)
        }
    }

    pub fn add_virtual_id(&self, id: SlotId, guard: &Guard<'_>) -> Result<SlotId> {
        if (id.tag() & VIRTUAL_TAG_BIT != 0 && self.virtual_ids.get(id, guard).is_some())
            || self.resource_map.get(id, guard).is_some()
        {
            Ok(self
                .virtual_ids
                .insert_with_tag(id.into(), VIRTUAL_TAG_BIT, guard))
        } else {
            Err(anyhow!("Target ID is invalid."))
        }
    }

    // Both ID types have to be the same. Else a virtual ID can point to the wrong resource.
    pub fn remap_virtual_id(&self, from: SlotId, to: SlotId, guard: &Guard<'_>) -> Result<()> {
        // Check for circular dependency
        {
            let mut ids = vec![from, to];
            let mut current_id = to;
            while current_id.tag() & VIRTUAL_TAG_BIT != 0
                && let Some(id) = self.virtual_ids.get(current_id, guard)
            {
                let id = id.load();
                if ids.contains(&id) {
                    return Err(anyhow!(
                        "Cannot remap virtual ID to have circular dependencies."
                    ));
                }
                ids.push(id);
                current_id = id;
            }
        }

        let Some(id) = self.virtual_ids.get(from, guard) else {
            return Err(anyhow!("Target ID does not exist"));
        };

        id.store(to);

        Ok(())
    }

    pub fn iter_resource_access(&self, guard: &Guard<'_>) -> Vec<ResourceAccess> {
        self.resource_map
            .iter(guard)
            .flat_map(|(_, resource)| match resource {
                Resource::Buffer(buffer) => buffer.resources(),
                Resource::Model(model) => model.resources(),
                Resource::Texture(texture) => texture.resources(),
                _ => vec![],
            })
            .collect()
    }

    pub fn remove_resource(&self, id: SlotId, guard: &Guard<'_>) {
        if id.tag() & VIRTUAL_TAG_BIT != 0 {
            self.virtual_ids.remove(id, guard);
        } else {
            self.resource_map.remove(id, guard);
        }
    }
}

struct AtomicSlotId(AtomicU64);

impl From<SlotId> for AtomicSlotId {
    fn from(id: SlotId) -> Self {
        Self(AtomicU64::new(
            u64::from(id.index()) | (u64::from(id.generation()) << 32),
        ))
    }
}

impl AtomicSlotId {
    fn store(&self, id: SlotId) {
        self.0.store(
            u64::from(id.index()) | (u64::from(id.generation()) << 32),
            std::sync::atomic::Ordering::Relaxed,
        );
    }

    fn load(&self) -> SlotId {
        let id = self.0.load(std::sync::atomic::Ordering::Relaxed);
        SlotId::new(id as u32, (id >> 32) as u32)
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
