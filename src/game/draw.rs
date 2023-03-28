use hashbrown::HashMap;
use parking_lot::Mutex;
use std::sync::Arc;
use vulkano::{
    buffer::{BufferUsage, CpuBufferPool},
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
    memory::allocator::{MemoryUsage, StandardMemoryAllocator},
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
    objects::{data::Vertex, Camera, Object},
    resources::Resources,
    vulkan::{window_size_dependent_setup, Vulkan},
};

use crate::{game::vulkan::shaders::*, game::Node};

#[allow(unused)]
pub struct Draw {
    pub recreate_swapchain: bool,
    descriptors: [Arc<PersistentDescriptorSet>; 3],
    pub previous_frame_end: Option<Box<dyn GpuFuture>>,
    vertex_buffer: CpuBufferPool<Vertex>,
    object_buffer: CpuBufferPool<vertexshader::ty::Object>,
    index_buffer: CpuBufferPool<u32>,
    camera_buffer: CpuBufferPool<Camera>,
    pub memoryallocator: Arc<StandardMemoryAllocator>,
    pub commandbufferallocator: StandardCommandBufferAllocator,
    descriptor_set_allocator: StandardDescriptorSetAllocator,
    texture_hash: HashMap<String, Arc<PersistentDescriptorSet>>,
}

impl Draw {
    pub fn setup(vulkan: &Vulkan) -> Self {
        let texture_hash = HashMap::new();

        let recreate_swapchain = false;

        let memoryallocator = Arc::new(StandardMemoryAllocator::new_default(vulkan.device.clone()));

        let vertex_buffer: CpuBufferPool<Vertex> =
            CpuBufferPool::vertex_buffer(memoryallocator.clone());

        let object_buffer: CpuBufferPool<vertexshader::ty::Object> = CpuBufferPool::new(
            memoryallocator.clone(),
            BufferUsage {
                uniform_buffer: true,
                ..Default::default()
            },
            MemoryUsage::Upload,
        );
        let index_buffer: CpuBufferPool<u32> = CpuBufferPool::new(
            memoryallocator.clone(),
            BufferUsage {
                index_buffer: true,
                ..Default::default()
            },
            MemoryUsage::Upload,
        );

        let camera_buffer: CpuBufferPool<Camera> = CpuBufferPool::new(
            memoryallocator.clone(),
            BufferUsage {
                uniform_buffer: true,
                ..Default::default()
            },
            MemoryUsage::Upload,
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
                [WriteDescriptorSet::buffer(
                    0,
                    object_buffer
                        .from_data(vertexshader::ty::Object {
                            color: [0.0, 0.0, 0.0, 0.0],
                            position: [0.0, 0.0],
                            size: [1.0, 1.0],
                            rotation: 0.0,
                            textureID: 0,
                            material: 0,
                        })
                        .unwrap(),
                )],
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
                [WriteDescriptorSet::buffer(
                    0,
                    camera_buffer.from_data(Camera::new()).unwrap(),
                )],
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
            previous_frame_end,
            vertex_buffer,
            object_buffer,
            index_buffer,
            camera_buffer,
            memoryallocator,
            commandbufferallocator,
            descriptor_set_allocator,
            texture_hash,
        }
    }

    pub fn update_font_objects(&mut self, vulkan: &Vulkan, resources: &Resources) {
        let dimensions = resources.cache.dimensions();

        let mut uploads = AutoCommandBufferBuilder::primary(
            &self.commandbufferallocator,
            vulkan.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let cache_texture = ImmutableImage::from_iter(
            &self.memoryallocator,
            resources
                .textures
                .get("fontatlas")
                .unwrap()
                .0
                .iter()
                .cloned(),
            ImageDimensions::Dim2d {
                width: dimensions.0,
                height: dimensions.1,
                array_layers: 1,
            },
            MipmapsCount::One,
            vulkano::format::Format::R8_UNORM,
            &mut uploads,
        )
        .unwrap();

        let cache_texture_view = ImageView::new(
            cache_texture.clone(),
            ImageViewCreateInfo {
                view_type: ImageViewType::Dim2dArray,
                ..ImageViewCreateInfo::from_image(&cache_texture)
            },
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

        self.previous_frame_end = Some(
            uploads
                .build()
                .unwrap()
                .execute(vulkan.queue.clone())
                .unwrap()
                .boxed(),
        );
        self.texture_hash.insert(
            "fontatlas".into(),
            PersistentDescriptorSet::new(
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
                    cache_texture_view.clone(),
                    sampler.clone(),
                )],
            )
            .unwrap(),
        );
    }
    pub fn update_textures(&mut self, vulkan: &Vulkan, resources: &Resources) {
        self.texture_hash = HashMap::new();

        let mut uploads = AutoCommandBufferBuilder::primary(
            &self.commandbufferallocator,
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

        for tex in resources.textures.clone().iter() {
            if tex.0 != "fontatlas" {
                let texture = {
                    let dimensions = ImageDimensions::Dim2d {
                        width: tex.1 .1 .0,
                        height: tex.1 .1 .1,
                        array_layers: 1, // 1 FOR NOW! WILL CHANGE WHEN TEXTURE ARRAY GETS ADDED TO THE THING!! OOG A BOOGA~~
                    };

                    let format = match tex.1 .3 {
                        1 => Format::R8_UNORM,
                        _ => Format::R8G8B8A8_UNORM,
                    };

                    let image = ImmutableImage::from_iter(
                        &self.memoryallocator,
                        tex.1 .0.clone().to_vec(),
                        dimensions,
                        MipmapsCount::One,
                        format,
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
                        texture.clone(),
                        sampler.clone(),
                    )],
                )
                .unwrap();
                self.texture_hash.insert(tex.0.to_string(), set);
            }
        }

        self.previous_frame_end = Some(
            uploads
                .build()
                .unwrap()
                .execute(vulkan.queue.clone())
                .unwrap()
                .boxed(),
        );
    }

    pub fn redrawevent(
        &mut self,
        vulkan: &mut Vulkan,
        objects: Vec<(
            Arc<Mutex<Node<Arc<Mutex<Object>>>>>,
            Option<Arc<Mutex<Node<Arc<Mutex<Object>>>>>>,
        )>,
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
                    clear_values: vec![Some([0.0, 0.0, 0.0, 0.0].into())],
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

        let push_constants = vertexshader::ty::PushConstant {
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
                    let mut descriptors = self.descriptors.clone();

                    descriptors[1] = PersistentDescriptorSet::new(
                        &self.descriptor_set_allocator,
                        vulkan
                            .pipeline
                            .layout()
                            .set_layouts()
                            .get(1)
                            .unwrap()
                            .clone(),
                        [WriteDescriptorSet::buffer(
                            0,
                            self.object_buffer
                                .from_data(vertexshader::ty::Object {
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
                                    textureID: if let Some(name) = &appearance.texture {
                                        descriptors[0] =
                                            self.texture_hash.get(&name.clone()).unwrap().clone();
                                        1
                                    } else {
                                        0
                                    },
                                    material: appearance.material,
                                })
                                .unwrap(),
                        )],
                    )
                    .unwrap();
                    descriptors[2] = PersistentDescriptorSet::new(
                        &self.descriptor_set_allocator,
                        vulkan
                            .pipeline
                            .layout()
                            .set_layouts()
                            .get(2)
                            .unwrap()
                            .clone(),
                        [WriteDescriptorSet::buffer(
                            0,
                            self.camera_buffer.from_data(camera).unwrap(),
                        )],
                    )
                    .unwrap();

                    let index_sub_buffer = self
                        .index_buffer
                        .from_iter(appearance.data.indices.clone())
                        .unwrap();
                    let vertex_sub_buffer = self
                        .vertex_buffer
                        .from_iter(appearance.data.vertices.clone())
                        .unwrap();
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
                        //.draw(appearance.data.vertices.len() as u32, 1, 0, 0)
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
