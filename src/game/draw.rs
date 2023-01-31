use std::sync::Arc;
use vulkano::{
    buffer::{BufferUsage, CpuBufferPool},
    command_buffer::{
        allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder, CommandBufferUsage,
        PrimaryCommandBufferAbstract, RenderPassBeginInfo, SubpassContents,
    },
    descriptor_set::{allocator::StandardDescriptorSetAllocator, PersistentDescriptorSet, WriteDescriptorSet},
    memory::allocator::{MemoryUsage, StandardMemoryAllocator},
    sync::{GpuFuture, self, FlushError}, swapchain::{SwapchainCreationError, AcquireError, acquire_next_image, SwapchainCreateInfo, SwapchainPresentInfo}, pipeline::Pipeline, image::{ImmutableImage, MipmapsCount, view::ImageView, ImageDimensions}, sampler::{SamplerCreateInfo, Filter, SamplerAddressMode, Sampler},
};
use winit::window::Window;

use super::{
    objects::{data::Vertex, Object},
    vulkan::{Vulkan, window_size_dependent_setup}, resources::Resources,
};

use crate::game::vulkan::shaders::*;

pub struct Draw {
    recreate_swapchain: bool,
    descriptors: [Arc<PersistentDescriptorSet>; 2],
    previous_frame_end: Option<Box<dyn GpuFuture>>,
    vertex_buffer: CpuBufferPool<Vertex>,
    object_buffer: CpuBufferPool<vertexshader::ty::Object>,
    index_buffer: CpuBufferPool<u16>,
    memoryallocator: Arc<StandardMemoryAllocator>,
    commandbufferallocator: StandardCommandBufferAllocator,
    descriptor_set_allocator: StandardDescriptorSetAllocator,
}

impl Draw {
    pub fn setup(vulkan: &Vulkan, resources: &Resources) -> Self {
        let recreate_swapchain = false;

        let memoryallocator = Arc::new(StandardMemoryAllocator::new_default(vulkan.device.clone()));

        let vertex_buffer: CpuBufferPool<Vertex> =
            CpuBufferPool::vertex_buffer(memoryallocator.clone().into());

        let object_buffer: CpuBufferPool<vertexshader::ty::Object> = CpuBufferPool::new(
            memoryallocator.clone().into(),
            BufferUsage {
                uniform_buffer: true,
                ..Default::default()
            },
            MemoryUsage::Upload,
        );

        let index_buffer: CpuBufferPool<u16> = CpuBufferPool::new(
            memoryallocator.clone().into(),
            BufferUsage {
                index_buffer: true,
                ..Default::default()
            },
            vulkano::memory::allocator::MemoryUsage::Upload,
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

        

        // placeholder texture
        let texture = {
            // let texture = 
            //     resources
            //     .textures
            //     .get("rusty")
            //     .unwrap()
            //     .as_ref()
            //     .clone();
            let texture: (Vec<u8>, ImageDimensions) = (
                vec![0, 0, 0, 255],
                ImageDimensions::Dim2d { width: 1, height: 1, array_layers: 1 }
            );

            let image = ImmutableImage::from_iter(
                &memoryallocator,
                texture.0,
                texture.1,
                MipmapsCount::One,
                vulkano::format::Format::R8G8B8A8_SRGB,
                &mut uploads,
            )
            .unwrap();
            ImageView::new_default(image).unwrap()
        };

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

        let descriptors = [
            PersistentDescriptorSet::new(
                &descriptor_set_allocator,
                vulkan.pipeline.layout().set_layouts().get(0).unwrap().clone(),
                [WriteDescriptorSet::image_view_sampler(
                    0,
                    texture.clone(),
                    sampler.clone(),
                )],
            )
            .unwrap(),
            PersistentDescriptorSet::new(
                &descriptor_set_allocator,
                vulkan.pipeline.layout().set_layouts().get(1).unwrap().clone(),
                [WriteDescriptorSet::buffer(
                    0,
                    object_buffer
                        .from_data(vertexshader::ty::Object {
                            color: [0.0, 0.0, 0.0, 0.0],
                            position: [0.0, 0.0],
                            size: [1.0, 1.0],
                            rotation: 0.0,
                            textureID: 0,
                        })
                        .unwrap(),
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
            memoryallocator,
            commandbufferallocator,
            descriptor_set_allocator,
        }
    }

    pub fn redrawevent(&mut self, vulkan: &mut Vulkan, objects: &Vec<Arc<Object>>) {
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
                    clear_values: vec![Some([0.0, 0.0, 0.0, 1.0].into())],
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
            camera: [0.0, 0.0],
        };

        //Draw Objects
        for obj in objects.iter() {
            self.descriptors[1] = PersistentDescriptorSet::new(
                &self.descriptor_set_allocator,
                vulkan.pipeline.layout().set_layouts().get(1).unwrap().clone(),
                [WriteDescriptorSet::buffer(
                    0,
                    self.object_buffer
                        .from_data(vertexshader::ty::Object {
                            color: obj.color,
                            position: obj.position,
                            size: obj.size,
                            rotation: obj.rotation,
                            textureID: if let Some(_) = obj.texture { 1 } else { 0 },
                        })
                        .unwrap(),
                )],
            )
            .unwrap();

            let index_sub_buffer = self
                .index_buffer
                .from_iter(obj.data.indices.clone())
                .unwrap();
            let vertex_sub_buffer = self
                .vertex_buffer
                .from_iter(obj.data.vertices.clone())
                .unwrap();
            builder
                .bind_descriptor_sets(
                    vulkano::pipeline::PipelineBindPoint::Graphics,
                    vulkan.pipeline.layout().clone(),
                    0,
                    self.descriptors.to_vec(),
                )
                .bind_vertex_buffers(0, vertex_sub_buffer.clone())
                .bind_index_buffer(index_sub_buffer.clone())
                .push_constants(vulkan.pipeline.layout().clone(), 0, push_constants)
                .draw(obj.data.vertices.len() as u32, 1, 0, 0)
                .unwrap();
        }
        // //Draw Fonts
        // // let text = "Mein Kater Rusty";
        

        // builder
        //     .bind_pipeline_graphics(self.text_pipeline.clone())
        //     .bind_vertex_buffers(0, [self.text_vertex_buffer.clone()])
        //     .bind_descriptor_sets(
        //         vulkano::pipeline::PipelineBindPoint::Graphics,
        //         self.text_pipeline.layout().clone(),
        //         0,
        //         self.text_set.clone(),
        //     )
        //     .draw(self.text_vertices.clone().len() as u32, 1, 0, 0)
        //     .unwrap();

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
