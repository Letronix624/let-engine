use crate::{error::draw::*, prelude::*};
use anyhow::Result;
use std::sync::Arc;
use vulkano::{
    command_buffer::{
        CommandBuffer, CommandBufferBeginInfo, CommandBufferInheritanceInfo, CommandBufferLevel,
        CommandBufferUsage, RecordingCommandBuffer, RenderPassBeginInfo, SubpassBeginInfo,
        SubpassContents,
    },
    descriptor_set::{DescriptorSet, WriteDescriptorSet},
    pipeline::{graphics::viewport::Viewport, GraphicsPipeline, Pipeline},
    render_pass::Framebuffer,
    swapchain::{
        acquire_next_image, Surface, Swapchain, SwapchainAcquireFuture, SwapchainCreateInfo,
        SwapchainPresentInfo,
    },
    sync::{self, GpuFuture},
    Validated, VulkanError as VulkanoError,
};

use super::resources::vulkan::{
    swapchain::create_swapchain_and_images, window_size_dependent_setup,
};

//use cgmath::{Deg, Matrix3, Matrix4, Ortho, Point3, Rad, Vector3};
use glam::f32::{Mat4, Quat, Vec3};

/// Responsible for drawing on the surface.
pub struct Draw {
    pub surface: Arc<Surface>,
    pub window: Arc<Window>,
    pub(crate) swapchain: Arc<Swapchain>,
    pub(crate) viewport: Viewport,
    pub(crate) framebuffers: Vec<Arc<Framebuffer>>,
    pub(crate) previous_frame_end: Option<Box<dyn GpuFuture>>,
    dimensions: [u32; 2],
    default_pipeline: Arc<GraphicsPipeline>,
    default_instance_pipeline: Arc<GraphicsPipeline>,
}

impl Draw {
    pub(crate) fn setup(window_builder: WindowBuilder) -> Result<Self> {
        let resources = &RESOURCES;
        let vulkan = resources.vulkan().clone();
        let loader = resources.loader().lock();
        let (surface, window) = EVENT_LOOP.with_borrow(|event_loop| {
            return vulkan::window::create_window(
                event_loop.get().expect("An unexpected error occured."), // I do not know when this could cause a crash.
                &resources.vulkan().instance,
                window_builder,
            );
        })?;

        let (swapchain, images) = create_swapchain_and_images(&vulkan.device, &surface)?;

        let mut viewport = Viewport {
            offset: [0.0, 0.0],
            extent: [0.0, 0.0],
            depth_range: 0.0..=1.0,
        };

        let framebuffers =
            window_size_dependent_setup(&images, vulkan.render_pass.clone(), &mut viewport)?;

        let uploads = RecordingCommandBuffer::new(
            loader.command_buffer_allocator.clone(),
            vulkan.queue.queue_family_index(),
            CommandBufferLevel::Primary,
            CommandBufferBeginInfo {
                usage: CommandBufferUsage::OneTimeSubmit,
                ..Default::default()
            },
        )?;

        let previous_frame_end = Some(uploads.end()?.execute(vulkan.queue.clone())?.boxed());

        let dimensions = [0; 2];

        let default_pipeline = vulkan.default_material.pipeline.clone();
        let default_instance_pipeline = vulkan.default_instance_material.pipeline.clone();

        Ok(Self {
            surface,
            window,
            swapchain,
            viewport,
            framebuffers,
            previous_frame_end,
            dimensions,
            default_pipeline,
            default_instance_pipeline,
        })
    }

    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }

    /// Recreates the swapchain in case it's out of date if someone for example changed the scene size or window dimensions.
    fn recreate_swapchain(&mut self) -> Result<(), VulkanError> {
        if SETTINGS
            .graphics
            .recreate_swapchain
            .load(std::sync::atomic::Ordering::Acquire)
        {
            let (new_swapchain, new_images) = match self.swapchain.recreate(SwapchainCreateInfo {
                image_extent: self.dimensions,
                present_mode: SETTINGS.graphics.present_mode().into(),
                ..self.swapchain.create_info()
            }) {
                Ok(r) => r,
                Err(e) => {
                    return Err(e.into());
                }
            };

            let resources = &RESOURCES;
            self.swapchain = new_swapchain;
            self.framebuffers = window_size_dependent_setup(
                &new_images,
                resources.vulkan().render_pass.clone(),
                &mut self.viewport,
            )
            .map_err(VulkanError::Other)?;
            SETTINGS
                .graphics
                .recreate_swapchain
                .store(false, std::sync::atomic::Ordering::Release);
        };
        Ok(())
    }

    /// Makes a primary and secondary command buffer already inside a render pass.
    fn make_command_buffer(
        &self,
        image_num: usize,
        clear_color: [f32; 4],
        loader: &Loader,
    ) -> Result<(RecordingCommandBuffer, RecordingCommandBuffer), VulkanError> {
        let resources = &RESOURCES;
        let vulkan = resources.vulkan().clone();
        let mut builder = RecordingCommandBuffer::new(
            loader.command_buffer_allocator.clone(),
            vulkan.queue.queue_family_index(),
            CommandBufferLevel::Primary,
            CommandBufferBeginInfo {
                usage: CommandBufferUsage::OneTimeSubmit,
                ..Default::default()
            },
        )
        .map_err(Validated::unwrap)
        .map_err(VulkanError::Validated)?;

        // Makes a commandbuffer that takes multiple secondary buffers.
        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![Some(clear_color.into())],
                    ..RenderPassBeginInfo::framebuffer(self.framebuffers[image_num].clone())
                },
                SubpassBeginInfo {
                    contents: SubpassContents::SecondaryCommandBuffers,
                    ..Default::default()
                },
            )
            .map_err(|e| VulkanError::Other(e.into()))?;

        let mut secondary_builder = RecordingCommandBuffer::new(
            loader.command_buffer_allocator.clone(),
            vulkan.queue.queue_family_index(),
            CommandBufferLevel::Secondary,
            CommandBufferBeginInfo {
                usage: CommandBufferUsage::OneTimeSubmit,
                inheritance_info: Some(CommandBufferInheritanceInfo {
                    render_pass: Some(vulkan.subpass.clone().into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .map_err(Validated::unwrap)
        .map_err(VulkanError::Validated)?;
        secondary_builder
            .set_viewport(0, [self.viewport.clone()].into_iter().collect())
            .map_err(|e| VulkanError::Other(e.into()))?;

        Ok((builder, secondary_builder))
    }

    fn make_mvp_matrix(
        object: &VisualObject,
        dimensions: [u32; 2],
        camera: &Object,
        camera_settings: CameraSettings,
    ) -> (Mat4, Mat4, Mat4) {
        let transform = object.transform.combine(*object.appearance.get_transform());
        let scaling = Vec3::new(transform.size[0], transform.size[1], 0.0);
        let rotation = Quat::from_rotation_z(transform.rotation);
        let translation = Vec3::new(transform.position[0], transform.position[1], 0.0);

        // Model matrix
        let model = Mat4::from_scale_rotation_translation(scaling, rotation, translation);

        // View matrix
        let rotation = Mat4::from_rotation_z(camera.transform.rotation);

        let zoom = 1.0 / camera_settings.zoom;

        // Projection matrix
        let proj = utils::ortho_maker(
            camera_settings.mode,
            camera.transform.position,
            zoom,
            vec2(dimensions[0] as f32, dimensions[1] as f32),
        );

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
        (model, view, proj)
    }

    /// Draws the Game Scene on the given command buffer.
    fn write_secondary_command_buffer(
        &self,
        command_buffer: &mut RecordingCommandBuffer,
        loader: &mut Loader,
    ) -> Result<(), VulkanError> {
        for layer in SCENE.layers().iter() {
            let mut order: Vec<VisualObject> = Vec::with_capacity(layer.objects_map.lock().len());
            let mut instances: Vec<Instance> = vec![];

            Node::order_position(&mut order, &layer.root.lock());

            for object in order {
                let appearance = &object.appearance;

                let Some(model) = appearance.get_model() else {
                    continue;
                };

                let resources = &RESOURCES;
                let shapes = resources.shapes().clone();

                let model_data = match model {
                    Model::Custom(data) => data,
                    Model::Square => &shapes.square,
                    Model::Triangle => &shapes.triangle,
                };

                // Skip drawing the object if the object is not marked visible or has no vertices.
                if appearance.is_instanced() {
                    // appearance.instance.drawing.
                    appearance.instance.draw(&mut instances);
                    let mut data = appearance.instance.instance_data.lock();
                    let (model, view, proj) = Self::make_mvp_matrix(
                        &object,
                        self.dimensions,
                        &layer.camera.lock().lock().object,
                        layer.camera_settings(),
                    );
                    let instance_data = InstanceData {
                        model,
                        view,
                        proj,
                        color: (*appearance.get_color()).into(),
                        layer: appearance.layer().unwrap_or(0),
                    };
                    data.push(instance_data);
                    continue;
                };

                let mut descriptors = vec![];

                // The pipeline of the current object. Takes the default one if there is none.
                let pipeline = if let Some(material) = appearance.get_material() {
                    if let Some(texture) = &material.texture {
                        descriptors.push(texture.set().clone());
                    }
                    if let Some(descriptor) = &material.descriptor {
                        descriptors.push(descriptor.clone());
                    }
                    material.pipeline.clone()
                } else {
                    self.default_pipeline.clone()
                };

                // MVP matrix for the object
                let objectvert_sub_buffer = loader
                    .object_buffer_allocator
                    .allocate_sized()
                    .map_err(|error| VulkanError::Other(error.into()))?;
                // Simple color and texture data for the fragment shader.
                let objectfrag_sub_buffer = loader
                    .object_buffer_allocator
                    .allocate_sized()
                    .map_err(|error| VulkanError::Other(error.into()))?;

                let (model, view, proj) = Self::make_mvp_matrix(
                    &object,
                    self.dimensions,
                    &layer.camera.lock().lock().object,
                    layer.camera_settings(),
                );

                *objectvert_sub_buffer
                    .write()
                    .map_err(|error| VulkanError::Other(error.into()))? =
                    ModelViewProj { model, view, proj };
                *objectfrag_sub_buffer
                    .write()
                    .map_err(|error| VulkanError::Other(error.into()))? = ObjectFrag {
                    color: (*appearance.get_color()).into(),
                    texture_id: if let Some(material) = appearance.get_material() {
                        material.layer
                    } else {
                        0
                    },
                };

                descriptors.insert(
                    0,
                    DescriptorSet::new(
                        loader.descriptor_set_allocator.clone(),
                        pipeline
                            .layout()
                            .set_layouts()
                            .first()
                            .ok_or(VulkanError::ShaderError)?
                            .clone(),
                        [
                            WriteDescriptorSet::buffer(0, objectvert_sub_buffer.clone()),
                            WriteDescriptorSet::buffer(1, objectfrag_sub_buffer.clone()),
                        ],
                        [],
                    )
                    .map_err(Validated::unwrap)
                    .map_err(VulkanError::Validated)?,
                );

                let command_buffer = command_buffer
                    .bind_pipeline_graphics(pipeline.clone())
                    .map_err(|e| VulkanError::Other(e.into()))?
                    .bind_descriptor_sets(
                        vulkano::pipeline::PipelineBindPoint::Graphics,
                        pipeline.layout().clone(),
                        0,
                        descriptors,
                    )
                    .map_err(|e| VulkanError::Other(e.into()))?
                    .bind_vertex_buffers(0, model_data.vertex_buffer())
                    .map_err(|e| VulkanError::Other(e.into()))?
                    .bind_index_buffer(model_data.index_buffer())
                    .map_err(|e| VulkanError::Other(e.into()))?;
                unsafe {
                    command_buffer
                        .draw_indexed(model_data.size() as u32, 1, 0, 0, 0)
                        .map_err(|e| VulkanError::Other(e.into()))?;
                }
            }
            for instance in instances {
                let Some(model) = instance.model.as_ref() else {
                    continue;
                };

                let mut data = instance.instance_data.lock();
                let instance_buffer = loader
                    .instance_buffer_allocator
                    .allocate_slice::<InstanceData>(data.len() as u64)
                    .map_err(|e| VulkanError::Other(e.into()))?;
                instance_buffer
                    .write()
                    .map_err(|e| VulkanError::Other(e.into()))?
                    .copy_from_slice(&data);

                let mut descriptors = vec![];

                // The pipeline of the current object. Takes the default one if there is none.
                let pipeline = if let Some(material) = &instance.material {
                    if let Some(texture) = &material.texture {
                        descriptors.push(texture.set().clone());
                    }
                    if let Some(descriptor) = &material.descriptor {
                        descriptors.push(descriptor.clone());
                    }
                    material.pipeline.clone()
                } else {
                    self.default_instance_pipeline.clone()
                };

                let resources = &RESOURCES;
                let shapes = resources.shapes().clone();
                let model = match &model {
                    Model::Custom(data) => data,
                    Model::Square => &shapes.square,
                    Model::Triangle => &shapes.triangle,
                };

                let command_buffer = command_buffer
                    .bind_pipeline_graphics(pipeline.clone())
                    .map_err(|e| VulkanError::Other(e.into()))?
                    .bind_descriptor_sets(
                        vulkano::pipeline::PipelineBindPoint::Graphics,
                        pipeline.layout().clone(),
                        0,
                        descriptors,
                    )
                    .map_err(|e| VulkanError::Other(e.into()))?
                    .bind_vertex_buffers(0, (model.vertex_buffer(), instance_buffer))
                    .map_err(|e| VulkanError::Other(e.into()))?
                    .bind_index_buffer(model.index_buffer())
                    .map_err(|e| VulkanError::Other(e.into()))?;
                unsafe {
                    command_buffer
                        .draw_indexed(model.size() as u32, data.len() as u32, 0, 0, 0)
                        .map_err(|e| VulkanError::Other(e.into()))?;
                }
                instance.finish_drawing();
                data.clear();
            }
        }
        Ok(())
    }

    /// Creates and executes a future in which the command buffer gets executed.
    fn execute_command_buffer(
        &mut self,
        command_buffer: Arc<CommandBuffer>,
        acquire_future: SwapchainAcquireFuture,
        image_num: u32,
    ) -> Result<(), VulkanError> {
        let resources = &RESOURCES;
        let vulkan = resources.vulkan().clone();
        let future = self
            .previous_frame_end
            .take()
            .ok_or(VulkanError::FlushFutureError(
                "Failed to obtain previous frame".to_string(),
            ))?
            .join(acquire_future)
            .then_execute(vulkan.queue.clone(), command_buffer)
            .map_err(|e| VulkanError::Other(e.into()))?
            .then_swapchain_present(
                vulkan.queue.clone(),
                SwapchainPresentInfo::swapchain_image_index(self.swapchain.clone(), image_num),
            )
            .then_signal_fence_and_flush();

        match future.map_err(Validated::unwrap) {
            Ok(mut future) => {
                if SETTINGS
                    .graphics
                    .cleanup
                    .swap(false, std::sync::atomic::Ordering::AcqRel)
                {
                    future.cleanup_finished();
                }
                self.previous_frame_end = Some(future.boxed());
            }
            Err(VulkanoError::OutOfDate) => {
                SETTINGS
                    .graphics
                    .recreate_swapchain
                    .store(true, std::sync::atomic::Ordering::Release);
                self.previous_frame_end = Some(sync::now(vulkan.device.clone()).boxed());
            }
            Err(e) => {
                self.previous_frame_end = Some(sync::now(vulkan.device.clone()).boxed());
                return Err(VulkanError::FlushFutureError(e.to_string()));
            }
        }
        Ok(())
    }

    pub fn mark_swapchain_outdated() {
        SETTINGS
            .graphics
            .recreate_swapchain
            .store(true, std::sync::atomic::Ordering::Release);
    }

    /// Redraws the scene.
    pub(crate) fn redraw_event(
        &mut self,
        #[cfg(feature = "egui")] gui: &mut egui_winit_vulkano::Gui,
    ) -> Result<(), VulkanError> {
        let resources = &RESOURCES;
        let mut loader = resources.loader().lock();

        let dimensions = self.window.inner_size();
        self.dimensions = [dimensions.x as u32, dimensions.y as u32];

        self.previous_frame_end
            .as_mut()
            .ok_or(VulkanError::Other(anyhow::Error::msg(
                "Could not obtain previous frame end.",
            )))?
            .cleanup_finished();

        if self.dimensions.contains(&0) {
            return Ok(());
        }

        Self::recreate_swapchain(self)?;

        let (image_num, suboptimal, acquire_future) =
            match acquire_next_image(self.swapchain.clone(), None).map_err(Validated::unwrap) {
                Ok(r) => r,
                Err(VulkanoError::OutOfDate) => {
                    Self::mark_swapchain_outdated();
                    return Err(VulkanError::SwapchainOutOfDate);
                }
                Err(e) => {
                    return Err(VulkanError::Validated(e));
                }
            };

        if suboptimal {
            Self::mark_swapchain_outdated();
        }

        let (mut builder, mut secondary_builder) = Self::make_command_buffer(
            self,
            image_num as usize,
            self.window.clear_color().rgba(),
            &loader,
        )?;

        Self::write_secondary_command_buffer(self, &mut secondary_builder, &mut loader)?;

        builder
            .execute_commands(secondary_builder.end()?)
            .map_err(|e| VulkanError::Other(e.into()))?;

        #[cfg(feature = "egui")]
        {
            // Creates and draws the second command buffer in case of egui.
            let cb = gui.draw_on_subpass_image(self.dimensions);
            builder
                .execute_commands(cb)
                .map_err(|e| VulkanError::Other(e.into()))?;
        }
        builder
            .end_render_pass(Default::default())
            .map_err(|e| VulkanError::Other(e.into()))?;
        let command_buffer = builder.end()?;

        Self::execute_command_buffer(self, command_buffer, acquire_future, image_num)?;
        Ok(())
    }
}
