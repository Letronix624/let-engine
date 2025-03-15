mod instance;
pub mod shaders;
use foldhash::{HashMap, HashSet};
pub use instance::Queues;
use let_engine_core::resources::material::Topology;
use parking_lot::Mutex;
use winit::raw_window_handle::HasDisplayHandle;
#[cfg(feature = "vulkan_debug")]
mod debug;
pub mod swapchain;

use anyhow::{anyhow, Context, Result};
use vulkano::{
    command_buffer::allocator::{
        StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo,
    },
    descriptor_set::allocator::{
        StandardDescriptorSetAllocator, StandardDescriptorSetAllocatorCreateInfo,
    },
    device::{Device, DeviceFeatures},
    image::{view::ImageView, Image},
    memory::allocator::StandardMemoryAllocator,
    pipeline::{
        cache::{PipelineCache, PipelineCacheCreateInfo},
        graphics::{
            color_blend::{AttachmentBlend, ColorBlendAttachmentState, ColorBlendState},
            input_assembly::{InputAssemblyState, PrimitiveTopology},
            multisample::MultisampleState,
            rasterization::RasterizationState,
            viewport::{Viewport, ViewportState},
            GraphicsPipelineCreateInfo,
        },
        layout::PipelineDescriptorSetLayoutCreateInfo,
        DynamicState, GraphicsPipeline, PipelineLayout, PipelineShaderStageCreateInfo,
    },
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
    sync::GpuFuture,
};

use std::sync::{Arc, OnceLock};

use super::material::GpuMaterial;

/// Just a holder of general immutable information about Vulkan.
#[derive(Clone)]
pub struct Vulkan {
    pub instance: Arc<vulkano::instance::Instance>,
    pub device: Arc<Device>,
    pub queues: Arc<Queues>,

    pub memory_allocator: Arc<StandardMemoryAllocator>,
    pub descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    pub command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    pub subpass: OnceLock<Subpass>,
    pub vulkan_pipeline_cache: Arc<PipelineCache>,
    pub pipeline_cache: Arc<Mutex<HashMap<GpuMaterial, Arc<GraphicsPipeline>>>>,

    pub future: Arc<Mutex<Option<Box<dyn GpuFuture + Send>>>>,
}

impl Vulkan {
    pub fn init(handle: &impl HasDisplayHandle) -> Result<Self> {
        let instance = instance::create_instance(handle)?;

        #[cfg(feature = "vulkan_debug")]
        std::mem::forget(debug::make_debug(&instance)?);

        let features = DeviceFeatures {
            fill_mode_non_solid: true,
            wide_lines: true,
            ..DeviceFeatures::empty()
        };
        let (device, queues) = instance::create_device_and_queues(&instance, features, handle)?;

        let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));

        let descriptor_set_allocator = StandardDescriptorSetAllocator::new(
            device.clone(),
            StandardDescriptorSetAllocatorCreateInfo::default(),
        )
        .into();

        let command_buffer_allocator = StandardCommandBufferAllocator::new(
            device.clone(),
            StandardCommandBufferAllocatorCreateInfo {
                secondary_buffer_count: 2,
                ..Default::default()
            },
        )
        .into();

        let subpass = OnceLock::new();

        let vulkan_pipeline_cache =
            unsafe { PipelineCache::new(device.clone(), PipelineCacheCreateInfo::default())? };

        let pipeline_cache = Arc::new(Mutex::new(HashMap::default()));

        // //Materials
        // let vs = vertex_shader(device.clone())?;
        // let fs = fragment_shader(device.clone())?;
        // let default_shaders = Shaders::from_modules(vs.clone(), fs.clone(), "main");

        // let tfs = textured_fragment_shader(device.clone())?;
        // let default_textured_shaders = Shaders::from_modules(vs.clone(), tfs.clone(), "main");

        // let tafs = texture_array_fragment_shader(device.clone())?;
        // let default_texture_array_shaders = Shaders::from_modules(vs.clone(), tafs.clone(), "main");

        // let instance_vert = instanced_vertex_shader(device.clone())?;
        // let instance_frag = instanced_fragment_shader(device.clone())?;
        // let default_instance_shaders =
        //     Shaders::from_modules(instance_vert.clone(), instance_frag.clone(), "main");

        // let textured_instance_frag = instanced_textured_fragment_shader(device.clone())?;
        // let default_textured_instance_shaders = Shaders::from_modules(
        //     instance_vert.clone(),
        //     textured_instance_frag.clone(),
        //     "main",
        // );

        // let texture_array_instance_frag = instanced_texture_array_fragment_shader(device.clone())?;
        // let default_texture_array_instance_shaders = Shaders::from_modules(
        //     instance_vert.clone(),
        //     texture_array_instance_frag.clone(),
        //     "main",
        // );

        // let vertex_buffer_description = [GameVertex::per_vertex(), InstanceData::per_instance()];

        // let mut pipelines = vec![];

        // let rasterisation_state = RasterizationState::default();

        // let vertex = vs
        //     .entry_point("main")
        //     .expect("Main function of default vertex shader has no main function.");
        // let fragment = fs
        //     .entry_point("main")
        //     .expect("Main function of default fragment shader has no main function.");

        // let pipeline: Arc<GraphicsPipeline> = pipeline::create_pipeline(
        //     &device,
        //     vertex.clone(),
        //     fragment,
        //     InputAssemblyState::default(),
        //     subpass.clone(),
        //     vertex_buffer_description[0].definition(&vertex)?,
        //     rasterisation_state.clone(),
        //     None,
        // )?;
        // pipelines.push(pipeline.clone());

        // let textured_fragment = tfs
        //     .entry_point("main")
        //     .expect("Main function not found in default textured fragment shader.");
        // let textured_pipeline = pipeline::create_pipeline(
        //     &device,
        //     vertex.clone(),
        //     textured_fragment,
        //     InputAssemblyState::default(),
        //     subpass.clone(),
        //     vertex_buffer_description[0].definition(&vertex)?,
        //     rasterisation_state.clone(),
        //     None,
        // )?;
        // pipelines.push(textured_pipeline.clone());

        // let texture_array_fragment = tafs
        //     .entry_point("main")
        //     .expect("Main function not found in default texture array shader.");
        // let texture_array_pipeline = pipeline::create_pipeline(
        //     &device,
        //     vertex.clone(),
        //     texture_array_fragment,
        //     InputAssemblyState::default(),
        //     subpass.clone(),
        //     vertex_buffer_description[0].definition(&vertex)?,
        //     rasterisation_state.clone(),
        //     None,
        // )?;
        // pipelines.push(texture_array_pipeline.clone());

        // let instance_vertex = instance_vert
        //     .entry_point("main")
        //     .expect("Main function not found in default instanced vertex shader.");
        // let instance_fragment = instance_frag
        //     .entry_point("main")
        //     .expect("Main function not found in default instanced fragment shader.");
        // let instance_pipeline = pipeline::create_pipeline(
        //     &device,
        //     instance_vertex.clone(),
        //     instance_fragment,
        //     InputAssemblyState::default(),
        //     subpass.clone(),
        //     vertex_buffer_description.definition(&instance_vertex)?,
        //     rasterisation_state.clone(),
        //     None,
        // )?;
        // pipelines.push(instance_pipeline.clone());

        // let textured_instance_fragment = textured_instance_frag
        //     .entry_point("main")
        //     .expect("Main function not found in default textured instanced fragment shader.");
        // let textured_instance_pipeline = pipeline::create_pipeline(
        //     &device,
        //     instance_vertex.clone(),
        //     textured_instance_fragment,
        //     InputAssemblyState::default(),
        //     subpass.clone(),
        //     vertex_buffer_description.definition(&instance_vertex)?,
        //     rasterisation_state.clone(),
        //     None,
        // )?;
        // pipelines.push(textured_instance_pipeline.clone());

        // let texture_array_instance_fragment = texture_array_instance_frag
        //     .entry_point("main")
        //     .expect("Main function not found in default texture array instance fragment shader.");
        // let texture_array_instance_pipeline = pipeline::create_pipeline(
        //     &device,
        //     instance_vertex.clone(),
        //     texture_array_instance_fragment,
        //     InputAssemblyState::default(),
        //     subpass.clone(),
        //     vertex_buffer_description.definition(&instance_vertex)?,
        //     rasterisation_state,
        //     None,
        // )?;
        // pipelines.push(texture_array_instance_pipeline.clone());

        // let default_material = Material::from_pipeline(&pipeline, false, default_shaders.clone());
        // let textured_material =
        //     Material::from_pipeline(&textured_pipeline, false, default_textured_shaders.clone());
        // let texture_array_material = Material::from_pipeline(
        //     &texture_array_pipeline,
        //     false,
        //     default_texture_array_shaders.clone(),
        // );
        // let default_instance_material =
        //     Material::from_pipeline(&instance_pipeline, true, default_instance_shaders.clone());

        // let textured_instance_material = Material::from_pipeline(
        //     &textured_instance_pipeline,
        //     true,
        //     default_textured_instance_shaders.clone(),
        // );

        // let texture_array_instance_material = Material::from_pipeline(
        //     &texture_array_instance_pipeline,
        //     true,
        //     default_texture_array_instance_shaders.clone(),
        // );

        Ok(Self {
            instance,
            device,
            queues,

            memory_allocator,
            descriptor_set_allocator,
            command_buffer_allocator,
            subpass,
            vulkan_pipeline_cache,
            pipeline_cache,
            future: Arc::new(Mutex::new(None)),
        })
    }

    pub fn subpass(&self) -> Result<&Subpass> {
        self.subpass.get().ok_or(anyhow!("Subpass not initialized"))
    }

    fn cache_pipeline(&self, material: &GpuMaterial) -> Result<Arc<GraphicsPipeline>> {
        let shaders = material.graphics_shaders();
        let settings = material.settings();

        let mut stages = vec![PipelineShaderStageCreateInfo::new(shaders.vertex.clone())];

        if let Some(fragment) = shaders.fragment.as_ref() {
            stages.push(PipelineShaderStageCreateInfo::new(fragment.clone()));
        };

        let layout = PipelineLayout::new(
            self.device.clone(),
            PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
                .into_pipeline_layout_create_info(self.device.clone())?,
        )?;

        let subpass = self.subpass()?.clone();

        let pipeline = GraphicsPipeline::new(
            self.device.clone(),
            Some(self.vulkan_pipeline_cache.clone()),
            GraphicsPipelineCreateInfo {
                stages: stages.into_iter().collect(),
                vertex_input_state: Some(material.vertex_input_state.clone()),
                input_assembly_state: Some(InputAssemblyState {
                    topology: topology_to_vulkan(settings.topology),
                    primitive_restart_enable: settings.primitive_restart,
                    ..Default::default()
                }),
                viewport_state: Some(ViewportState::default()),
                rasterization_state: Some(RasterizationState {
                    line_width: settings.line_width,
                    ..Default::default()
                }),
                multisample_state: Some(MultisampleState::default()),
                color_blend_state: Some(ColorBlendState::with_attachment_states(
                    subpass.num_color_attachments(),
                    ColorBlendAttachmentState {
                        blend: Some(AttachmentBlend::alpha()),
                        ..Default::default()
                    },
                )),
                subpass: Some(subpass.into()),
                dynamic_state: HashSet::from_iter([DynamicState::Viewport]),
                ..GraphicsPipelineCreateInfo::layout(layout)
            },
        )?;

        let mut cache = self.pipeline_cache.lock();

        cache.insert(material.clone(), pipeline.clone());

        Ok(pipeline)
    }

    pub fn get_pipeline(&self, material: &GpuMaterial) -> Option<Arc<GraphicsPipeline>> {
        self.pipeline_cache.lock().get(material).cloned()
    }

    pub fn get_or_init_pipeline(&self, material: &GpuMaterial) -> Result<Arc<GraphicsPipeline>> {
        if let Some(pipeline) = self.get_pipeline(material) {
            Ok(pipeline)
        } else {
            self.cache_pipeline(material)
        }
    }
}

/// Sets the dynamic viewport up to work with the newly set resolution of the window.
//  For games make the viewport less dynamic.
pub fn window_size_dependent_setup(
    images: &[Arc<Image>],
    render_pass: Arc<RenderPass>,
    viewport: &mut Viewport,
) -> Result<Vec<Arc<Framebuffer>>> {
    let extent = images[0].extent();
    viewport.extent = [extent[0] as f32, extent[1] as f32];

    let framebuffers: Vec<Arc<Framebuffer>> = images
        .iter()
        .map(|image| {
            let view = ImageView::new_default(image.clone())
                .context("Could not make a frame texture.")
                .unwrap();
            Framebuffer::new(
                render_pass.clone(),
                FramebufferCreateInfo {
                    attachments: vec![view],
                    ..Default::default()
                },
            )
            .context("Could not make a framebuffer to present to the window.")
            .unwrap()
        })
        .collect();

    Ok(framebuffers)
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
