use parking_lot::Mutex;
use std::sync::Arc;
use vulkano::{
    buffer::{allocator::*, BufferContents, BufferUsage},
    command_buffer::{
        allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder, CommandBufferUsage,
        PrimaryCommandBufferAbstract, RenderPassBeginInfo, SubpassContents,
    },
    descriptor_set::{
        allocator::StandardDescriptorSetAllocator, PersistentDescriptorSet, WriteDescriptorSet,
    },
    format::Format,
    image::{
        view::{ImageView, ImageViewCreateInfo},
        ImageDimensions, ImageViewType, ImmutableImage, MipmapsCount, SwapchainImage
    },
    memory::allocator::StandardMemoryAllocator,
    pipeline::{
        Pipeline, 
        graphics::viewport::Viewport
    },
    render_pass::{Framebuffer, Subpass},
    sampler::Sampler,
    swapchain::{
        acquire_next_image, AcquireError, SwapchainCreateInfo, SwapchainCreationError,
        SwapchainPresentInfo, Swapchain,
    },
    sync::{self, FlushError, GpuFuture},
};
use winit::window::Window;

use super::{
    materials,
    objects::{data::*, Object},
    vulkan::{window_size_dependent_setup, Vulkan},
};

use crate::{game::Node, texture::Format as tFormat, texture::*};

use cgmath::{Deg, Matrix3, Matrix4, Ortho, Point3, Rad, Vector2, Vector3};

pub struct Draw {
    pub recreate_swapchain: bool,
    pub swapchain: Arc<Swapchain>,
    pub images: Vec<Arc<SwapchainImage>>,
    pub viewport: Viewport,
    pub framebuffers: Vec<Arc<Framebuffer>>,
    pub vertex_buffer_allocator: SubbufferAllocator,
    pub index_buffer_allocator: SubbufferAllocator,
    pub object_buffer_allocator: SubbufferAllocator,
    pub previous_frame_end: Option<Box<dyn GpuFuture>>,
    pub memoryallocator: Arc<StandardMemoryAllocator>,
    pub commandbufferallocator: StandardCommandBufferAllocator,
    descriptor_set_allocator: StandardDescriptorSetAllocator,
}

impl Draw {
    pub fn setup(vulkan: &Vulkan) -> Self {
        let recreate_swapchain = false;

        let (swapchain, images) = super::vulkan::swapchain::create_swapchain_and_images(&vulkan.device, &vulkan.surface);
        
        let mut viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [0.0, 0.0],
            depth_range: 0.0..1.0,
        };

        let framebuffers = window_size_dependent_setup(&images, vulkan.render_pass.clone(), &mut viewport);

        let memoryallocator = Arc::new(StandardMemoryAllocator::new_default(vulkan.device.clone()));

        let vertex_buffer_allocator: SubbufferAllocator = SubbufferAllocator::new(
            memoryallocator.clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
        );

        let index_buffer_allocator: SubbufferAllocator = SubbufferAllocator::new(
            memoryallocator.clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::INDEX_BUFFER,
                ..Default::default()
            },
        );

        let object_buffer_allocator: SubbufferAllocator = SubbufferAllocator::new(
            memoryallocator.clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::UNIFORM_BUFFER,
                ..Default::default()
            },
        );

        let commandbufferallocator =
            StandardCommandBufferAllocator::new(vulkan.device.clone(), Default::default());
        let descriptor_set_allocator = StandardDescriptorSetAllocator::new(vulkan.device.clone());

        let uploads = AutoCommandBufferBuilder::primary(
            &commandbufferallocator,
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
            vertex_buffer_allocator,
            index_buffer_allocator,
            object_buffer_allocator,
            previous_frame_end,
            memoryallocator,
            commandbufferallocator,
            descriptor_set_allocator,
        }
    }

    pub fn load_material(
        &mut self,
        vulkan: &Vulkan,
        settings: materials::MaterialSettings,
        descriptor_bindings: Vec<WriteDescriptorSet>,
    ) -> materials::Material {
        let subpass = Subpass::from(vulkan.render_pass.clone(), 0).unwrap();
        materials::Material::new(
            settings,
            descriptor_bindings,
            vulkan,
            subpass,
            &self.descriptor_set_allocator,
        )
    }

    pub fn load_texture(
        &mut self,
        vulkan: &Vulkan,
        data: Vec<u8>,
        dimensions: (u32, u32),
        layers: u32,
        format: tFormat,
        settings: TextureSettings,
    ) -> Arc<PersistentDescriptorSet> {
        let mut uploads = AutoCommandBufferBuilder::primary(
            &self.commandbufferallocator,
            vulkan.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let format = if settings.srgb {
            match format {
                tFormat::R8 => Format::R8_SRGB,
                tFormat::RGBA8 => Format::R8G8B8A8_SRGB,
                tFormat::RGBA16 => Format::R16G16B16A16_UNORM,
            }
        } else {
            match format {
                tFormat::R8 => Format::R8_UNORM,
                tFormat::RGBA8 => Format::R8G8B8A8_UNORM,
                tFormat::RGBA16 => Format::R16G16B16A16_UNORM,
            }
        };

        let image = ImmutableImage::from_iter(
            &self.memoryallocator,
            data,
            ImageDimensions::Dim2d {
                width: dimensions.0,
                height: dimensions.1,
                array_layers: layers,
            },
            MipmapsCount::One,
            format,
            &mut uploads,
        )
        .unwrap();
        
        let set_layout;

        let texture_view = ImageView::new(
            image.clone(),
            ImageViewCreateInfo {
                view_type: if layers == 1 {
                    set_layout = vulkan.
                        textured_material
                        .pipeline
                        .layout()
                        .set_layouts()
                        .get(1)
                        .unwrap()
                        .clone();
                    ImageViewType::Dim2d
                } else {
                    set_layout = vulkan.
                        texture_array_material
                        .pipeline
                        .layout()
                        .set_layouts()
                        .get(1)
                        .unwrap()
                        .clone();
                    ImageViewType::Dim2dArray
                },
                ..ImageViewCreateInfo::from_image(&image)
            },
        )
        .unwrap();

        let samplercreateinfo = settings.sampler.to_vulkano();

        let sampler = Sampler::new(vulkan.device.clone(), samplercreateinfo).unwrap();

        let set = PersistentDescriptorSet::new(
            &self.descriptor_set_allocator,
            set_layout,
            [WriteDescriptorSet::image_view_sampler(
                0,
                texture_view.clone(),
                sampler.clone(),
            )],
        )
        .unwrap();

        self.previous_frame_end = Some(
            uploads
                .build()
                .unwrap()
                .execute(vulkan.queue.clone())
                .unwrap()
                .boxed(),
        );
        set
    }

    pub fn redrawevent(
        &mut self,
        vulkan: &Vulkan,
        objects: &Vec<(
            Arc<Mutex<Node<Arc<Mutex<Object>>>>>,
            Option<Arc<Mutex<Node<Arc<Mutex<Object>>>>>>,
        )>,
        clear_color: [f32; 4],
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

        let mut builder = AutoCommandBufferBuilder::primary(
            &self.commandbufferallocator,
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
                SubpassContents::Inline,
            )
            .unwrap()
            .set_viewport(0, [self.viewport.clone()]);

        if suboptimal {
            self.recreate_swapchain = true;
        }

        for layer in objects.iter() {
            let mut order: Vec<Object> = vec![];

            Node::order_position(&mut order, &*layer.0.lock());

            for obj in order {
                if let Some(appearance) = obj.graphics.clone() {
                    if &appearance.data.vertices.len() == &0 {
                        continue;
                    }

                    let pipeline = if let Some(material) = &appearance.material {
                        material.pipeline.clone()
                    } else {
                        vulkan.default_material.pipeline.clone()
                    };

                    let mut descriptors = vec![];

                    let objectvert_sub_buffer =
                        self.object_buffer_allocator.allocate_sized().unwrap();
                    let objectfrag_sub_buffer =
                        self.object_buffer_allocator.allocate_sized().unwrap();

                    let translation = Matrix3::from_translation(Vector2::new(
                        obj.position[0] + appearance.position[0],
                        obj.position[1] + appearance.position[1],
                    ));
                    let rotation =
                        Matrix3::from_angle_z(Rad::from(Deg(obj.rotation + appearance.rotation)));
                    let scaling = Matrix3::from_nonuniform_scale(
                        obj.size[0] * appearance.size[0],
                        obj.size[1] * appearance.size[1],
                    );

                    let model = Matrix4::from_nonuniform_scale(1.0, 1.0, 1.0)
                        * Matrix4::from(rotation)
                        * Matrix4::from(scaling)
                        * Matrix4::from(translation);

                    let ortho;

                    let view = if let Some(camera) = &layer.1 {
                        let camera = camera.lock().get_object();

                        let rotation = Matrix4::from_angle_z(Rad::from(Deg(camera.rotation)));

                        let zoom = 1.0 / camera.camera.unwrap().zoom;
                        ortho = ortho_maker(
                            camera.camera.unwrap().mode,
                            camera.position,
                            zoom,
                            (dimensions.width as f32, dimensions.height as f32),
                        );

                        Matrix4::look_at_rh(
                            Point3::from([camera.position[0], camera.position[1], 1.0]),
                            Point3::from([camera.position[0], camera.position[1], 0.0]),
                            Vector3::unit_y(),
                        ) * rotation
                    } else {
                        ortho = Ortho {
                            left: -1.0,
                            right: 1.0,
                            bottom: 1.0,
                            top: -1.0,
                            near: -1.0,
                            far: 1.0,
                        };
                        Matrix4::look_at_rh(
                            Point3::from([0., 0., 0.]),
                            Point3::from([0., 0., 0.]),
                            Vector3::unit_y(),
                        )
                    };

                    let proj = Matrix4::from(ortho);

                    *objectvert_sub_buffer.write().unwrap() = ModelViewProj {
                        model: model.into(),
                        view: view.into(),
                        proj: proj.into(),
                    };
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

                    descriptors.insert(0, PersistentDescriptorSet::new(
                        &self.descriptor_set_allocator,
                        pipeline.layout().set_layouts().get(0).unwrap().clone(),
                        [
                            WriteDescriptorSet::buffer(0, objectvert_sub_buffer.clone()),
                            WriteDescriptorSet::buffer(1, objectfrag_sub_buffer.clone()),
                        ],
                    )
                    .unwrap());
                    
                    let vertex_sub_buffer = self
                        .vertex_buffer_allocator
                        .allocate_slice(appearance.data.vertices.clone().len() as _)
                        .unwrap();
                    let index_sub_buffer = self
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

                    builder
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
    #[allow(unused)]
    pub fn write_descriptor<T: BufferContents>(
        &self,
        descriptor: T,
        set: u32,
    ) -> WriteDescriptorSet {
        let buf = self.object_buffer_allocator.allocate_sized().unwrap();
        *buf.write().unwrap() = descriptor;
        WriteDescriptorSet::buffer(set, buf)
    }
}

fn ortho_maker(
    mode: CameraScaling,
    position: [f32; 2],
    zoom: f32,
    dimensions: (f32, f32),
) -> Ortho<f32> {
    match mode {
        CameraScaling::Stretch => Ortho {
            left: position[0] - zoom,
            right: position[0] + zoom,
            bottom: position[1] - zoom,
            top: position[1] + zoom,
            near: -1.0,
            far: 1.0,
        },
        CameraScaling::Linear => {
            let (width, height) = (
                0.5 / (dimensions.1 / (dimensions.0 + dimensions.1)),
                0.5 / (dimensions.0 / (dimensions.0 + dimensions.1)),
            );
            Ortho {
                left: position[0] - zoom * width,
                right: position[0] + zoom * width,
                bottom: position[1] - zoom * height,
                top: position[1] + zoom * height,
                near: -1.0,
                far: 1.0,
            }
        }
        CameraScaling::Circle => {
            let (width, height) = (
                1.0 / (dimensions.1.atan2(dimensions.0).sin() / 0.707106781),
                1.0 / (dimensions.1.atan2(dimensions.0).cos() / 0.707106781),
            );
            Ortho {
                left: position[0] - zoom * width,
                right: position[0] + zoom * width,
                bottom: position[1] - zoom * height,
                top: position[1] + zoom * height,
                near: -1.0,
                far: 1.0,
            }
        }
        CameraScaling::Limited => {
            let (width, height) = (
                1.0 / (dimensions.1 / dimensions.0.clamp(0.0, dimensions.1)),
                1.0 / (dimensions.0 / dimensions.1.clamp(0.0, dimensions.0)),
            );
            Ortho {
                left: position[0] - zoom * width,
                right: position[0] + zoom * width,
                bottom: position[1] - zoom * height,
                top: position[1] + zoom * height,
                near: -1.0,
                far: 1.0,
            }
        }
        CameraScaling::Expand => {
            let (width, height) = (dimensions.0 * 0.001, dimensions.1 * 0.001);
            Ortho {
                left: position[0] - zoom * width,
                right: position[0] + zoom * width,
                bottom: position[1] - zoom * height,
                top: position[1] + zoom * height,
                near: -1.0,
                far: 1.0,
            }
        }
    }
}
