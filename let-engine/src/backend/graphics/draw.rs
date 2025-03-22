use anyhow::Result;
use crossbeam::channel::Receiver;
use smallvec::SmallVec;
use std::{any::Any, collections::BTreeMap, sync::Arc};
use vulkano::{
    buffer::{
        allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo},
        BufferUsage,
    },
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer,
        RenderPassBeginInfo, SubpassBeginInfo, SubpassContents,
    },
    descriptor_set::{DescriptorSet, WriteDescriptorSet},
    device::Queue,
    image::sampler::Sampler,
    memory::allocator::MemoryTypeFilter,
    pipeline::{graphics::viewport::Viewport, Pipeline},
    render_pass::{Framebuffer, Subpass},
    swapchain::{
        acquire_next_image, Surface, Swapchain, SwapchainAcquireFuture, SwapchainCreateInfo,
        SwapchainPresentInfo,
    },
    sync::{self, GpuFuture},
    DeviceSize, Validated, VulkanError as VulkanoError,
};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use let_engine_core::{
    camera::Camera,
    objects::{
        scenes::{LayerView, Scene},
        Descriptor, MvpConfig, Node, VisualObject,
    },
    utils::ortho_maker,
};

use glam::{
    f32::{Mat4, Quat, Vec3},
    vec2, UVec2,
};

use super::{
    vulkan::{swapchain::create_swapchain_and_images, window_size_dependent_setup, Vulkan, VK},
    Graphics, GraphicsInterface, VulkanError, VulkanTypes,
};

/// Responsible for drawing on the surface.
pub struct Draw {
    swapchain: Arc<Swapchain>,
    subpass: Subpass,
    framebuffers: Vec<Arc<Framebuffer>>,
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
    ) -> Result<Self> {
        let vulkan = VK.get().unwrap();

        let surface = Surface::from_window(vulkan.instance.clone(), window)?;

        let (swapchain, images) =
            create_swapchain_and_images(&vulkan.device, &surface, &interface)?;

        let render_pass = vulkano::single_pass_renderpass!(
            vulkan.device.clone(),
            attachments: {
                color: {
                    format: vulkan.device.physical_device().surface_formats(&surface, Default::default())?[0].0,
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {}
            }
        )?;

        let drawing_queue = vulkan.queues.get_graphics().clone();

        let subpass = Subpass::from(render_pass.clone(), 0).unwrap();

        let _ = vulkan.subpass.set(subpass.clone());

        let mut viewport = Viewport {
            offset: [0.0; 2],
            extent: [0.0; 2],
            depth_range: 0.0..=1.0,
        };

        let framebuffers =
            window_size_dependent_setup(&images, render_pass.clone(), &mut viewport)?;

        let recreate_swapchain = false;

        let dimensions = UVec2::ZERO;

        let settings = interface.settings();
        let settings_channel = interface.settings_channels.1.clone();

        let uniform_buffer_allocator: SubbufferAllocator = SubbufferAllocator::new(
            vulkan.memory_allocator.clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::UNIFORM_BUFFER,
                memory_type_filter: MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
        );

        Ok(Self {
            swapchain,
            subpass,
            framebuffers,
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
    fn recreate_swapchain(&mut self) -> Result<(), VulkanError> {
        if self.recreate_swapchain {
            let (new_swapchain, new_images) = self
                .swapchain
                .recreate(SwapchainCreateInfo {
                    image_extent: self.dimensions.into(),
                    present_mode: self.settings.present_mode.into(),
                    ..self.swapchain.create_info()
                })
                .map_err(|e| VulkanError::from(e.unwrap()))?;

            self.swapchain = new_swapchain;
            self.framebuffers = window_size_dependent_setup(
                &new_images,
                self.subpass.render_pass().clone(),
                &mut self.viewport,
            )?;
            // vulkan.pipelines.clear();
            self.recreate_swapchain = false;
        };
        Ok(())
    }

    /// Makes a primary and secondary command buffer already inside a render pass.
    fn make_command_buffer(
        &self,
        image_num: usize,
        vulkan: &Vulkan,
    ) -> Result<AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, VulkanError> {
        let mut builder = AutoCommandBufferBuilder::primary(
            vulkan.command_buffer_allocator.clone(),
            self.drawing_queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .map_err(|e| VulkanError::from(e.unwrap()))?;

        // Makes a commandbuffer that takes multiple secondary buffers.
        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![Some(self.settings.clear_color.rgba().into())],
                    render_pass: self.subpass.render_pass().clone(),
                    ..RenderPassBeginInfo::framebuffer(self.framebuffers[image_num].clone())
                },
                SubpassBeginInfo {
                    contents: SubpassContents::Inline,
                    ..Default::default()
                },
            )
            .unwrap()
            .set_viewport(0, [self.viewport.clone()].into_iter().collect())
            .unwrap();

        Ok(builder)
    }

    fn make_mvp_matrix(
        object: &VisualObject<VulkanTypes>,
        dimensions: UVec2,
        camera: &Camera,
        mvp_config: &MvpConfig,
    ) -> SmallVec<[Mat4; 3]> {
        let mut mvp = SmallVec::new();

        if mvp_config.model {
            let transform = object.appearance.get_transform().combine(object.transform);
            let scaling = Vec3::new(transform.size[0], transform.size[1], 0.0);
            let rotation = Quat::from_rotation_z(transform.rotation);
            let translation = Vec3::new(transform.position[0], transform.position[1], 0.0);

            let model = Mat4::from_scale_rotation_translation(scaling, rotation, translation);

            mvp.push(model);
        };

        if mvp_config.view {
            let rotation = Mat4::from_rotation_z(camera.transform.rotation);

            let view = Mat4::look_at_rh(
                Vec3::from([
                    camera.transform.position[0],
                    camera.transform.position[1],
                    1.0,
                ]),
                Vec3::from([
                    camera.transform.position[0],
                    camera.transform.position[1],
                    0.0,
                ]),
                Vec3::Y,
            ) * rotation;

            mvp.push(view);
        };

        if mvp_config.projection {
            let zoom = 1.0 / camera.transform.size;
            let proj = ortho_maker(
                camera.scaling,
                camera.transform.position,
                zoom,
                vec2(dimensions[0] as f32, dimensions[1] as f32),
            );

            mvp.push(proj);
        };

        mvp
    }

    // TODO: Clean up clean up
    /// Draws the Game Scene on the given command buffer.
    fn draw_scene(
        &self,
        command_buffer: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        scene: &Scene<VulkanTypes>,
        vulkan: &Vulkan,
    ) -> Result<(), VulkanError> {
        let views: Vec<Arc<LayerView<VulkanTypes>>> = {
            let view = scene.views().lock();
            view.iter().cloned().collect()
        };

        /* Iterate All views */

        for layer_view in views {
            let layer = layer_view.layer();

            // Clear layer views with less references than 3.
            if Arc::strong_count(&layer_view) <= 2 {
                scene.update();
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

            /* Draw Objects */

            for object in order {
                let appearance = &object.appearance;

                // Skip objects marked as invisible.
                if !*appearance.get_visible() {
                    continue;
                };

                let material = appearance.get_material();

                /* Default MVP Matrix Creation */

                let mvp_matrices = Self::make_mvp_matrix(
                    &object,
                    self.dimensions,
                    &layer_view.camera(),
                    appearance.mvp_config(),
                );

                let mvp_subbuffer = (!mvp_matrices.is_empty()).then_some({
                    // MVP matrix for the object
                    let mvp_subbuffer = match self
                        .uniform_buffer_allocator
                        .allocate_slice(mvp_matrices.len() as DeviceSize)
                    {
                        Ok(subbuffer) => subbuffer,
                        Err(
                            vulkano::memory::allocator::MemoryAllocatorError::AllocateDeviceMemory(
                                e,
                            ),
                        ) => return Err(VulkanError::from(e.unwrap())),
                        _ => unreachable!(),
                    };

                    mvp_subbuffer
                        .write()
                        .unwrap()
                        .copy_from_slice(&mvp_matrices);

                    mvp_subbuffer
                });

                /* Descriptor Creation */

                let graphics_pipeline = vulkan.get_or_init_pipeline(material)?;

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
                                    let inner_texture = texture.inner().lock();

                                    let sampler = Sampler::new(
                                        vulkan.device.clone(),
                                        inner_texture.vk_sampler(),
                                    )
                                    .map_err(|e| VulkanError::from(e.unwrap()))?;

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
                            .map_err(|e| VulkanError::from(e.unwrap()))?,
                        );
                    }
                }

                // Bind everything to the command buffer.
                let model = appearance.get_model();

                let command_buffer = command_buffer
                    .set_viewport(0, [self.viewport.clone()].into_iter().collect())
                    .unwrap()
                    .bind_pipeline_graphics(graphics_pipeline.clone())
                    .unwrap()
                    .bind_descriptor_sets(
                        vulkano::pipeline::PipelineBindPoint::Graphics,
                        graphics_pipeline.layout().clone(),
                        0,
                        descriptors,
                    )
                    .unwrap()
                    .bind_vertex_buffers(0, model.vertex_buffer().clone())
                    .unwrap();

                // Draw object
                if let Some(index_subbuffer) = model.index_buffer().cloned() {
                    unsafe {
                        command_buffer
                            .bind_index_buffer(index_subbuffer)
                            .unwrap()
                            .draw_indexed(model.index_len(), 1, 0, 0, 0)
                            .unwrap();
                    }
                } else {
                    unsafe {
                        command_buffer.draw(model.vertex_len(), 1, 0, 0).unwrap();
                    }
                };
            }
        }
        Ok(())
    }

    /// Creates and executes a future in which the command buffer gets executed.
    fn execute_command_buffer(
        &mut self,
        command_buffer: Arc<PrimaryAutoCommandBuffer>,
        acquire_future: SwapchainAcquireFuture,
        image_num: u32,
        vulkan: &Vulkan,
    ) -> Result<(), VulkanError> {
        let future = if let Some(future) = vulkan.future.lock().take() {
            future.join(acquire_future).boxed_send()
        } else {
            acquire_future.boxed_send()
        }
        .then_execute(self.drawing_queue.clone(), command_buffer)
        .unwrap()
        .then_swapchain_present(
            self.drawing_queue.clone(),
            SwapchainPresentInfo::new(self.swapchain.clone(), image_num),
        )
        .then_signal_fence_and_flush();

        match future.map_err(Validated::unwrap) {
            Ok(future) => {
                *vulkan.future.lock() = Some(future.boxed_send());
            }
            Err(VulkanoError::OutOfDate) => {
                self.recreate_swapchain = true;
                *vulkan.future.lock() = Some(sync::now(vulkan.device.clone()).boxed_send());
            }
            Err(e) => {
                *vulkan.future.lock() = Some(sync::now(vulkan.device.clone()).boxed_send());
                return Err(VulkanError::Other(e));
            }
        }
        Ok(())
    }

    pub fn resize(&mut self, new_size: UVec2) {
        self.recreate_swapchain = true;
        self.dimensions = new_size;
    }

    /// Redraws the scene.
    pub fn redraw_event(&mut self, scene: &Scene<VulkanTypes>) -> Result<(), VulkanError> {
        let vulkan = VK.get().unwrap();

        if let Ok(settings) = self.settings_channel.try_recv() {
            self.settings = settings;
        }

        if let Some(future) = vulkan.future.lock().as_mut() {
            future.cleanup_finished();
        };

        if self.dimensions.x == 0 || self.dimensions.y == 0 {
            return Ok(());
        }

        let (image_num, suboptimal, acquire_future) = loop {
            self.recreate_swapchain()?;

            break match acquire_next_image(self.swapchain.clone(), None).map_err(Validated::unwrap)
            {
                Ok(r) => r,
                Err(VulkanoError::OutOfDate) => {
                    self.recreate_swapchain = true;
                    continue;
                }
                Err(e) => {
                    return Err(e.into());
                }
            };
        };

        if suboptimal {
            self.recreate_swapchain = true;
        }

        let mut builder = self.make_command_buffer(image_num as usize, vulkan)?;

        self.draw_scene(&mut builder, scene, vulkan)?;

        builder.end_render_pass(Default::default()).unwrap();
        let command_buffer = builder.build().map_err(|e| VulkanError::from(e.unwrap()))?;

        self.execute_command_buffer(command_buffer, acquire_future, image_num, vulkan)?;
        Ok(())
    }
}
