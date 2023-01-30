extern crate image;
extern crate vulkano;
use crate::data::*;
use crate::Object;
use crate::GAME;
use rusttype::gpu_cache::Cache;
use rusttype::{point, PositionedGlyph, Rect, Scale};
use std::collections::HashMap;
use std::sync::Arc;
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer, CpuBufferPool};
use vulkano::command_buffer::{
    allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder, CommandBufferUsage,
    PrimaryCommandBufferAbstract, RenderPassBeginInfo, SubpassContents,
};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::DeviceExtensions;
use vulkano::device::{Device, Queue};
use vulkano::image::{view::ImageView, ImageAccess, SwapchainImage};
use vulkano::image::{ImageDimensions, ImmutableImage, MipmapsCount};
use vulkano::instance::{debug::*, Instance};
use vulkano::memory::allocator::{MemoryUsage, StandardMemoryAllocator};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::pipeline::{GraphicsPipeline, Pipeline};
use vulkano::render_pass::RenderPass;
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, Subpass};
use vulkano::sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo};
use vulkano::shader::ShaderModule;
use vulkano::swapchain::{
    acquire_next_image, AcquireError, Surface, Swapchain, SwapchainCreateInfo,
    SwapchainCreationError, SwapchainPresentInfo,
};
use vulkano::sync::{self};
use vulkano::sync::{FlushError, GpuFuture};
use winit::{
    event_loop::EventLoop,
    window::{Fullscreen, Window},
};

mod font;
mod instance;
mod pipeline;
mod shaders;
mod swapchain;
mod window;

use shaders::*;

#[allow(unused)]
pub struct App {
    instance: Arc<Instance>,
    debugmessenger: Option<DebugUtilsMessenger>,
    pub surface: Arc<Surface>,
    device_extensions: DeviceExtensions,
    physical_device: Arc<PhysicalDevice>,
    queue_family_index: u32,
    device: Arc<Device>,
    queue: Arc<Queue>,
    swapchain: Arc<Swapchain>,
    images: Vec<Arc<SwapchainImage>>,
    vs: Arc<ShaderModule>,
    fs: Arc<ShaderModule>,
    render_pass: Arc<RenderPass>,
    pipeline: Arc<GraphicsPipeline>,
    viewport: Viewport,
    framebuffers: Vec<Arc<Framebuffer>>,
    pub recreate_swapchain: bool,
    descriptors: [Arc<PersistentDescriptorSet>; 2],
    previous_frame_end: Option<Box<dyn GpuFuture>>,
    pub objects: HashMap<String, Object>,
    pub render_order: Vec<String>,
    vertex_buffer: CpuBufferPool<Vertex>,
    object_buffer: CpuBufferPool<vertexshader::ty::Object>,
    index_buffer: CpuBufferPool<u16>,
    memoryallocator: Arc<StandardMemoryAllocator>,
    commandbufferallocator: StandardCommandBufferAllocator,
    descriptor_set_allocator: StandardDescriptorSetAllocator,
    text_pipeline: Arc<GraphicsPipeline>,
    text_vertex_buffer: Arc<CpuAccessibleBuffer<[TextVertex]>>,
    text_set: Arc<PersistentDescriptorSet>,
    text_vertices: Vec<TextVertex>,
    text_cache: Cache<'static>,
}

impl App {
    pub fn initialize() -> (Self, EventLoop<()>) {
        let instance = instance::create_instance();
        let debugmessenger = instance::setup_debug(&instance);
        let (event_loop, surface) = window::create_window(&instance);
        let device_extensions = instance::create_device_extensions();
        let (physical_device, queue_family_index) =
            instance::create_physical_and_queue(&instance, device_extensions, &surface);
        let (device, queue) = instance::create_device_and_queues(
            &physical_device,
            &device_extensions,
            queue_family_index,
        );

        let dimensions: [f32; 2] = surface
            .object()
            .unwrap()
            .downcast_ref::<Window>()
            .unwrap()
            .inner_size()
            .into();

        let (swapchain, images) = swapchain::create_swapchain_and_images(&device, &surface);

        let memoryallocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));

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

        let vs = vertexshader::load(device.clone()).unwrap();
        let fs = fragmentshader::load(device.clone()).unwrap();

        println!(
            "Using device: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type,
        );

        let render_pass = Self::create_render_pass(&device, &swapchain);
        let descriptor_set_allocator = StandardDescriptorSetAllocator::new(device.clone());
        let commandbufferallocator =
            StandardCommandBufferAllocator::new(device.clone(), Default::default());
        let mut uploads = AutoCommandBufferBuilder::primary(
            &commandbufferallocator,
            queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();
        let texture = {
            let texture = GAME
                .lock()
                .unwrap()
                .resources
                .textures
                .get("rusty")
                .unwrap()
                .as_ref()
                .clone();

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
            device.clone(),
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

        let subpass = Subpass::from(render_pass.clone(), 0).unwrap();
        let pipeline: Arc<GraphicsPipeline> =
            pipeline::create_pipeline(&device, &vs, &fs, subpass.clone());

        let descriptors = [
            PersistentDescriptorSet::new(
                &descriptor_set_allocator,
                pipeline.layout().set_layouts().get(0).unwrap().clone(),
                [WriteDescriptorSet::image_view_sampler(
                    0,
                    texture.clone(),
                    sampler.clone(),
                )],
            )
            .unwrap(),
            PersistentDescriptorSet::new(
                &descriptor_set_allocator,
                pipeline.layout().set_layouts().get(1).unwrap().clone(),
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

        //font

        let tvs = text_vertexshader::load(device.clone()).unwrap();
        let tfs = text_fragmentshader::load(device.clone()).unwrap();

        let mut text_cache = Cache::builder().dimensions(1000, 1000).build();

        let text_pipeline =
            pipeline::create_font_pipeline(&device, &tvs, &tfs, subpass, dimensions);

        let mut viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [0.0, 0.0],
            depth_range: 0.0..1.0,
        };

        let (text_vertex_buffer, text_set, text_vertices) = font::load(
            "test",
            &mut text_cache,
            memoryallocator.clone(),
            device.clone(),
            &mut uploads,
            dimensions,
            &descriptor_set_allocator,
            text_pipeline.clone(),
        );

        let framebuffers =
            Self::window_size_dependent_setup(&images, render_pass.clone(), &mut viewport);

        let recreate_swapchain = false;

        let previous_frame_end = Some(
            uploads
                .build()
                .unwrap()
                .execute(queue.clone())
                .unwrap()
                .boxed(),
        );

        let objects: HashMap<String, Object> = HashMap::new();
        let render_order = Vec::new();

        surface
            .object()
            .unwrap()
            .downcast_ref::<Window>()
            .unwrap()
            .set_visible(true);
        (
            Self {
                instance,
                debugmessenger,
                surface,
                device_extensions,
                physical_device,
                queue_family_index,
                device,
                queue,
                swapchain,
                images,
                vs,
                fs,
                render_pass,
                pipeline,
                viewport,
                framebuffers,
                recreate_swapchain,
                descriptors,
                previous_frame_end,
                objects,
                render_order,
                memoryallocator,
                commandbufferallocator,
                vertex_buffer,
                object_buffer,
                index_buffer,
                descriptor_set_allocator,
                text_pipeline,
                text_vertex_buffer,
                text_set,
                text_vertices,
                text_cache,
            },
            event_loop,
        )
    }

    fn create_render_pass(device: &Arc<Device>, swapchain: &Arc<Swapchain>) -> Arc<RenderPass> {
        vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: swapchain.image_format(),
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {}
            }
        )
        .unwrap()
    }

    fn window_size_dependent_setup(
        images: &[Arc<SwapchainImage>],
        render_pass: Arc<RenderPass>,
        viewport: &mut Viewport,
    ) -> Vec<Arc<Framebuffer>> {
        let dimensions = images[0].dimensions().width_height();
        viewport.dimensions = [dimensions[0] as f32, dimensions[1] as f32];

        images
            .iter()
            .map(|image| {
                let view = ImageView::new_default(image.clone()).unwrap();
                Framebuffer::new(
                    render_pass.clone(),
                    FramebufferCreateInfo {
                        attachments: vec![view],
                        ..Default::default()
                    },
                )
                .unwrap()
            })
            .collect::<Vec<_>>()
    }
    pub fn fullscreen(surface: Arc<Surface>) {
        let window = surface.object().unwrap().downcast_ref::<Window>().unwrap();
        if window.fullscreen() == None {
            window.set_fullscreen(Some(Fullscreen::Borderless(window.current_monitor())));
        //borderless
        //surface.window().set_fullscreen(Some(Fullscreen::Exclusive(MonitorHandle::video_modes(&surface.window().current_monitor().unwrap()).next().unwrap()))); //exclusive
        } else {
            window.set_fullscreen(None)
        }
    }
    pub fn redrawevent(&mut self) {
        //windowevents
        let window = self
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
            self.framebuffers = Self::window_size_dependent_setup(
                &new_images,
                self.render_pass.clone(),
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
            self.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();
        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![Some([0.0, 0.0, 0.0, 0.0].into())],
                    ..RenderPassBeginInfo::framebuffer(
                        self.framebuffers[image_num as usize].clone(),
                    )
                },
                SubpassContents::Inline,
            )
            .unwrap()
            .set_viewport(0, [self.viewport.clone()])
            .bind_pipeline_graphics(self.pipeline.clone());

        if suboptimal {
            self.recreate_swapchain = true;
        }

        //buffer updates

        let push_constants = vertexshader::ty::PushConstant {
            resolution: [dimensions.width as f32, dimensions.height as f32],
            camera: [0.0, 0.0],
        };

        //Draw Objects
        for obj in self
            .render_order
            .iter()
            .map(|x| self.objects.get(x).unwrap())
        {
            self.descriptors[1] = PersistentDescriptorSet::new(
                &self.descriptor_set_allocator,
                self.pipeline.layout().set_layouts().get(1).unwrap().clone(),
                [WriteDescriptorSet::buffer(
                    0,
                    self.object_buffer
                        .from_data(vertexshader::ty::Object {
                            color: obj.color,
                            position: obj.position(),
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
                    self.pipeline.layout().clone(),
                    0,
                    self.descriptors.to_vec(),
                )
                .bind_vertex_buffers(0, vertex_sub_buffer.clone())
                .bind_index_buffer(index_sub_buffer.clone())
                .push_constants(self.pipeline.layout().clone(), 0, push_constants)
                .draw(obj.data.vertices.len() as u32, 1, 0, 0)
                .unwrap();
        }
        //Draw Fonts
        // let text = "Mein Kater Rusty";
        

        builder
            .bind_pipeline_graphics(self.text_pipeline.clone())
            .bind_vertex_buffers(0, [self.text_vertex_buffer.clone()])
            .bind_descriptor_sets(
                vulkano::pipeline::PipelineBindPoint::Graphics,
                self.text_pipeline.layout().clone(),
                0,
                self.text_set.clone(),
            )
            .draw(self.text_vertices.clone().len() as u32, 1, 0, 0)
            .unwrap();

        builder.end_render_pass().unwrap();
        let command_buffer = builder.build().unwrap();

        let future = self
            .previous_frame_end
            .take()
            .unwrap()
            .join(acquire_future)
            .then_execute(self.queue.clone(), command_buffer)
            .unwrap()
            .then_swapchain_present(
                self.queue.clone(),
                SwapchainPresentInfo::swapchain_image_index(self.swapchain.clone(), image_num),
            )
            .then_signal_fence_and_flush();

        match future {
            Ok(future) => {
                self.previous_frame_end = Some(future.boxed());
            }
            Err(FlushError::OutOfDate) => {
                self.recreate_swapchain = true;
                self.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
            }
            Err(e) => {
                println!("Failed to flush future: {:?}", e);
                self.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
            }
        }
    }
}
