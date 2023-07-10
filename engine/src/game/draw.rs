use std::sync::Arc;
use vulkano::{
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferInheritanceInfo, CommandBufferUsage,
        PrimaryCommandBufferAbstract, RenderPassBeginInfo, SubpassContents,
    },
    descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet},
    image::SwapchainImage,
    pipeline::{graphics::viewport::Viewport, Pipeline},
    render_pass::Framebuffer,
    swapchain::{
        acquire_next_image, AcquireError, Swapchain, SwapchainCreateInfo, SwapchainCreationError,
        SwapchainPresentInfo,
    },
    sync::{self, FlushError, GpuFuture},
};
use winit::window::Window;

use super::{
    objects::{data::*, Object},
    vulkan::{window_size_dependent_setup, Vulkan},
    Loader,
};

use crate::game::Node;

//use cgmath::{Deg, Matrix3, Matrix4, Ortho, Point3, Rad, Vector3};
use glam::f32::{Mat4, Quat, Vec2, Vec3};

pub struct Draw {
    pub recreate_swapchain: bool,
    pub swapchain: Arc<Swapchain>,
    pub images: Vec<Arc<SwapchainImage>>,
    pub viewport: Viewport,
    pub framebuffers: Vec<Arc<Framebuffer>>,
    pub previous_frame_end: Option<Box<dyn GpuFuture>>,
}

impl Draw {
    pub fn setup(vulkan: &Vulkan, loader: &Loader) -> Self {
        let recreate_swapchain = false;

        let (swapchain, images) =
            super::vulkan::swapchain::create_swapchain_and_images(&vulkan.device, &vulkan.surface);

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
        Self {
            recreate_swapchain,
            swapchain,
            images,
            viewport,
            framebuffers,
            previous_frame_end,
        }
    }

    pub fn redrawevent(
        &mut self,
        vulkan: &Vulkan,
        loader: &mut Loader,
        scene: &super::Scene,
        clear_color: [f32; 4],
        #[cfg(feature = "egui")] gui: &mut egui_winit_vulkano::Gui,
    ) {
        //windowevents
        let window = vulkan
            .surface
            .object()
            .unwrap()
            .downcast_ref::<Window>()
            .unwrap();
        let dimensions = window.inner_size();

        self.previous_frame_end.as_mut().unwrap().cleanup_finished();

        if dimensions.width == 0 || dimensions.height == 0 {
            return;
        }

        if self.recreate_swapchain {
            let (new_swapchain, new_images) = match self.swapchain.recreate(SwapchainCreateInfo {
                image_extent: dimensions.into(),
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

        let (image_num, suboptimal, acquire_future) =
            match acquire_next_image(self.swapchain.clone(), None) {
                Ok(r) => r,
                Err(AcquireError::OutOfDate) => {
                    self.recreate_swapchain = true;
                    return;
                }
                Err(e) => panic!("Failed to acquire next image: {:?}", e),
            };

        let dimensions = self.framebuffers[image_num as usize].extent();

        if suboptimal {
            self.recreate_swapchain = true;
        }

        let mut builder = AutoCommandBufferBuilder::primary(
            &loader.command_buffer_allocator,
            vulkan.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![Some(clear_color.into())],
                    ..RenderPassBeginInfo::framebuffer(
                        self.framebuffers[image_num as usize].clone(),
                    )
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

        for layer in scene.get_layers().iter() {
            let mut order: Vec<Object> = vec![];

            Node::order_position(&mut order, &layer.root.lock());

            for obj in order {
                let transform = obj.transform.combine(obj.appearance.transform);
                let appearance = &obj.appearance;
                if !appearance.visible || appearance.data.vertices.is_empty() {
                    continue;
                }

                let pipeline = if let Some(material) = &appearance.material {
                    material.pipeline.clone()
                } else {
                    vulkan.default_material.pipeline.clone()
                };

                let mut descriptors = vec![];

                let objectvert_sub_buffer =
                    loader.object_buffer_allocator.allocate_sized().unwrap();
                let objectfrag_sub_buffer =
                    loader.object_buffer_allocator.allocate_sized().unwrap();

                let scaling = Vec3::new(transform.size[0], transform.size[1], 0.0);
                let rotation = Quat::from_rotation_z(transform.rotation);
                let translation = Vec3::new(transform.position[0], transform.position[1], 0.0);

                let model = Mat4::from_scale_rotation_translation(scaling, rotation, translation);

                let proj;

                let view = if let Some(camera) = layer.camera.lock().as_ref() {
                    let rotation = Mat4::from_rotation_z(camera.transform().rotation);

                    let zoom = 1.0 / camera.settings().zoom;
                    proj = ortho_maker(
                        camera.settings().mode,
                        camera.transform().position,
                        zoom,
                        (dimensions[0] as f32, dimensions[1] as f32),
                    );

                    Mat4::look_at_rh(
                        Vec3::from([
                            camera.transform().position[0],
                            camera.transform().position[1],
                            1.0,
                        ]),
                        Vec3::from([
                            camera.transform().position[0],
                            camera.transform().position[1],
                            0.0,
                        ]),
                        Vec3::Y,
                    ) * rotation
                } else {
                    proj = Mat4::orthographic_rh(-1.0, 1.0, 1.0, -1.0, -1.0, 1.0);
                    Mat4::look_at_rh(Vec3::from([0., 0., 0.]), Vec3::from([0., 0., 0.]), Vec3::Y)
                };

                *objectvert_sub_buffer.write().unwrap() = ModelViewProj { model, view, proj };
                *objectfrag_sub_buffer.write().unwrap() = ObjectFrag {
                    color: appearance.color,
                    texture_id: if let Some(material) = &appearance.material {
                        if let Some(texture) = &material.texture {
                            descriptors.push(texture.set.clone());
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

                let vertex_sub_buffer = loader
                    .vertex_buffer_allocator
                    .allocate_slice(appearance.data.vertices.clone().len() as _)
                    .unwrap();
                let index_sub_buffer = loader
                    .index_buffer_allocator
                    .allocate_slice(appearance.data.indices.clone().len() as _)
                    .unwrap();

                vertex_sub_buffer
                    .write()
                    .unwrap()
                    .copy_from_slice(&appearance.data.vertices);
                index_sub_buffer
                    .write()
                    .unwrap()
                    .copy_from_slice(&appearance.data.indices);

                secondary_builder
                    .bind_pipeline_graphics(pipeline.clone())
                    .bind_descriptor_sets(
                        vulkano::pipeline::PipelineBindPoint::Graphics,
                        pipeline.layout().clone(),
                        0,
                        descriptors,
                    )
                    .bind_vertex_buffers(0, vertex_sub_buffer.clone())
                    .bind_index_buffer(index_sub_buffer.clone())
                    .draw_indexed(appearance.data.indices.len() as u32, 1, 0, 0, 0)
                    .unwrap();
            }
        }
        builder
            .execute_commands(secondary_builder.build().unwrap())
            .unwrap();

        #[cfg(feature = "egui")]
        {
            let cb = gui.draw_on_subpass_image(dimensions);
            builder.execute_commands(cb).unwrap();
        }
        builder.end_render_pass().unwrap();
        let command_buffer = builder.build().unwrap();

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
}

fn ortho_maker(mode: CameraScaling, position: Vec2, zoom: f32, dimensions: (f32, f32)) -> Mat4 {
    let (width, height) = super::objects::scale(mode, dimensions);
    Mat4::orthographic_rh(
        position.x - zoom * width,
        position.x + zoom * width,
        position.y - zoom * height,
        position.y + zoom * height,
        -1.0,
        1.0,
    )
}
