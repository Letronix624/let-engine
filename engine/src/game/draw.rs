use std::sync::Arc;
use vulkano::{
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferInheritanceInfo, CommandBufferUsage,
        PrimaryAutoCommandBuffer, PrimaryCommandBufferAbstract, RenderPassBeginInfo,
        SecondaryAutoCommandBuffer, SubpassContents,
    },
    descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet},
    pipeline::{graphics::viewport::Viewport, GraphicsPipeline, Pipeline},
    render_pass::Framebuffer,
    swapchain::{
        acquire_next_image, AcquireError, Swapchain, SwapchainAcquireFuture, SwapchainCreateInfo,
        SwapchainCreationError, SwapchainPresentInfo,
    },
    sync::{self, FlushError, GpuFuture},
};

use super::{
    resources::vulkan::{
        swapchain::create_swapchain_and_images, window_size_dependent_setup, Vulkan,
    },
    Loader,
};

use crate::{
    camera::CameraSettings, game::Node, objects::VisualObject, resources::data::*,
    resources::Resources, utils, Object,
};

//use cgmath::{Deg, Matrix3, Matrix4, Ortho, Point3, Rad, Vector3};
use glam::f32::{Mat4, Quat, Vec3};

/// Responsible for drawing on the surface.
pub struct Draw {
    pub(crate) recreate_swapchain: bool,
    pub(crate) swapchain: Arc<Swapchain>,
    pub(crate) viewport: Viewport,
    pub(crate) framebuffers: Vec<Arc<Framebuffer>>,
    pub(crate) previous_frame_end: Option<Box<dyn GpuFuture>>,
    dimensions: [u32; 2],
    default_material: Arc<GraphicsPipeline>,
}

impl Draw {
    pub(crate) fn setup(resources: &Resources) -> Self {
        let vulkan = resources.vulkan();
        let loader = resources.loader().lock();

        let recreate_swapchain = false;

        let (swapchain, images) = create_swapchain_and_images(&vulkan.device, &vulkan.surface);

        let mut viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [0.0, 0.0],
            depth_range: 0.0..1.0,
        };

        let framebuffers =
            window_size_dependent_setup(&images, vulkan.render_pass.clone(), &mut viewport);

        let uploads = AutoCommandBufferBuilder::primary(
            &loader.command_buffer_allocator,
            vulkan.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let previous_frame_end = Some(
            uploads
                .build()
                .unwrap()
                .execute(vulkan.queue.clone())
                .unwrap()
                .boxed(),
        );

        let dimensions = [0; 2];

        let default_material = vulkan.default_material.pipeline.clone();

        Self {
            recreate_swapchain,
            swapchain,
            viewport,
            framebuffers,
            previous_frame_end,
            dimensions,
            default_material,
        }
    }

    /// Recreates the swapchain in case it's out of date if someone for example changed the scene size or window dimensions.
    fn recreate_swapchain(&mut self, vulkan: &Vulkan) {
        if self.recreate_swapchain {
            let (new_swapchain, new_images) = match self.swapchain.recreate(SwapchainCreateInfo {
                image_extent: self.dimensions,
                ..self.swapchain.create_info()
            }) {
                Ok(r) => r,
                Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return,
                Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
            };

            self.swapchain = new_swapchain;
            self.framebuffers = window_size_dependent_setup(
                &new_images,
                vulkan.render_pass.clone(),
                &mut self.viewport,
            );
            self.recreate_swapchain = false;
        }
    }

    /// Makes a primary and secondary command buffer already inside a render pass.
    fn make_command_buffer(
        &self,
        vulkan: &Vulkan,
        loader: &Loader,
        image_num: usize,
        clear_color: [f32; 4],
    ) -> (
        AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        AutoCommandBufferBuilder<SecondaryAutoCommandBuffer>,
    ) {
        let mut builder = AutoCommandBufferBuilder::primary(
            &loader.command_buffer_allocator,
            vulkan.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        // Makes a commandbuffer that takes multiple secondary buffers.
        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![Some(clear_color.into())],
                    ..RenderPassBeginInfo::framebuffer(self.framebuffers[image_num].clone())
                },
                SubpassContents::SecondaryCommandBuffers,
            )
            .unwrap();

        let mut secondary_builder = AutoCommandBufferBuilder::secondary(
            &loader.command_buffer_allocator,
            vulkan.queue.queue_family_index(),
            CommandBufferUsage::MultipleSubmit,
            CommandBufferInheritanceInfo {
                render_pass: Some(vulkan.subpass.clone().into()),
                ..Default::default()
            },
        )
        .unwrap();
        secondary_builder.set_viewport(0, [self.viewport.clone()]);

        (builder, secondary_builder)
    }

    fn make_mvp_matrix(
        object: &VisualObject,
        dimensions: [u32; 2],
        camera: &Object,
        camera_settings: CameraSettings,
    ) -> ModelViewProj {
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
            (dimensions[0] as f32, dimensions[1] as f32),
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
        ModelViewProj { model, view, proj }
    }

    /// Draws the Game Scene on the given command buffer.
    fn write_secondary_command_buffer(
        &self,
        scene: &crate::Scene,
        command_buffer: &mut AutoCommandBufferBuilder<SecondaryAutoCommandBuffer>,
        loader: &Loader,
    ) {
        for layer in scene.get_layers().iter() {
            let mut order: Vec<VisualObject> = vec![];

            Node::order_position(&mut order, &layer.root.lock());

            for object in order {
                let appearance = &object.appearance;
                // Skip drawing the object if the object is not marked visible or has no vertices.
                if !appearance.get_visible() || appearance.get_model().is_none() {
                    continue;
                }

                // The pipeline of the current object. Takes the default one if there is none.
                let pipeline = if let Some(material) = appearance.get_material() {
                    material.pipeline.clone()
                } else {
                    self.default_material.clone()
                };

                let mut descriptors = vec![];

                // MVP matrix for the object
                let objectvert_sub_buffer =
                    loader.object_buffer_allocator.allocate_sized().unwrap();
                // Simple color and texture data for the fragment shader.
                let objectfrag_sub_buffer =
                    loader.object_buffer_allocator.allocate_sized().unwrap();

                *objectvert_sub_buffer.write().unwrap() = Self::make_mvp_matrix(
                    &object,
                    self.dimensions,
                    &layer.camera.lock().lock().object,
                    layer.camera_settings(),
                );
                *objectfrag_sub_buffer.write().unwrap() = ObjectFrag {
                    color: *appearance.get_color(),
                    texture_id: if let Some(material) = appearance.get_material() {
                        if let Some(texture) = &material.texture {
                            descriptors.push(texture.set().clone());
                        }
                        if let Some(descriptor) = &material.descriptor {
                            descriptors.push(descriptor.clone());
                        }
                        material.layer
                    } else {
                        0
                    },
                };

                descriptors.insert(
                    0,
                    PersistentDescriptorSet::new(
                        &loader.descriptor_set_allocator,
                        pipeline.layout().set_layouts().get(0).unwrap().clone(),
                        [
                            WriteDescriptorSet::buffer(0, objectvert_sub_buffer.clone()),
                            WriteDescriptorSet::buffer(1, objectfrag_sub_buffer.clone()),
                        ],
                    )
                    .unwrap(),
                );

                let model = appearance.get_model().as_ref().unwrap();

                command_buffer
                    .bind_pipeline_graphics(pipeline.clone())
                    .bind_descriptor_sets(
                        vulkano::pipeline::PipelineBindPoint::Graphics,
                        pipeline.layout().clone(),
                        0,
                        descriptors,
                    )
                    .bind_vertex_buffers(0, model.get_vertex_buffer())
                    .bind_index_buffer(model.get_index_buffer())
                    .draw_indexed(model.get_size() as u32, 1, 0, 0, 0)
                    .unwrap();
            }
        }
    }

    /// Creates and executes a future in which the command buffer gets executed.
    fn execute_command_buffer(
        &mut self,
        command_buffer: PrimaryAutoCommandBuffer,
        acquire_future: SwapchainAcquireFuture,
        image_num: u32,
        vulkan: &Vulkan,
    ) {
        let future = self
            .previous_frame_end
            .take()
            .unwrap()
            .join(acquire_future)
            .then_execute(vulkan.queue.clone(), command_buffer)
            .unwrap()
            .then_swapchain_present(
                vulkan.queue.clone(),
                SwapchainPresentInfo::swapchain_image_index(self.swapchain.clone(), image_num),
            )
            .then_signal_fence_and_flush();

        match future {
            Ok(future) => {
                self.previous_frame_end = Some(future.boxed());
            }
            Err(FlushError::OutOfDate) => {
                self.recreate_swapchain = true;
                self.previous_frame_end = Some(sync::now(vulkan.device.clone()).boxed());
            }
            Err(e) => {
                println!("Failed to flush future: {:?}", e);
                self.previous_frame_end = Some(sync::now(vulkan.device.clone()).boxed());
            }
        }
    }

    /// Redraws the scene.
    pub(crate) fn redrawevent(
        &mut self,
        resources: &Resources,
        scene: &crate::Scene,
        #[cfg(feature = "egui")] gui: &mut egui_winit_vulkano::Gui,
    ) {
        let vulkan = resources.vulkan();
        let loader = resources.loader().as_ref().lock();

        let window = &vulkan.window;

        self.dimensions = window.inner_size().into();

        self.previous_frame_end.as_mut().unwrap().cleanup_finished();

        if self.dimensions.contains(&0) {
            return;
        }

        Self::recreate_swapchain(self, resources.vulkan());

        let (image_num, suboptimal, acquire_future) =
            match acquire_next_image(self.swapchain.clone(), None) {
                Ok(r) => r,
                Err(AcquireError::OutOfDate) => {
                    self.recreate_swapchain = true;
                    return;
                }
                Err(e) => panic!("Failed to acquire next image: {:?}", e),
            };

        if suboptimal {
            self.recreate_swapchain = true;
        }

        let (mut builder, mut secondary_builder) = Self::make_command_buffer(
            self,
            vulkan,
            &loader,
            image_num as usize,
            window.clear_color(),
        );

        Self::write_secondary_command_buffer(self, scene, &mut secondary_builder, &loader);

        builder
            .execute_commands(secondary_builder.build().unwrap())
            .unwrap();

        #[cfg(feature = "egui")]
        {
            // Creates and draws the second command buffer in case of egui.
            let cb = gui.draw_on_subpass_image(self.dimensions);
            builder.execute_commands(cb).unwrap();
        }
        builder.end_render_pass().unwrap();
        let command_buffer = builder.build().unwrap();

        Self::execute_command_buffer(self, command_buffer, acquire_future, image_num, vulkan);
    }
}
