use image::{ImageBuffer, Rgb, Rgba};
use rusttype::{gpu_cache::Cache, point, PositionedGlyph};
use std::{
    collections::HashMap,
    io::Cursor,
    sync::{Arc, Mutex},
};
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
    objects::{
        data::{TextVertex, Vertex},
        Display, Object,
    },
    resources::Resources,
    vulkan::{window_size_dependent_setup, Vulkan},
};

use crate::{game::vulkan::shaders::*, ObjectNode, VisualObject};

#[allow(unused)]
pub struct Draw {
    pub recreate_swapchain: bool,
    descriptors: [Arc<PersistentDescriptorSet>; 2],
    previous_frame_end: Option<Box<dyn GpuFuture>>,
    vertex_buffer: CpuBufferPool<Vertex>,
    object_buffer: CpuBufferPool<vertexshader::ty::Object>,
    index_buffer: CpuBufferPool<u16>,
    memoryallocator: Arc<StandardMemoryAllocator>,
    commandbufferallocator: StandardCommandBufferAllocator,
    descriptor_set_allocator: StandardDescriptorSetAllocator,
    texture_hash: HashMap<String, u32>,
    font_cache: Cache<'static>,
    text_cache: HashMap<(String, String), (Vec<TextVertex>, Arc<PersistentDescriptorSet>)>,
    text_vertex_buffer: CpuBufferPool<TextVertex>,
}

impl Draw {
    pub fn setup(vulkan: &Vulkan, resources: &Resources) -> Self {
        let mut texture_hash: HashMap<String, u32> = HashMap::new();

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
        let index_buffer: CpuBufferPool<u16> = CpuBufferPool::new(
            memoryallocator.clone(),
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

        //textures
        let texture = {
            let mut wh = [0; 2];

            let mut texture: Vec<u8> = resources
                .textures
                .clone()
                .into_iter()
                .zip(1_u32..)
                .flat_map(|t| {
                    texture_hash.insert(t.0 .0, t.1);
                    let tex = load_texture(t.0 .1);
                    wh = tex.1.width_height();
                    tex.0
                })
                .collect();

            let mut dimensions = ImageDimensions::Dim2d {
                width: wh[0],
                height: wh[1],
                array_layers: texture_hash.len() as u32,
            };

            if dimensions.width_height() == [0; 2] {
                texture = vec![0, 0, 0, 255];
                dimensions = ImageDimensions::Dim2d {
                    width: 1,
                    height: 1,
                    array_layers: 1,
                };
            }

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

        //fonts
        let font_cache = Cache::builder().dimensions(512, 512).build();

        let text_vertex_buffer = CpuBufferPool::new(
            memoryallocator.clone(),
            BufferUsage {
                vertex_buffer: true,
                ..Default::default()
            },
            MemoryUsage::Upload,
        );

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
            texture_hash,
            font_cache,
            text_cache: HashMap::new(),
            text_vertex_buffer,
        }
    }

    pub fn update_font_objects(
        &mut self,
        vulkan: &mut Vulkan,
        resources: &Resources,
        visual_object: VisualObject,
    ) -> (Vec<TextVertex>, Arc<PersistentDescriptorSet>) {
        let dimensions: [u32; 2] = vulkan
            .surface
            .object()
            .unwrap()
            .downcast_ref::<Window>()
            .unwrap()
            .inner_size()
            .try_into()
            .unwrap();

        let mut cache_pixel_buffer = vec![0; 512 * 512];

        let glyphs: Vec<PositionedGlyph>;

        let font: String = visual_object.clone().font.unwrap();

        let font = resources.fonts.get(&font).unwrap();
        glyphs = font
            .layout(
                visual_object.text.unwrap().as_ref(),
                rusttype::Scale::uniform(50.0),
                point(0.0, 50.0),
            )
            .collect();

        for glyph in &glyphs {
            self.font_cache.queue_glyph(0, glyph.clone());
        }

        self.font_cache
            .cache_queued(|rect, src_data| {
                let width = (rect.max.x - rect.min.x) as usize;
                let height = (rect.max.y - rect.min.y) as usize;
                let mut dst_index = rect.min.y as usize * 512 + rect.min.x as usize;
                let mut src_index = 0;
                for _ in 0..height {
                    let dst_slice = &mut cache_pixel_buffer[dst_index..dst_index + width];
                    let src_slice = &src_data[src_index..src_index + width];
                    dst_slice.copy_from_slice(src_slice);

                    dst_index += 512;
                    src_index += width;
                }
            })
            .unwrap();

        let mut uploads = AutoCommandBufferBuilder::primary(
            &self.commandbufferallocator,
            vulkan.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let cache_texture = ImmutableImage::from_iter(
            &self.memoryallocator,
            cache_pixel_buffer.iter().cloned(),
            ImageDimensions::Dim2d {
                width: 512,
                height: 512,
                array_layers: 1,
            },
            MipmapsCount::One,
            vulkano::format::Format::R8_UNORM,
            &mut uploads,
        )
        .unwrap();

        let cache_texture_view = ImageView::new_default(cache_texture).unwrap();

        let text_vertices: Vec<TextVertex> = glyphs
            .clone()
            .iter()
            .flat_map(|g| {
                if let Ok(Some((uv_rect, screen_rect))) = self.font_cache.rect_for(0, g) {
                    let gl_rect = rusttype::Rect {
                        min: point(
                            (screen_rect.min.x as f32 / dimensions[0] as f32 - 0.5) * 2.0,
                            (screen_rect.min.y as f32 / dimensions[1] as f32 - 0.5) * 2.0,
                        ),
                        max: point(
                            (screen_rect.max.x as f32 / dimensions[0] as f32 - 0.5) * 2.0,
                            (screen_rect.max.y as f32 / dimensions[1] as f32 - 0.5) * 2.0,
                        ),
                    };
                    vec![
                        TextVertex {
                            position: [gl_rect.min.x, gl_rect.max.y],
                            tex_position: [uv_rect.min.x, uv_rect.max.y],
                        },
                        TextVertex {
                            position: [gl_rect.min.x, gl_rect.min.y],
                            tex_position: [uv_rect.min.x, uv_rect.min.y],
                        },
                        TextVertex {
                            position: [gl_rect.max.x, gl_rect.min.y],
                            tex_position: [uv_rect.max.x, uv_rect.min.y],
                        },
                        TextVertex {
                            position: [gl_rect.max.x, gl_rect.min.y],
                            tex_position: [uv_rect.max.x, uv_rect.min.y],
                        },
                        TextVertex {
                            position: [gl_rect.max.x, gl_rect.max.y],
                            tex_position: [uv_rect.max.x, uv_rect.max.y],
                        },
                        TextVertex {
                            position: [gl_rect.min.x, gl_rect.max.y],
                            tex_position: [uv_rect.min.x, uv_rect.max.y],
                        },
                    ]
                    .into_iter()
                } else {
                    vec![].into_iter()
                }
            })
            .collect();

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

        (
            text_vertices,
            PersistentDescriptorSet::new(
                &self.descriptor_set_allocator,
                vulkan
                    .text_pipeline
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
        )
    }

    pub fn redrawevent(
        &mut self,
        vulkan: &mut Vulkan,
        objects: &Vec<Arc<Mutex<ObjectNode>>>,
        resources: &Resources,
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

        let mut order: Vec<Object> = vec![];

        for obj in objects {
            let object = obj.lock().unwrap().object.clone();
            order.push(object.clone());
            ObjectNode::order_position(&mut order, obj);
        }

        for obj in order {
            if let Some(visual_object) = obj.graphics {
                if visual_object.display == Display::Data {
                    self.descriptors[1] = PersistentDescriptorSet::new(
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
                                    color: obj.color,
                                    position: obj.position,
                                    size: obj.size,
                                    rotation: obj.rotation,
                                    textureID: if let Some(name) = visual_object.texture {
                                        *self.texture_hash.get(&name).unwrap()
                                    } else {
                                        0
                                    },
                                })
                                .unwrap(),
                        )],
                    )
                    .unwrap();

                    let index_sub_buffer = self
                        .index_buffer
                        .from_iter(visual_object.data.indices.clone())
                        .unwrap();
                    let vertex_sub_buffer = self
                        .vertex_buffer
                        .from_iter(visual_object.data.vertices.clone())
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
                        .draw(visual_object.data.vertices.len() as u32, 1, 0, 0)
                        .unwrap();
                } else {
                    loop {
                        match self.text_cache.get(&(
                            visual_object.clone().font.unwrap(),
                            visual_object.clone().text.unwrap(),
                        )) {
                            Some((vertices, text_descriptor)) => {
                                let vertex_subbuffer =
                                    self.text_vertex_buffer.from_iter(vertices.clone()).unwrap();
                                builder
                                    .bind_pipeline_graphics(vulkan.text_pipeline.clone())
                                    .bind_vertex_buffers(0, [vertex_subbuffer.clone()])
                                    .bind_descriptor_sets(
                                        vulkano::pipeline::PipelineBindPoint::Graphics,
                                        vulkan.text_pipeline.layout().clone(),
                                        0,
                                        text_descriptor.clone(),
                                    )
                                    .draw(vertices.clone().len() as u32, 1, 0, 0)
                                    .unwrap();
                                break;
                            }
                            None => {
                                let text_data = Self::update_font_objects(self, vulkan, resources, visual_object.clone());
                                self.text_cache.insert(
                                    (
                                        visual_object.clone().font.unwrap(),
                                        visual_object.clone().text.unwrap(),
                                    ),
                                    text_data,
                                );
                                println!("New text");
                            }
                        }
                    }
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
fn rgb_to_rgba(rgb_image: &ImageBuffer<Rgb<u8>, Vec<u8>>) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let (width, height) = rgb_image.dimensions();
    let mut rgba_image = ImageBuffer::new(width, height);
    for (x, y, pixel) in rgb_image.enumerate_pixels() {
        let Rgb([r, g, b]) = *pixel;
        let rgba = Rgba([r, g, b, 255]);
        rgba_image.put_pixel(x, y, rgba);
    }
    rgba_image
}

fn load_texture(png_bytes: Arc<Vec<u8>>) -> (Vec<u8>, ImageDimensions) {
    let cursor = Cursor::new(png_bytes.to_vec());
    let decoder = png::Decoder::new(cursor);
    let mut reader = decoder.read_info().unwrap();
    let info = reader.info();
    let dimensions = ImageDimensions::Dim2d {
        width: info.width,
        height: info.height,
        array_layers: 1,
    };
    let color_type = info.color_type.clone();
    let pixels = info.width * info.height;

    let mut image_data = Vec::new();
    image_data.resize((pixels * 4) as usize, 0);
    reader.next_frame(&mut image_data).unwrap();

    if color_type == png::ColorType::Rgb {
        image_data.resize((pixels * 3) as usize, 0);
        let imbuf =
            image::ImageBuffer::from_vec(dimensions.width(), dimensions.height(), image_data)
                .unwrap();
        let imbuf = rgb_to_rgba(&imbuf);
        image_data = imbuf.to_vec();
    }

    (image_data, dimensions)
}
