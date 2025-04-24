use anyhow::Result;
use crossbeam::channel::Receiver;
use smallvec::SmallVec;
use std::{any::Any, collections::BTreeMap, sync::Arc};
use vulkano::{
    buffer::{
        allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo},
        BufferUsage,
    },
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer},
    descriptor_set::{sys::RawDescriptorSet, DescriptorSet, WriteDescriptorSet},
    device::Queue,
    image::sampler::Sampler,
    memory::allocator::MemoryTypeFilter,
    pipeline::{graphics::viewport::Viewport, Pipeline},
    swapchain::{
        Surface, Swapchain, SwapchainAcquireFuture, SwapchainCreateInfo, SwapchainPresentInfo,
    },
    sync::{self, GpuFuture},
    DeviceSize, Validated, VulkanError as VulkanoError,
};
use vulkano_taskgraph::{Id, Task};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use let_engine_core::{
    camera::Camera,
    objects::{
        scenes::{LayerView, Scene},
        Descriptor, MvpConfig, Node, VisualObject,
    },
};

use glam::{
    f32::{Mat4, Quat, Vec3},
    UVec2, Vec2,
};

use super::{
    vulkan::{swapchain::create_swapchain, Vulkan, VK},
    Graphics, GraphicsInterface, VulkanError, VulkanTypes,
};

/// Responsible for drawing on the surface.
pub struct Draw {
    swapchain: Id<Swapchain>,
    // previous_frame_end: Option<Box<dyn GpuFuture>>,
    recreate_swapchain: bool,

    settings_channel: Receiver<Graphics>,
    settings: Graphics,
    dimensions: UVec2,

    drawing_queue: Arc<Queue>,

    viewport: Viewport,

    uniform_buffer_allocator: SubbufferAllocator,
}

impl Draw {
    pub fn setup(
        interface: GraphicsInterface,
        window: Arc<impl HasWindowHandle + HasDisplayHandle + Any + Send + Sync>,
        scene: Scene<VulkanTypes>,
    ) -> Result<Self> {
        let vulkan = VK.get().unwrap();

        let surface = Surface::from_window(vulkan.instance.clone(), window)?;

        let (swapchain, dimensions) =
            create_swapchain(&vulkan.device, surface, &interface, vulkan)?;

        let viewport = Viewport {
            offset: [0.0; 2],
            extent: dimensions.as_vec2().into(),
            depth_range: 0.0..=1.0,
        };

        let drawing_queue = vulkan.queues.get_graphics().clone();

        let recreate_swapchain = false;

        let settings = interface.settings();
        let settings_channel = interface.settings_channels.1.clone();

        let uniform_buffer_allocator: SubbufferAllocator = SubbufferAllocator::new(
            vulkan.resources.memory_allocator().clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::UNIFORM_BUFFER,
                memory_type_filter: MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
        );

        Ok(Self {
            swapchain,
            drawing_queue,
            // previous_frame_end,
            recreate_swapchain,
            dimensions,
            viewport,
            settings,
            settings_channel,
            uniform_buffer_allocator,
        })
    }

    // fn window(&self) -> Window

    /// Recreates the swapchain in case it is out of date if someone for example changed the scene size or window dimensions.
    fn recreate_swapchain(&mut self, vulkan: &Vulkan) -> Result<(), VulkanError> {
        if self.recreate_swapchain {
            self.swapchain = vulkan
                .resources
                .recreate_swapchain(self.swapchain, |create_info| SwapchainCreateInfo {
                    image_extent: self.dimensions.into(),
                    ..create_info
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
    pub fn redraw_event(&mut self) -> Result<(), VulkanError> {
        if let Ok(settings) = self.settings_channel.try_recv() {
            self.settings = settings;
        }

        if self.dimensions.x == 0 || self.dimensions.y == 0 {
            return Ok(());
        }

        let vulkan = VK.get().unwrap();

        self.recreate_swapchain(vulkan)?;

        // DRAW

        Ok(())
    }
}

struct DrawTask {
    swapchain_id: Id<Swapchain>,
    settings: Graphics,
    scene: Scene<VulkanTypes>,
}

impl Task for DrawTask {
    type World = Draw;

    fn clear_values(&self, clear_values: &mut vulkano_taskgraph::ClearValues<'_>) {
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
            let view = self.scene.views().lock();
            view.iter().cloned().collect()
        };

        /* Iterate All views */

        for layer_view in views {
            let layer = layer_view.layer();

            // Clear layer views with less references than 3.
            if Arc::strong_count(&layer_view) <= 2 {
                self.scene.update();
                continue;
            }
            // Skip disabled layer view
            if !layer_view.draw() {
                continue;
            }

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

            let view_matrix = layer_view.camera().make_view_matrix();
            let projection_matrix = layer_view.camera().make_projection_matrix(world.dimensions);

            /* Draw Objects */

            for object in order {
                let appearance = &object.appearance;

                // Skip objects marked as invisible.
                if !*appearance.get_visible() {
                    continue;
                };

                let material = appearance.get_material();

                /* Default MVP Matrix Creation */

                let mut mvp_matrices = SmallVec::with_capacity(3);

                {
                    let mvp_config = appearance.mvp_config();

                    if mvp_config.model {
                        mvp_matrices.push(object.make_model_matrix());
                    }
                    if mvp_config.view {
                        mvp_matrices.push(view_matrix);
                    }
                    if mvp_config.projection {
                        mvp_matrices.push(projection_matrix);
                    }
                }

                let mvp_subbuffer = (!mvp_matrices.is_empty()).then_some({
                    // MVP matrix for the object
                    let mvp_subbuffer = match world
                        .uniform_buffer_allocator
                        .allocate_slice(mvp_matrices.len() as DeviceSize)
                    {
                        Ok(subbuffer) => subbuffer,
                        Err(
                            vulkano::memory::allocator::MemoryAllocatorError::AllocateDeviceMemory(
                                e,
                            ),
                        ) => todo!("{e}"),
                        _ => unreachable!(),
                    };

                    mvp_subbuffer
                        .write()
                        .unwrap()
                        .copy_from_slice(&mvp_matrices);

                    mvp_subbuffer
                });

                /* Descriptor Creation */

                let Ok(graphics_pipeline) = vulkan.get_or_init_pipeline(material) else {
                    log::error!("Failed to create pipeline for object.");
                    continue;
                };

                let descriptor_map = appearance.descriptors();

                let mut descriptors: Vec<&RawDescriptorSet> = Vec::new();

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
                                    let inner_texture = texture.inner().lock();

                                    let sampler = Sampler::new(
                                        vulkan.device.clone(),
                                        inner_texture.vk_sampler(),
                                    )
                                    .unwrap();

                                    writes.push(WriteDescriptorSet::image_view_sampler(
                                        binding,
                                        inner_texture.view().clone(),
                                        sampler,
                                    ));
                                }
                                Descriptor::Buffer(buffer) => {
                                    writes.push(WriteDescriptorSet::buffer(
                                        binding,
                                        buffer.buffer().clone(),
                                    ));
                                }
                                Descriptor::Mvp => {
                                    if let Some(mvp_subbuffer) = mvp_subbuffer.clone() {
                                        writes.push(WriteDescriptorSet::buffer(
                                            binding,
                                            mvp_subbuffer,
                                        ));
                                    }
                                }
                            };
                        }

                        descriptors.push(
                            DescriptorSet::new(
                                vulkan.descriptor_set_allocator.clone(),
                                set_layouts[i].clone(),
                                writes,
                                [],
                            )
                            .unwrap()
                            .as_raw(),
                        );
                    }
                }

                // Bind everything to the command buffer.
                let model = appearance.get_model();

                cbf.bind_pipeline_graphics(&graphics_pipeline)?;
                cbf.as_raw()
                    .bind_descriptor_sets(
                        vulkano::pipeline::PipelineBindPoint::Graphics,
                        &graphics_pipeline.layout(),
                        0,
                        &descriptors,
                        &[],
                    )
                    .unwrap();
                cbf.bind_vertex_buffers(0, model.vertex_buffer().clone(), &[0], &[], &[])?;

                // Draw object
                if let Some(index_subbuffer) = model.index_buffer().cloned() {
                    unsafe {
                        cbf.bind_index_buffer(
                            index_subbuffer,
                            0,
                            model.index_len() as u64,
                            vulkano::buffer::IndexType::U32,
                        )?;
                        cbf.draw_indexed(model.index_len(), 1, 0, 0, 0)?;
                    }
                } else {
                    unsafe {
                        command_buffer.draw(model.vertex_len(), 1, 0, 0).unwrap();
                    }
                };
            }
        }
        cbf.set_viewport(0, std::slice::from_ref(&world.viewport))?;

        Ok(())
    }
}
