use anyhow::Result;
use crossbeam::channel::Receiver;
use foldhash::HashSet;
use std::{any::Any, collections::BTreeMap, num::NonZero, sync::Arc};
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    descriptor_set::{sys::RawDescriptorSet, DescriptorSet, WriteDescriptorSet},
    format::Format,
    image::sampler::Sampler,
    memory::{
        allocator::{
            suballocator::Region, AllocationCreateInfo, BumpAllocator, DeviceLayout,
            MemoryTypeFilter, Suballocator,
        },
        DeviceAlignment,
    },
    pipeline::{
        graphics::{
            color_blend::{AttachmentBlend, ColorBlendAttachmentState, ColorBlendState},
            input_assembly::InputAssemblyState,
            multisample::MultisampleState,
            rasterization::RasterizationState,
            viewport::{Viewport, ViewportState},
            GraphicsPipelineCreateInfo,
        },
        DynamicState, GraphicsPipeline, Pipeline, PipelineLayout, PipelineShaderStageCreateInfo,
    },
    swapchain::{Surface, Swapchain, SwapchainCreateInfo},
    DeviceSize, Validated,
};
use vulkano_taskgraph::{
    graph::{AttachmentInfo, CompileInfo, ExecutableTaskGraph, NodeId, TaskGraph},
    resource::AccessTypes,
    resource_map, Id, Task,
};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use let_engine_core::objects::{
    scenes::{LayerView, Scene},
    Descriptor, Node, VisualObject,
};

use glam::{f32::Mat4, UVec2};

use crate::backend::graphics::vulkan::NewResource;

use super::{
    material::GpuMaterial,
    vulkan::{swapchain::create_swapchain, topology_to_vulkan, Resource, Vulkan, VK},
    Graphics, GraphicsInterface, VulkanError, VulkanTypes,
};

/// Responsible for drawing on the surface.
pub struct Draw {
    swapchain_id: Id<Swapchain>,
    virtual_swapchain_id: Id<Swapchain>,

    recreate_swapchain: bool,

    settings_channel: Receiver<Graphics>,
    settings: Graphics,
    dimensions: UVec2,
    image_format: Format,

    task_graph: ExecutableTaskGraph<Self>,
    draw_node_id: NodeId,
    resource_accesses: HashSet<Resource>,

    viewport: Viewport,

    view_proj_region: Region,
    view_proj_alignment: DeviceAlignment,
    view_proj_buffer_id: Id<Buffer>,

    scene: Arc<Scene<VulkanTypes>>,
}

impl Draw {
    pub fn new(
        interface: GraphicsInterface,
        window: &Arc<impl HasWindowHandle + HasDisplayHandle + Any + Send + Sync>,
        scene: Arc<Scene<VulkanTypes>>,
    ) -> Result<Self> {
        let vulkan = VK.get().unwrap();

        let surface = Surface::from_window(&vulkan.instance, window)?;

        let (swapchain_id, dimensions, image_format) =
            create_swapchain(&vulkan.device, surface, &interface, vulkan)?;

        let recreate_swapchain = false;

        let viewport = Viewport {
            offset: [0.0; 2],
            extent: dimensions.as_vec2().into(),
            min_depth: 0.0,
            max_depth: 1.0,
        };

        let drawing_queue = vulkan.queues.general().clone();

        let settings = interface.settings();
        let settings_channel = interface.settings_channels.1.clone();

        let (view_proj_region, view_proj_alignment, view_proj_buffer_id) =
            Self::create_view_proj_buffer(vulkan);

        let mut task_graph = TaskGraph::new(&vulkan.resources);

        let virtual_swapchain_id = task_graph.add_swapchain(&SwapchainCreateInfo {
            image_format,
            ..Default::default()
        });
        let framebuffer_id = task_graph.add_framebuffer();

        task_graph.add_host_buffer_access(
            view_proj_buffer_id,
            vulkano_taskgraph::resource::HostAccessType::Write,
        );

        let draw_node_id = task_graph
            .create_task_node(
                "root draw",
                vulkano_taskgraph::QueueFamilyType::Graphics,
                DrawTask {
                    swapchain_id: virtual_swapchain_id,
                    settings: interface.settings(),
                },
            )
            .framebuffer(framebuffer_id)
            .color_attachment(
                virtual_swapchain_id.current_image_id(),
                AccessTypes::COLOR_ATTACHMENT_WRITE,
                vulkano_taskgraph::resource::ImageLayoutType::Optimal,
                &AttachmentInfo {
                    clear: true,
                    ..Default::default()
                },
            )
            .buffer_access(view_proj_buffer_id, AccessTypes::VERTEX_SHADER_UNIFORM_READ)
            .build();

        let task_graph = unsafe {
            task_graph.compile(&CompileInfo {
                queues: &[&drawing_queue],
                present_queue: Some(&drawing_queue),
                flight_id: vulkan.graphics_flight,
                ..Default::default()
            })
        }
        .unwrap();

        Ok(Self {
            swapchain_id,
            virtual_swapchain_id,
            recreate_swapchain,
            image_format,
            dimensions,
            viewport,
            task_graph,
            draw_node_id,
            resource_accesses: HashSet::default(),
            settings,
            settings_channel,
            view_proj_region,
            view_proj_alignment,
            view_proj_buffer_id,
            scene,
        })
    }

    fn recompile_task_graph(&mut self, vulkan: &Vulkan) -> Result<()> {
        let mut task_graph = TaskGraph::new(&vulkan.resources);

        let virtual_swapchain_id = task_graph.add_swapchain(&SwapchainCreateInfo {
            image_format: self.image_format,
            ..Default::default()
        });
        let framebuffer_id = task_graph.add_framebuffer();

        task_graph.add_host_buffer_access(
            self.view_proj_buffer_id,
            vulkano_taskgraph::resource::HostAccessType::Write,
        );

        let mut builder = task_graph.create_task_node(
            "root draw",
            vulkano_taskgraph::QueueFamilyType::Graphics,
            DrawTask {
                swapchain_id: virtual_swapchain_id,
                settings: self.settings,
            },
        );
        builder.framebuffer(framebuffer_id);
        builder.color_attachment(
            virtual_swapchain_id.current_image_id(),
            AccessTypes::COLOR_ATTACHMENT_WRITE,
            vulkano_taskgraph::resource::ImageLayoutType::Optimal,
            &AttachmentInfo {
                clear: true,
                ..Default::default()
            },
        );
        builder.buffer_access(
            self.view_proj_buffer_id,
            AccessTypes::VERTEX_SHADER_UNIFORM_READ,
        );

        for access in self.resource_accesses.iter() {
            match *access {
                Resource::Buffer { id, access_types } => {
                    builder.buffer_access(id, access_types);
                }
                Resource::Image { id, access_types } => {
                    builder.image_access(
                        id,
                        access_types,
                        vulkano_taskgraph::resource::ImageLayoutType::Optimal,
                    );
                }
            }
        }

        self.draw_node_id = builder.build();

        let drawing_queue = vulkan.queues.general();

        self.task_graph = unsafe {
            task_graph.compile(&CompileInfo {
                queues: &[drawing_queue],
                present_queue: Some(drawing_queue),
                flight_id: vulkan.graphics_flight,
                ..Default::default()
            })
        }?;

        Ok(())
    }

    fn create_view_proj_buffer(vulkan: &Vulkan) -> (Region, DeviceAlignment, Id<Buffer>) {
        let alignment = {
            let physical_device = vulkan.device.physical_device();

            // Allow maximum of 256 camera instances

            unsafe {
                DeviceAlignment::new_unchecked(
                    128.max(
                        physical_device
                            .properties()
                            .min_uniform_buffer_offset_alignment
                            .as_devicesize(),
                    ),
                )
            }
        };

        let size = 256 * alignment.as_devicesize();

        let region = unsafe { Region::new_unchecked(0, size) };

        let buffer_id = vulkan
            .resources
            .create_buffer(
                &BufferCreateInfo {
                    usage: BufferUsage::UNIFORM_BUFFER,
                    ..Default::default()
                },
                &AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::HOST_SEQUENTIAL_WRITE
                        | MemoryTypeFilter::PREFER_HOST,
                    allocate_preference:
                        vulkano::memory::allocator::MemoryAllocatePreference::AlwaysAllocate,
                    ..Default::default()
                },
                DeviceLayout::new(unsafe { NonZero::new_unchecked(size) }, alignment).unwrap(),
            )
            .unwrap();

        (region, alignment, buffer_id)
    }

    /// Recreates the swapchain in case it is out of date if someone for example changed the scene size or window dimensions.
    fn recreate_swapchain(&mut self, vulkan: &Vulkan) -> Result<(), VulkanError> {
        if self.recreate_swapchain {
            self.swapchain_id = vulkan
                .resources
                .recreate_swapchain(self.swapchain_id, |create_info| SwapchainCreateInfo {
                    image_extent: self.dimensions.into(),
                    ..*create_info
                })
                .map_err(|e| VulkanError::from(e.unwrap()))?;

            self.recreate_swapchain = false;
        };
        Ok(())
    }

    pub fn resize(&mut self, new_size: UVec2) {
        self.recreate_swapchain = true;
        self.dimensions = new_size;
        self.viewport.extent = new_size.as_vec2().into();
    }

    /// Redraws the scene.
    pub fn redraw_event(&mut self, pre_present_notify: impl FnOnce()) -> Result<(), VulkanError> {
        if let Ok(settings) = self.settings_channel.try_recv() {
            self.settings = settings;
        }

        if self.dimensions.x == 0 || self.dimensions.y == 0 {
            return Ok(());
        }

        let vulkan = VK.get().unwrap();

        if !vulkan.access_queue.1.is_empty() {
            for event in vulkan.access_queue.1.try_iter() {
                match event {
                    NewResource::Add(resource) => {
                        self.resource_accesses.insert(resource);
                    }
                    NewResource::Remove(resource) => {
                        self.resource_accesses.remove(&resource);
                    }
                }
            }
            self.recompile_task_graph(vulkan).unwrap();
        }

        self.recreate_swapchain(vulkan)?;

        let resource_map = resource_map!(
            &self.task_graph,
            self.virtual_swapchain_id => self.swapchain_id,
        )
        .unwrap();

        let flight = vulkan.graphics_flight().unwrap();
        flight.wait(None).unwrap();

        match unsafe {
            self.task_graph
                .execute(resource_map, self, pre_present_notify)
        } {
            Ok(()) => Ok(()),
            Err(vulkano_taskgraph::graph::ExecuteError::Swapchain {
                error: Validated::Error(vulkano::VulkanError::OutOfDate),
                ..
            }) => {
                self.recreate_swapchain = true;
                self.recreate_swapchain(vulkan)
            }
            Err(e) => panic!("{e:?}"),
        }
    }
}

/// Graphics pipeline management methods
impl Draw {
    /// Creates and caches a graphics pipeline according to the given material.
    fn cache_pipeline(
        &self,
        material: &GpuMaterial,
        vulkan: &Vulkan,
    ) -> Result<Arc<GraphicsPipeline>, VulkanError> {
        let shaders = material.graphics_shaders();
        let settings = material.settings();

        let mut stages = vec![PipelineShaderStageCreateInfo::new(&shaders.vertex)];

        if let Some(fragment) = shaders.fragment.as_ref() {
            stages.push(PipelineShaderStageCreateInfo::new(fragment));
        };

        let layout = PipelineLayout::from_stages(&vulkan.device, &stages)
            .map_err(|e| VulkanError::from(e.unwrap()))?;

        let draw_node = self.task_graph.task_node(self.draw_node_id).unwrap();

        let subpass = draw_node.subpass();

        let pipeline = GraphicsPipeline::new(
            &vulkan.device,
            Some(&vulkan.vulkan_pipeline_cache),
            &GraphicsPipelineCreateInfo {
                stages: &stages,
                vertex_input_state: Some(&material.vertex_input_state),
                input_assembly_state: Some(&InputAssemblyState {
                    topology: topology_to_vulkan(settings.topology),
                    primitive_restart_enable: settings.primitive_restart,
                    ..Default::default()
                }),
                viewport_state: Some(&ViewportState::default()),
                rasterization_state: Some(&RasterizationState {
                    line_width: settings.line_width,
                    ..Default::default()
                }),
                multisample_state: Some(&MultisampleState::default()),
                color_blend_state: Some(&ColorBlendState {
                    attachments: &[ColorBlendAttachmentState {
                        blend: Some(AttachmentBlend::alpha()),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                subpass: subpass.map(|x| x.into()),
                dynamic_state: &[DynamicState::Viewport],
                ..GraphicsPipelineCreateInfo::new(&layout)
            },
        )
        .map_err(|e| VulkanError::from(e.unwrap()))?;

        let mut cache = vulkan.pipeline_cache.lock();

        cache.insert(material.clone(), pipeline.clone());

        Ok(pipeline)
    }

    /// Searches a pipeline according to the given material and returns it if found.
    ///
    /// If this material is not found, it will be created and added to the cache.
    pub fn get_or_init_pipeline(
        &self,
        material: &GpuMaterial,
        vulkan: &Vulkan,
    ) -> Result<Arc<GraphicsPipeline>, VulkanError> {
        if let Some(pipeline) = vulkan.get_pipeline(material) {
            Ok(pipeline)
        } else {
            self.cache_pipeline(material, vulkan)
        }
    }
}

struct DrawTask {
    swapchain_id: Id<Swapchain>,
    settings: Graphics,
}

impl Task for DrawTask {
    type World = Draw;

    fn clear_values(
        &self,
        clear_values: &mut vulkano_taskgraph::ClearValues<'_>,
        _world: &Self::World,
    ) {
        clear_values.set(
            self.swapchain_id.current_image_id(),
            self.settings.clear_color.rgba(),
        );
    }

    // TODO: Clean up clean up
    /// Draws the Game Scene on the given command buffer.
    unsafe fn execute(
        &self,
        cbf: &mut vulkano_taskgraph::command_buffer::RecordingCommandBuffer<'_>,
        tcx: &mut vulkano_taskgraph::TaskContext<'_>,
        world: &Self::World,
    ) -> vulkano_taskgraph::TaskResult {
        let vulkan = VK.get().unwrap();

        let views: Vec<Arc<LayerView<VulkanTypes>>> = {
            let view = world.scene.views().lock();
            view.iter().cloned().collect()
        };
        cbf.set_viewport(0, std::slice::from_ref(&world.viewport))?;

        let mut suballocator = BumpAllocator::new(world.view_proj_region);

        let camera_allocations = views.iter().map(|_| {
            let suballocation = suballocator
                .allocate(
                    DeviceLayout::new_sized::<[Mat4; 2]>(),
                    vulkano::memory::allocator::AllocationType::Linear,
                    world.view_proj_alignment,
                )
                .expect("You can not create more than 256 cameras."); // TODO: Resize
            let start = suballocation.offset;
            let size = suballocation.size;

            (start, size)
        });

        /* Iterate All views */

        for (view, (start, range)) in views.iter().zip(camera_allocations) {
            let layer = view.layer();

            // Clear layer views with less references than 3.
            if Arc::strong_count(view) <= 2 {
                world.scene.update();
                continue;
            }
            // Skip disabled layer view
            if !view.draw() {
                continue;
            }
            // Skip if extent contains 0
            let extent = view.extent();
            if extent.x == 0 || extent.y == 0 {
                continue;
            }

            // Write camera matrices into buffer regions
            let write: &mut [Mat4; 2] =
                tcx.write_buffer(world.view_proj_buffer_id, start..start + range)?;

            write[0] = view.camera().make_view_matrix();
            write[1] = view.make_projection_matrix();

            // Order all objects to the right draw order.
            let mut order: Vec<VisualObject<VulkanTypes>> =
                Vec::with_capacity(layer.number_of_objects());

            for object in layer.children().lock().iter() {
                let object = object.lock();
                order.push(VisualObject {
                    transform: object.object.transform,
                    appearance: object.object.appearance.clone(),
                });
                Node::order_position(&mut order, &object);
            }

            /* Draw Objects */

            for object in order {
                let appearance = &object.appearance;

                // Skip objects marked as invisible.
                if !*appearance.get_visible() {
                    continue;
                };

                let material = appearance.get_material();

                /* Default MVP Matrix Creation */

                let model_matrix: Mat4 = object.make_model_matrix();

                /* TEMP Descriptor Creation TEMP */

                let Ok(graphics_pipeline) = world.get_or_init_pipeline(material, vulkan) else {
                    log::error!("Failed to create pipeline for object.");
                    continue;
                };

                let descriptor_map = appearance.descriptors();

                let mut descriptors: Vec<Arc<DescriptorSet>> = Vec::new();

                if !descriptor_map.is_empty() {
                    let max_set = descriptor_map.keys().last().unwrap().set as usize;

                    let mut sets: Vec<BTreeMap<u32, &Descriptor<VulkanTypes>>> =
                        vec![BTreeMap::new(); max_set + 1];

                    for (location, descriptor) in descriptor_map {
                        sets[location.set as usize].insert(location.binding, descriptor);
                    }

                    let set_layouts = graphics_pipeline.layout().set_layouts();

                    for (i, set) in sets.into_iter().enumerate() {
                        let mut writes: Vec<WriteDescriptorSet> = Vec::new();

                        for (binding, descriptor) in set {
                            match descriptor {
                                Descriptor::Texture(texture) => {
                                    let view = texture.image_view().clone();

                                    let sampler =
                                        Sampler::new(&vulkan.device, &texture.vk_sampler())
                                            .unwrap();

                                    writes.push(WriteDescriptorSet::image_view_sampler(
                                        binding, view, sampler,
                                    ));
                                }
                                Descriptor::Buffer(buffer) => {
                                    let buffer = tcx.buffer(buffer.buffer())?;

                                    let subbuffer =
                                        vulkano::buffer::Subbuffer::new(buffer.buffer().clone());

                                    writes.push(WriteDescriptorSet::buffer(binding, subbuffer));
                                }
                                Descriptor::Mvp => {
                                    let buffer = tcx.buffer(world.view_proj_buffer_id)?;
                                    let subbuffer =
                                        vulkano::buffer::Subbuffer::new(buffer.buffer().clone());

                                    writes.push(WriteDescriptorSet::buffer_with_range(
                                        binding,
                                        vulkano::descriptor_set::DescriptorBufferInfo {
                                            buffer: subbuffer,
                                            offset: start,
                                            range,
                                        },
                                    ));
                                }
                            };
                        }

                        let descriptor = DescriptorSet::new(
                            vulkan.descriptor_set_allocator.clone(),
                            set_layouts[i].clone(),
                            writes,
                            [],
                        )
                        .unwrap();

                        descriptors.push(descriptor);
                    }
                }

                // Bind everything to the command buffer.
                let model = appearance.get_model();

                cbf.bind_pipeline_graphics(&graphics_pipeline)?;
                cbf.as_raw()
                    .bind_descriptor_sets(
                        vulkano::pipeline::PipelineBindPoint::Graphics,
                        graphics_pipeline.layout(),
                        0,
                        &descriptors
                            .iter()
                            .map(|x| x.as_raw())
                            .collect::<Vec<&RawDescriptorSet>>(),
                        &[],
                    )
                    .unwrap();
                cbf.push_constants(graphics_pipeline.layout(), 0, &model_matrix)?;
                cbf.bind_vertex_buffers(
                    0,
                    std::slice::from_ref(model.vertex_buffer_id()),
                    &[0],
                    &[],
                    &[],
                )?;

                // Draw object
                if let Some(index_subbuffer) = model.index_buffer_id().copied() {
                    unsafe {
                        cbf.bind_index_buffer(
                            index_subbuffer,
                            0,
                            model.index_len() as DeviceSize,
                            vulkano::buffer::IndexType::U32,
                        )?;
                        cbf.draw_indexed(model.index_len() as u32, 1, 0, 0, 0)?;
                    }
                } else {
                    unsafe {
                        cbf.draw(model.vertex_len() as u32, 1, 0, 0).unwrap();
                    }
                };
            }
        }

        Ok(())
    }
}
