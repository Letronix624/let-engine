mod instance;
pub mod shaders;
use foldhash::{HashMap, HashSet};
pub use instance::Queues;
use let_engine_core::resources::material::Topology;
use parking_lot::Mutex;
use vulkano_taskgraph::{
    resource::{Flight, Resources},
    Id,
};
use winit::raw_window_handle::HasDisplayHandle;
#[cfg(feature = "vulkan_debug")]
mod debug;
pub mod swapchain;

use vulkano::{
    descriptor_set::allocator::{
        StandardDescriptorSetAllocator, StandardDescriptorSetAllocatorCreateInfo,
    },
    device::{Device, DeviceFeatures},
    pipeline::{
        cache::{PipelineCache, PipelineCacheCreateInfo},
        graphics::{
            color_blend::{AttachmentBlend, ColorBlendAttachmentState, ColorBlendState},
            input_assembly::{InputAssemblyState, PrimitiveTopology},
            multisample::MultisampleState,
            rasterization::RasterizationState,
            viewport::ViewportState,
            GraphicsPipelineCreateInfo,
        },
        layout::PipelineDescriptorSetLayoutCreateInfo,
        DynamicState, GraphicsPipeline, PipelineLayout, PipelineShaderStageCreateInfo,
    },
};

use std::sync::{Arc, OnceLock};

use super::{material::GpuMaterial, DefaultGraphicsBackendError, Graphics, VulkanError};

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

    pub vulkan_pipeline_cache: Arc<PipelineCache>,
    pub pipeline_cache: Mutex<HashMap<GpuMaterial, Arc<GraphicsPipeline>>>,
}

impl Vulkan {
    pub fn init(
        handle: &impl HasDisplayHandle,
        settings: &Graphics,
    ) -> Result<Self, DefaultGraphicsBackendError> {
        let instance = instance::create_instance(handle, settings.window_handle_retries)?;

        #[cfg(feature = "vulkan_debug")]
        std::mem::forget(debug::make_debug(&instance)?);

        let features = DeviceFeatures {
            fill_mode_non_solid: true,
            wide_lines: true,
            ..DeviceFeatures::empty()
        };
        let (device, queues) = instance::create_device_and_queues(&instance, features, handle)?;

        let descriptor_set_allocator = StandardDescriptorSetAllocator::new(
            device.clone(),
            StandardDescriptorSetAllocatorCreateInfo::default(),
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

        let vulkan_pipeline_cache = unsafe {
            PipelineCache::new(device.clone(), PipelineCacheCreateInfo::default())
                .map_err(|e| DefaultGraphicsBackendError::Vulkan(e.unwrap().into()))?
        };

        let pipeline_cache = Mutex::new(HashMap::default());

        Ok(Self {
            instance,
            device,
            queues,

            descriptor_set_allocator,

            resources,
            graphics_flight,
            transfer_flight,

            vulkan_pipeline_cache,
            pipeline_cache,
        })
    }

    fn cache_pipeline(&self, material: &GpuMaterial) -> Result<Arc<GraphicsPipeline>, VulkanError> {
        let shaders = material.graphics_shaders();
        let settings = material.settings();

        let mut stages = vec![PipelineShaderStageCreateInfo::new(shaders.vertex.clone())];

        if let Some(fragment) = shaders.fragment.as_ref() {
            stages.push(PipelineShaderStageCreateInfo::new(fragment.clone()));
        };

        let layout_create_info = PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
            .into_pipeline_layout_create_info(self.device.clone())
            .map_err(|e| VulkanError::from(e.error.unwrap()))?;

        let layout = PipelineLayout::new(self.device.clone(), layout_create_info)
            .map_err(|e| VulkanError::from(e.unwrap()))?;

        todo!();
        // let subpass = ;

        // let pipeline = GraphicsPipeline::new(
        //     self.device.clone(),
        //     Some(self.vulkan_pipeline_cache.clone()),
        //     GraphicsPipelineCreateInfo {
        //         stages: stages.into_iter().collect(),
        //         vertex_input_state: Some(material.vertex_input_state.clone()),
        //         input_assembly_state: Some(InputAssemblyState {
        //             topology: topology_to_vulkan(settings.topology),
        //             primitive_restart_enable: settings.primitive_restart,
        //             ..Default::default()
        //         }),
        //         viewport_state: Some(ViewportState::default()),
        //         rasterization_state: Some(RasterizationState {
        //             line_width: settings.line_width,
        //             ..Default::default()
        //         }),
        //         multisample_state: Some(MultisampleState::default()),
        //         color_blend_state: Some(ColorBlendState::with_attachment_states(
        //             subpass.num_color_attachments(),
        //             ColorBlendAttachmentState {
        //                 blend: Some(AttachmentBlend::alpha()),
        //                 ..Default::default()
        //             },
        //         )),
        //         subpass: Some(subpass.into()),
        //         dynamic_state: HashSet::from_iter([DynamicState::Viewport]),
        //         ..GraphicsPipelineCreateInfo::new(layout)
        //     },
        // )
        // .map_err(|e| VulkanError::from(e.unwrap()))?;

        // let mut cache = self.pipeline_cache.lock();

        // cache.insert(material.clone(), pipeline.clone());

        // Ok(pipeline)
    }

    pub fn get_pipeline(&self, material: &GpuMaterial) -> Option<Arc<GraphicsPipeline>> {
        self.pipeline_cache.lock().get(material).cloned()
    }

    pub fn get_or_init_pipeline(
        &self,
        material: &GpuMaterial,
    ) -> Result<Arc<GraphicsPipeline>, VulkanError> {
        if let Some(pipeline) = self.get_pipeline(material) {
            Ok(pipeline)
        } else {
            self.cache_pipeline(material)
        }
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
