use parking_lot::Mutex;
use std::sync::Arc;
use vulkano::{
    buffer::{allocator::*, BufferUsage},
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
        ImageDimensions, ImageViewType, ImmutableImage, MipmapsCount,
    },
    memory::allocator::StandardMemoryAllocator,
    pipeline::Pipeline,
    sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo},
    swapchain::{
        acquire_next_image, AcquireError, SwapchainCreateInfo, SwapchainCreationError,
        SwapchainPresentInfo,
    },
    sync::{self, FlushError, GpuFuture},
};
use winit::window::Window;

use super::{
    objects::{data::*, Object},
    vulkan::{window_size_dependent_setup, Vulkan},
};

use crate::{game::Node, texture::Format as tFormat, texture::*};

#[allow(unused)]
pub struct Draw {
    pub recreate_swapchain: bool,
    descriptors: [Arc<PersistentDescriptorSet>; 3],
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

        let mut uploads = AutoCommandBufferBuilder::primary(
            &commandbufferallocator,
            vulkan.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let sampler = Sampler::new(
            vulkan.device.clone(),
            SamplerCreateInfo {
                mag_filter: Filter::Nearest,
                min_filter: Filter::Linear,
                address_mode: [
                    SamplerAddressMode::ClampToBorder,
                    SamplerAddressMode::ClampToBorder,
                    SamplerAddressMode::Repeat,
                ],
                ..Default::default()
            },
        )
        .unwrap();

        //placeholder texture
        let texture = {
            let texture = vec![0, 0, 0, 255];
            let dimensions = ImageDimensions::Dim2d {
                width: 1,
                height: 1,
                array_layers: 1,
            };

            let image = ImmutableImage::from_iter(
                &memoryallocator,
                texture,
                dimensions,
                MipmapsCount::One,
                Format::R8G8B8A8_SRGB,
                &mut uploads,
            )
            .unwrap();
            ImageView::new(
                image.clone(),
                ImageViewCreateInfo {
                    view_type: ImageViewType::Dim2dArray,
                    ..ImageViewCreateInfo::from_image(&image)
                },
            )
            .unwrap()
        };

        let object_sub_buffer = object_buffer_allocator.allocate_sized().unwrap();
        let camera_sub_buffer = object_buffer_allocator.allocate_sized().unwrap();

        *object_sub_buffer.write().unwrap() = DrawObject::default();
        *camera_sub_buffer.write().unwrap() = Camera::new();

        let descriptors = [
            PersistentDescriptorSet::new(
                &descriptor_set_allocator,
                vulkan
                    .pipeline
                    .layout()
                    .set_layouts()
                    .get(0)
                    .unwrap()
                    .clone(),
                [WriteDescriptorSet::image_view_sampler(
                    0,
                    texture.clone(),
                    sampler.clone(),
                )],
            )
            .unwrap(),
            PersistentDescriptorSet::new(
                &descriptor_set_allocator,
                vulkan
                    .pipeline
                    .layout()
                    .set_layouts()
                    .get(1)
                    .unwrap()
                    .clone(),
                [WriteDescriptorSet::buffer(0, object_sub_buffer.clone())],
            )
            .unwrap(),
            PersistentDescriptorSet::new(
                &descriptor_set_allocator,
                vulkan
                    .pipeline
                    .layout()
                    .set_layouts()
                    .get(2)
                    .unwrap()
                    .clone(),
                [WriteDescriptorSet::buffer(0, camera_sub_buffer.clone())],
            )
            .unwrap(),
        ];

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
            descriptors,
            vertex_buffer_allocator,
            index_buffer_allocator,
            object_buffer_allocator,
            previous_frame_end,
            memoryallocator,
            commandbufferallocator,
            descriptor_set_allocator,
        }
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

        let texture_view = ImageView::new(
            image.clone(),
            ImageViewCreateInfo {
                view_type: ImageViewType::Dim2dArray,
                ..ImageViewCreateInfo::from_image(&image)
            },
        )
        .unwrap();

        let samplercreateinfo = settings.sampler.to_vulkano();

        let sampler = Sampler::new(vulkan.device.clone(), samplercreateinfo).unwrap();

        let set = PersistentDescriptorSet::new(
            &self.descriptor_set_allocator,
            vulkan
                .pipeline
                .layout()
                .set_layouts()
                .get(0)
                .unwrap()
                .clone(),
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
        vulkan: &mut Vulkan,
        objects: Vec<(
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
            let (new_swapchain, new_images) = match vulkan.swapchain.recreate(SwapchainCreateInfo {
                image_extent: dimensions.into(),
                ..vulkan.swapchain.create_info()
            }) {
                Ok(r) => r,
                Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return,
                Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
            };

            vulkan.swapchain = new_swapchain;
            vulkan.framebuffers = window_size_dependent_setup(
                &new_images,
                vulkan.render_pass.clone(),
                &mut vulkan.viewport,
            );
            self.recreate_swapchain = false;
        }

        let (image_num, suboptimal, acquire_future) =
            match acquire_next_image(vulkan.swapchain.clone(), None) {
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
                        vulkan.framebuffers[image_num as usize].clone(),
                    )
                },
                SubpassContents::Inline,
            )
            .unwrap()
            .set_viewport(0, [vulkan.viewport.clone()])
            .bind_pipeline_graphics(vulkan.pipeline.clone());

        if suboptimal {
            self.recreate_swapchain = true;
        }

        //buffer updates

        let push_constants = PushConstant {
            resolution: [dimensions.width as f32, dimensions.height as f32],
        };

        //Draw Objects

        for layer in objects.iter() {
            let camera = if let Some(camera) = &layer.1 {
                let camera = camera.lock().get_object();
                Camera {
                    position: camera.position,
                    rotation: camera.rotation,
                    zoom: camera.camera.unwrap_or_default().zoom,
                    mode: camera.camera.unwrap_or_default().mode as u32,
                }
            } else {
                Camera::new()
            };

            let mut order: Vec<Object> = vec![];

            Node::order_position(&mut order, &*layer.0.lock());

            for obj in order {
                if let Some(appearance) = obj.graphics.clone() {
                    if &appearance.data.vertices.len() == &0 {
                        continue
                    }
                    let mut descriptors = self.descriptors.clone();
                    let object_sub_buffer = self.object_buffer_allocator.allocate_sized().unwrap();
                    let camera_sub_buffer = self.object_buffer_allocator.allocate_sized().unwrap();

                    *object_sub_buffer.write().unwrap() = DrawObject {
                        color: appearance.color,
                        position: [
                            obj.position[0] + appearance.position[0],
                            obj.position[1] + appearance.position[1],
                        ],
                        size: [
                            obj.size[0] * appearance.size[0],
                            obj.size[1] * appearance.size[1],
                        ],
                        rotation: obj.rotation + appearance.rotation,
                        texture_id: if let Some(texture) = &appearance.texture {
                            descriptors[0] = texture.set.clone();
                            appearance.texture_id
                        } else {
                            0
                        },
                        material: if let Some(texture) = &appearance.texture {
                            texture.material
                        } else {
                            0
                        },
                    };

                    descriptors[1] = PersistentDescriptorSet::new(
                        &self.descriptor_set_allocator,
                        vulkan
                            .pipeline
                            .layout()
                            .set_layouts()
                            .get(1)
                            .unwrap()
                            .clone(),
                        [WriteDescriptorSet::buffer(0, object_sub_buffer.clone())],
                    )
                    .unwrap();
                    *camera_sub_buffer.write().unwrap() = camera;
                    descriptors[2] = PersistentDescriptorSet::new(
                        &self.descriptor_set_allocator,
                        vulkan
                            .pipeline
                            .layout()
                            .set_layouts()
                            .get(2)
                            .unwrap()
                            .clone(),
                        [WriteDescriptorSet::buffer(0, camera_sub_buffer.clone())],
                    )
                    .unwrap();

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
                        .bind_descriptor_sets(
                            vulkano::pipeline::PipelineBindPoint::Graphics,
                            vulkan.pipeline.layout().clone(),
                            0,
                            descriptors.to_vec(),
                        )
                        .bind_vertex_buffers(0, vertex_sub_buffer.clone())
                        .bind_index_buffer(index_sub_buffer.clone())
                        .push_constants(vulkan.pipeline.layout().clone(), 0, push_constants)
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
                SwapchainPresentInfo::swapchain_image_index(vulkan.swapchain.clone(), image_num),
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
