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
    image::{Image, sampler::Sampler},
    memory::{
        DeviceAlignment,
        allocator::{
            AllocationCreateInfo, BumpAllocator, DeviceLayout, MemoryTypeFilter, Suballocation,
            Suballocator, suballocator::Region,
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

use let_engine_core::objects::{
    Descriptor,
    scenes::{DrawTarget, LayerId, LayerViewId, Scene},
};

use glam::{UVec2, f32::Mat4, vec2};

use crate::backend::gpu::texture::TextureId;

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

    task_graph: Option<ExecutableTaskGraph<DrawWorld>>,

    suballocator: BumpAllocator,
    view_proj_alignment: DeviceAlignment,
    view_proj_buffer_id: Id<Buffer>,

    last_layer_tree_version: usize,

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

        let (view_proj_region, view_proj_alignment, view_proj_buffer_id) =
            Self::create_view_proj_buffer(vulkan);

        let suballocator = BumpAllocator::new(view_proj_region);

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

        let task_graph = None;

        vulkan.flag_taskgraph_to_be_rebuilt();

        Ok(Self {
            swapchain_id,
            virtual_swapchain_id: swapchain_id,
            recreate_swapchain,
            image_format,
            task_graph,
            dimensions,
            settings,
            settings_receiver,
            suballocator,
            last_layer_tree_version: 0,
            view_proj_alignment,
            view_proj_buffer_id,
            #[cfg(feature = "egui")]
            egui_system,
        })
    }

    fn recompile_task_graph(&mut self, vulkan: &Vulkan, scene: &Scene<VulkanTypes>) -> Result<()> {
        self.suballocator.reset();

        fn post_order_tasks(
            draw: &mut Draw,
            vulkan: &Vulkan,
            scene: &Scene<VulkanTypes>,
            task_graph: &mut TaskGraph<DrawWorld>,
            layer_id: LayerId,
            mut parent_id: Option<NodeId>,
            window_framebuffer_id: Id<vulkano::render_pass::Framebuffer>,
        ) {
            let layer = scene.layer(layer_id).unwrap();
            for view_id in layer.view_ids_iter() {
                let view = scene.view(view_id).unwrap();

                let view_suballocation = draw
                    .suballocator
                    .allocate(
                        DeviceLayout::new_sized::<[Mat4; 2]>(),
                        vulkano::memory::allocator::AllocationType::Linear,
                        draw.view_proj_alignment,
                    )
                    .expect("You can not create more than 256 cameras."); // TODO: Resize

                let framebuffer_id = match view.draw_target() {
                    DrawTarget::Window => window_framebuffer_id,
                    DrawTarget::Texture(_) => task_graph.add_framebuffer(),
                };

                let mut format = Format::UNDEFINED;

                let guard = unsafe { vulkan.collector.pin() };

                let image_id = match view.draw_target() {
                    DrawTarget::Window => ImageId::Swapchain(draw.virtual_swapchain_id),
                    DrawTarget::Texture(image_id) => {
                        let settings = vulkan.texture(*image_id, &guard).unwrap().settings();
                        assert!(settings.render_target, "Texture is not a render target");
                        format = crate::backend::gpu::format_to_vulkano(&settings.format);
                        ImageId::Texture(*image_id)
                    }
                };

                let mut builder = task_graph.create_task_node(
                    format!("view-{:?}", view_id),
                    vulkano_taskgraph::QueueFamilyType::Graphics,
                    DrawTask {
                        view_id,
                        view_suballocation,
                        node_id: NodeId::INVALID,
                        image_id,
                    },
                );

                builder.framebuffer(framebuffer_id);
                builder.color_attachment(
                    image_id.image_id(vulkan),
                    AccessTypes::COLOR_ATTACHMENT_WRITE,
                    vulkano_taskgraph::resource::ImageLayoutType::Optimal,
                    &AttachmentInfo {
                        clear: view.clear_color().is_some(),
                        format,
                        ..Default::default()
                    },
                );
                builder.buffer_access(
                    draw.view_proj_buffer_id,
                    AccessTypes::VERTEX_SHADER_UNIFORM_READ,
                );

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

                let node_id = builder.build();
                if let Some(parent_id) = parent_id {
                    task_graph.add_edge(node_id, parent_id).unwrap();
                }

                let task: &mut DrawTask = task_graph
                    .task_node_mut(node_id)
                    .unwrap()
                    .task_mut()
                    .downcast_mut()
                    .unwrap();

                task.node_id = node_id;
                parent_id = Some(node_id);
            }

            for layer_id in layer.layer_ids_iter() {
                post_order_tasks(
                    draw,
                    vulkan,
                    scene,
                    task_graph,
                    layer_id,
                    parent_id,
                    window_framebuffer_id,
                );
            }
        }

        let mut task_graph = TaskGraph::new(&vulkan.resources);

        self.virtual_swapchain_id = task_graph.add_swapchain(&SwapchainCreateInfo {
            image_format: self.image_format,
            ..Default::default()
        });
        let window_framebuffer_id = task_graph.add_framebuffer();

        task_graph.add_host_buffer_access(
            self.view_proj_buffer_id,
            vulkano_taskgraph::resource::HostAccessType::Write,
        );

        #[cfg(feature = "egui")]
        let egui_node = self.egui_system.render_egui(
            &mut task_graph,
            self.virtual_swapchain_id,
            window_framebuffer_id,
        );

        post_order_tasks(
            self,
            vulkan,
            scene,
            &mut task_graph,
            scene.root_layer_id(),
            #[cfg(not(feature = "egui"))]
            None,
            #[cfg(feature = "egui")]
            Some(egui_node),
            window_framebuffer_id,
        );

        let drawing_queue = vulkan.queues.general();

        self.task_graph = Some(unsafe {
            task_graph.compile(&CompileInfo {
                queues: &[drawing_queue],
                present_queue: Some(drawing_queue),
                flight_id: vulkan.graphics_flight,
                ..Default::default()
            })
        }?);

        #[cfg(feature = "egui")]
        self.egui_system.create_task_pipeline(
            self.task_graph.as_mut().unwrap(),
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

        if vulkan.rebuild_taskgraph() || self.last_layer_tree_version != scene.layer_tree_version()
        {
            self.recompile_task_graph(vulkan, scene).unwrap();
            self.last_layer_tree_version = scene.layer_tree_version();
        }

        self.recreate_swapchain(vulkan)?;

        #[cfg(feature = "egui")]
        self.egui_system
            .update_task_draw_data(self.task_graph.as_mut().unwrap());

        let task_graph = self.task_graph.as_ref().unwrap();

        let resource_map = resource_map!(
            task_graph,
            self.virtual_swapchain_id => self.swapchain_id,
        )
        .unwrap();

        let flight = vulkan.graphics_flight().unwrap();
        flight.wait(None).unwrap();

        // SAFETY: Creating the `DrawWorld` should only be done here and must drop by the end of this method, so after the task executed
        match unsafe {
            task_graph.execute(
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
        node_id: NodeId,
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

        let task_graph = self.task_graph.as_ref().unwrap();

        let draw_node = task_graph.task_node(node_id).unwrap();

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

        cache.insert((material_id, node_id), pipeline.clone());

        Ok(pipeline)
    }

    /// Searches a pipeline according to the given material and returns it if found.
    ///
    /// If this material is not found, it will be created and added to the cache.
    pub fn get_or_init_pipeline(
        &self,
        material_id: MaterialId,
        node_id: NodeId,
        vulkan: &Vulkan,
    ) -> Result<Arc<GraphicsPipeline>, VulkanError> {
        if let Some(pipeline) = {
            vulkan
                .pipeline_cache
                .lock()
                .get(&(material_id, node_id))
                .cloned()
        } {
            Ok(pipeline)
        } else {
            self.cache_pipeline(material_id, node_id, vulkan)
        }
    }
}

impl Drop for Draw {
    fn drop(&mut self) {
        let vulkan = VK.get().unwrap();
        vulkan.wait_transfer();
        vulkan.graphics_flight().unwrap().wait_idle().unwrap();
    }
}

// This is nothing but a hack arount the Rust and Vulkano limitation of one single world.
// It is not possible to pass 2 references, so I do it the dirty way: store two references under one.
struct DrawWorld {
    draw: *const Draw,
    scene: *const Scene<VulkanTypes>,
}

impl DrawWorld {
    fn new(draw: &Draw, scene: &Scene<VulkanTypes>) -> Self {
        Self { draw, scene }
    }
}

#[derive(Clone, Copy)]
enum ImageId {
    Texture(TextureId),
    Swapchain(Id<Swapchain>),
}

impl ImageId {
    fn image_id(&self, vulkan: &Vulkan) -> Id<Image> {
        match self {
            ImageId::Texture(id) => vulkan
                .texture(*id, &unsafe { vulkan.collector.pin() })
                .unwrap()
                .image_id(),
            ImageId::Swapchain(id) => id.current_image_id(),
        }
    }
}

struct DrawTask {
    view_id: LayerViewId,
    view_suballocation: Suballocation,
    node_id: NodeId,
    image_id: ImageId,
}

impl Task for DrawTask {
    type World = DrawWorld;

    fn clear_values(
        &self,
        clear_values: &mut vulkano_taskgraph::ClearValues<'_>,
        world: &Self::World,
    ) {
        let vulkan = VK.get().unwrap();
        let image_id = self.image_id.image_id(vulkan);
        let scene = unsafe { &*world.scene };
        let view = scene.view(self.view_id).unwrap();
        if let Some(clear_color) = view.clear_color() {
            clear_values.set(image_id, clear_color.rgba());
        }
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

        let view = scene.view(self.view_id).unwrap();
        let layer = scene.layer(view.layer_id()).unwrap();

        // Skip disabled layer view
        if !view.draw {
            return Ok(());
        }

        {
            let min = vec2(
                view.extent[0].x.min(view.extent[1].x),
                view.extent[0].y.min(view.extent[1].y),
            );
            let max = vec2(
                view.extent[0].x.max(view.extent[1].x),
                view.extent[0].y.max(view.extent[1].y),
            );
            // Skip if extent contains 0
            if (max - min).min_element() == 0.0 {
                return Ok(());
            }

            unsafe {
                cbf.set_viewport(
                    0,
                    std::slice::from_ref(&Viewport {
                        offset: (min * draw.dimensions.as_vec2()).into(),
                        extent: ((max - min) * draw.dimensions.as_vec2()).into(),
                        ..Default::default()
                    }),
                )?
            };
        }

        let start = self.view_suballocation.offset;
        let range = self.view_suballocation.size;

        // Write camera matrices into buffer regions
        let write: &mut [Mat4; 2] =
            tcx.write_buffer(draw.view_proj_buffer_id, start..start + range)?;

        write[0] = view.transform.make_view_matrix();
        write[1] = view
            .scaling
            .make_projection_matrix(draw.dimensions.as_vec2());

        let guard = unsafe { vulkan.collector.pin() };

        /* Draw Objects */

        for id in layer.object_ids_iter() {
            let object = scene.object(id).unwrap();

            let appearance = &object.appearance;

            // Skip objects marked as invisible.
            if !*appearance.visible() {
                continue;
            };

            let material_id = appearance.material_id();

            /* Default MVP Matrix Creation */

            let model_matrix: Mat4 = object.make_model_matrix();

            /* TEMP Descriptor Creation TEMP */

            let Ok(graphics_pipeline) =
                draw.get_or_init_pipeline(material_id, self.node_id, vulkan)
            else {
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
                                    Sampler::new(&vulkan.device, &texture.vk_sampler()).unwrap(),
                                );

                                writes.push(WriteDescriptorSet::image(
                                    binding,
                                    vulkano::descriptor_set::DescriptorImageInfo {
                                        sampler,
                                        image_view,
                                        image_layout:
                                            vulkano::image::ImageLayout::ShaderReadOnlyOptimal,
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

        Ok(())
    }
}
