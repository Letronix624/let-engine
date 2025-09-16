use anyhow::Result;
use crossbeam::channel::Receiver;

#[cfg(feature = "egui")]
use egui_winit_vulkano::{EguiSystem, RenderEguiWorld};

use std::{
    collections::BTreeMap,
    num::NonZero,
    sync::{Arc, OnceLock},
};
use vulkano::{
    DeviceSize,
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    descriptor_set::{DescriptorSet, WriteDescriptorSet, sys::RawDescriptorSet},
    format::Format,
    image::sampler::Sampler,
    memory::{
        DeviceAlignment,
        allocator::{
            AllocationCreateInfo, BumpAllocator, DeviceLayout, MemoryTypeFilter, Suballocator,
            suballocator::Region,
        },
    },
    pipeline::{
        DynamicState, GraphicsPipeline, Pipeline, PipelineLayout, PipelineShaderStageCreateInfo,
        graphics::{
            GraphicsPipelineCreateInfo,
            color_blend::{AttachmentBlend, ColorBlendAttachmentState, ColorBlendState},
            input_assembly::InputAssemblyState,
            multisample::MultisampleState,
            rasterization::RasterizationState,
            viewport::{Viewport, ViewportState},
        },
    },
    swapchain::{Surface, Swapchain, SwapchainCreateInfo},
};
use vulkano_taskgraph::{
    Id, Task,
    graph::{AttachmentInfo, CompileInfo, ExecutableTaskGraph, NodeId, TaskGraph},
    resource::AccessTypes,
    resource_map,
};
use winit::event_loop::ActiveEventLoop;

use let_engine_core::objects::{Descriptor, scenes::Scene};

use glam::{UVec2, f32::Mat4};

use super::{
    GpuSettings, PresentMode, VulkanError, VulkanTypes,
    material::MaterialId,
    vulkan::{ResourceAccess, VK, Vulkan, swapchain::create_swapchain, topology_to_vulkan},
};

/// Responsible for drawing on the surface.
pub struct Draw {
    swapchain_id: Id<Swapchain>,
    virtual_swapchain_id: Id<Swapchain>,

    recreate_swapchain: bool,

    settings_receiver: Receiver<GpuSettings>,
    settings: GpuSettings,
    dimensions: UVec2,
    image_format: Format,

    task_graph: ExecutableTaskGraph<DrawWorld>,
    draw_node_id: NodeId,

    viewport: Viewport,

    view_proj_region: Region,
    view_proj_alignment: DeviceAlignment,
    view_proj_buffer_id: Id<Buffer>,
    #[cfg(feature = "egui")]
    egui_system: EguiSystem<DrawWorld>,
}

#[cfg(feature = "egui")]
impl RenderEguiWorld<DrawWorld> for DrawWorld {
    fn get_egui_system(&self) -> &EguiSystem<DrawWorld> {
        unsafe { &(&*self.draw).egui_system }
    }

    fn get_swapchain_id(&self) -> Id<Swapchain> {
        unsafe { (&*self.draw).swapchain_id }
    }
}

impl Draw {
    pub fn new(
        settings: GpuSettings,
        settings_receiver: Receiver<GpuSettings>,
        present_modes: &OnceLock<Box<[PresentMode]>>,
        _event_loop: &ActiveEventLoop,
        window: &Arc<winit::window::Window>,
    ) -> Result<Self> {
        let vulkan = VK.get().unwrap();

        let surface = Surface::from_window(&vulkan.instance, window)?;

        let (swapchain_id, dimensions, image_format) =
            create_swapchain(&vulkan.device, surface.clone(), present_modes, vulkan)?;

        let recreate_swapchain = false;

        let viewport = Viewport {
            offset: [0.0; 2],
            extent: dimensions.as_vec2().into(),
            min_depth: 0.0,
            max_depth: 1.0,
        };

        let drawing_queue = vulkan.queues.general().clone();

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
                    settings,
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

        #[cfg(feature = "egui")]
        let egui_system = EguiSystem::new(
            _event_loop,
            &surface,
            vulkan.queues.general(),
            &vulkan.resources,
            vulkan.graphics_flight,
            image_format,
            egui_winit_vulkano::EguiSystemConfig {
                use_bindless: false,
                debug_utils: None,
            },
        );

        Ok(Self {
            swapchain_id,
            virtual_swapchain_id,
            recreate_swapchain,
            image_format,
            dimensions,
            viewport,
            task_graph,
            draw_node_id,
            settings,
            settings_receiver,
            view_proj_region,
            view_proj_alignment,
            view_proj_buffer_id,
            #[cfg(feature = "egui")]
            egui_system,
        })
    }

    fn recompile_task_graph(&mut self, vulkan: &Vulkan) -> Result<()> {
        let mut task_graph = TaskGraph::new(&vulkan.resources);

        let virtual_swapchain_id = task_graph.add_swapchain(&SwapchainCreateInfo {
            image_format: self.image_format,
            ..Default::default()
        });
        let virtual_framebuffer_id = task_graph.add_framebuffer();

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
        builder.framebuffer(virtual_framebuffer_id);
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

        let guard = unsafe { vulkan.collector.pin() };

        let resources = vulkan.iter_resource_access(&guard);

        for access in resources {
            match access {
                ResourceAccess::Buffer { id, access_types } => {
                    builder.buffer_access(id, access_types);
                }
                ResourceAccess::Image { id, access_types } => {
                    builder.image_access(
                        id,
                        access_types,
                        vulkano_taskgraph::resource::ImageLayoutType::Optimal,
                    );
                }
            }
        }

        self.draw_node_id = builder.build();

        #[cfg(feature = "egui")]
        {
            let egui_node = self.egui_system.render_egui(
                &mut task_graph,
                virtual_swapchain_id,
                virtual_framebuffer_id,
            );

            task_graph.add_edge(self.draw_node_id, egui_node).unwrap();
        }

        let drawing_queue = vulkan.queues.general();

        self.task_graph = unsafe {
            task_graph.compile(&CompileInfo {
                queues: &[drawing_queue],
                present_queue: Some(drawing_queue),
                flight_id: vulkan.graphics_flight,
                ..Default::default()
            })
        }?;

        #[cfg(feature = "egui")]
        self.egui_system.create_task_pipeline(
            &mut self.task_graph,
            &vulkan.resources,
            &vulkan.device,
        );

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

    #[cfg(feature = "egui")]
    pub fn egui_update(&mut self, event: &winit::event::WindowEvent) -> bool {
        self.egui_system.update(event)
    }

    #[cfg(feature = "egui")]
    pub fn draw_egui(&mut self) -> egui::Context {
        self.egui_system.immediate_ui()
    }

    pub fn resize(&mut self, new_size: UVec2) {
        self.recreate_swapchain = true;
        self.dimensions = new_size;
        self.viewport.extent = new_size.as_vec2().into();
    }

    /// Redraws the scene.
    pub fn redraw_event(
        &mut self,
        scene: &Scene<VulkanTypes>,
        pre_present_notify: impl FnOnce(),
    ) -> Result<(), VulkanError> {
        if let Ok(settings) = self.settings_receiver.try_recv() {
            self.settings = settings;
        }

        if self.dimensions.x == 0 || self.dimensions.y == 0 {
            return Ok(());
        }

        let vulkan = VK.get().unwrap();

        if vulkan.clean_resources() {
            self.recompile_task_graph(vulkan).unwrap();
        }

        self.recreate_swapchain(vulkan)?;

        #[cfg(feature = "egui")]
        self.egui_system.update_task_draw_data(&mut self.task_graph);

        let resource_map = resource_map!(
            &self.task_graph,
            self.virtual_swapchain_id => self.swapchain_id,
        )
        .unwrap();

        let flight = vulkan.graphics_flight().unwrap();
        flight.wait(None).unwrap();

        // SAFETY: Creating the `DrawWorld` should only be done here and must drop by the end of this method, so after the task executed
        match unsafe {
            self.task_graph.execute(
                resource_map,
                &DrawWorld::new(self, scene),
                pre_present_notify,
            )
        } {
            Ok(()) => Ok(()),
            Err(vulkano_taskgraph::graph::ExecuteError::Swapchain {
                error: vulkano::VulkanError::OutOfDate,
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
        material_id: MaterialId,
        vulkan: &Vulkan,
    ) -> Result<Arc<GraphicsPipeline>, VulkanError> {
        let guard = unsafe { vulkan.collector.pin() };
        let material = vulkan.material(material_id, &guard).unwrap();

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

        cache.insert(material_id, pipeline.clone());

        Ok(pipeline)
    }

    /// Searches a pipeline according to the given material and returns it if found.
    ///
    /// If this material is not found, it will be created and added to the cache.
    pub fn get_or_init_pipeline(
        &self,
        material_id: MaterialId,
        vulkan: &Vulkan,
    ) -> Result<Arc<GraphicsPipeline>, VulkanError> {
        if let Some(pipeline) = vulkan.get_pipeline(material_id) {
            Ok(pipeline)
        } else {
            self.cache_pipeline(material_id, vulkan)
        }
    }
}

struct DrawTask {
    swapchain_id: Id<Swapchain>,
    settings: GpuSettings,
}

// This is nothing but a hack arount the Rust and Vulkano limitation of one single world.
// It is not possible to pass 2 references, so I do it the dirty way: store two references under one.
struct DrawWorld {
    draw: *const Draw,
    scene: *const Scene<VulkanTypes>,
}

impl DrawWorld {
    unsafe fn new(draw: &Draw, scene: &Scene<VulkanTypes>) -> Self {
        Self { draw, scene }
    }
}

impl Task for DrawTask {
    type World = DrawWorld;

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

        let scene = unsafe { &*world.scene };
        let draw = unsafe { &*world.draw };

        // TODO: Add order
        let views = scene.views_iter();

        unsafe { cbf.set_viewport(0, std::slice::from_ref(&draw.viewport))? };

        let mut suballocator = BumpAllocator::new(draw.view_proj_region);

        let views = views.map(|(_, view)| {
            let suballocation = suballocator
                .allocate(
                    DeviceLayout::new_sized::<[Mat4; 2]>(),
                    vulkano::memory::allocator::AllocationType::Linear,
                    draw.view_proj_alignment,
                )
                .expect("You can not create more than 256 cameras."); // TODO: Resize
            let start = suballocation.offset;
            let size = suballocation.size;

            (view, start, size)
        });

        /* Iterate All views */

        for (view, start, range) in views {
            let layer = scene.layer(view.layer_id()).unwrap();

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
                tcx.write_buffer(draw.view_proj_buffer_id, start..start + range)?;

            write[0] = view.camera().make_view_matrix();
            write[1] = view.make_projection_matrix();

            let guard = unsafe { vulkan.collector.pin() };

            /* Draw Objects */

            for id in layer.object_ids_iter() {
                let object = scene.object(*id).unwrap();

                let appearance = &object.appearance;

                // Skip objects marked as invisible.
                if !*appearance.visible() {
                    continue;
                };

                let material_id = appearance.material_id();

                /* Default MVP Matrix Creation */

                let model_matrix: Mat4 = object.make_model_matrix();

                /* TEMP Descriptor Creation TEMP */

                let Ok(graphics_pipeline) = draw.get_or_init_pipeline(material_id, vulkan) else {
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
                                Descriptor::Texture(texture_id) => {
                                    let texture = vulkan.texture(*texture_id, &guard).unwrap();
                                    let image_view = Some(texture.image_view().clone());

                                    let sampler = Some(
                                        Sampler::new(&vulkan.device, &texture.vk_sampler())
                                            .unwrap(),
                                    );

                                    writes.push(WriteDescriptorSet::image(
                                        binding,
                                        vulkano::descriptor_set::DescriptorImageInfo {
                                            sampler,
                                            image_view,
                                            ..Default::default()
                                        },
                                    ));
                                }
                                Descriptor::Buffer(buffer_id) => {
                                    let buffer = vulkan.buffer(*buffer_id, &guard).unwrap();
                                    let buffer = tcx.buffer(buffer.buffer_id())?;

                                    let subbuffer =
                                        vulkano::buffer::Subbuffer::new(buffer.buffer().clone());

                                    writes.push(WriteDescriptorSet::buffer(binding, subbuffer));
                                }
                                Descriptor::Mvp => {
                                    let buffer = tcx.buffer(draw.view_proj_buffer_id)?;
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
                let model = vulkan
                    .model(unsafe { appearance.model_id() }, &guard)
                    .unwrap();

                unsafe { cbf.bind_pipeline_graphics(&graphics_pipeline)? };
                unsafe {
                    cbf.as_raw().bind_descriptor_sets(
                        vulkano::pipeline::PipelineBindPoint::Graphics,
                        graphics_pipeline.layout(),
                        0,
                        &descriptors
                            .iter()
                            .map(|x| x.as_raw())
                            .collect::<Vec<&RawDescriptorSet>>(),
                        &[],
                    )
                }
                .unwrap();
                cbf.destroy_objects(descriptors);
                unsafe { cbf.push_constants(graphics_pipeline.layout(), 0, &model_matrix)? };
                unsafe {
                    cbf.bind_vertex_buffers(
                        0,
                        std::slice::from_ref(&model.vertex_buffer_id()),
                        &[0],
                        &[],
                        &[],
                    )?
                };

                // Draw object
                if let Some(index_subbuffer) = model.index_buffer_id() {
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
