use std::sync::Arc;

use crate::data::TextVertex;
use crate::GAME;
use rusttype::{gpu_cache::Cache, point, Font, Point, PositionedGlyph, Rect, Scale};
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer},
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer},
    descriptor_set::{
        allocator::{DescriptorSetAllocator, StandardDescriptorSetAllocator},
        PersistentDescriptorSet, WriteDescriptorSet,
    },
    device::Device,
    image::{view::ImageView, ImageDimensions, ImmutableImage, MipmapsCount},
    memory::allocator::{FreeListAllocator, GenericMemoryAllocator},
    pipeline::{GraphicsPipeline, Pipeline},
    sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo},
    swapchain::FullScreenExclusive,
};

fn layout_paragraph<'a>(
    font: &Font<'a>,
    scale: Scale,
    width: u32,
    text: &str,
) -> (Vec<PositionedGlyph<'a>>, [f32; 2]) {
    let mut result = Vec::new();
    let v_metrics = font.v_metrics(scale);
    let advance_height = v_metrics.ascent - v_metrics.descent + v_metrics.line_gap;
    let mut caret = point(0.0, v_metrics.ascent);
    let mut last_glyph_id = None;
    for c in text.chars() {
        if c.is_control() {
            match c {
                '\r' => {
                    caret = point(0.0, caret.y + advance_height);
                }
                '\n' => {}
                _ => {}
            }
            continue;
        }
        let base_glyph = font.glyph(c);
        if let Some(id) = last_glyph_id.take() {
            caret.x += font.pair_kerning(scale, id, base_glyph.id());
        }
        last_glyph_id = Some(base_glyph.id());
        let mut glyph = base_glyph.scaled(scale).positioned(caret);
        if let Some(bb) = glyph.pixel_bounding_box() {
            if bb.max.x > width as i32 {
                caret = point(0.0, caret.y + advance_height);
                glyph.set_position(caret);
                last_glyph_id = None;
            }
        }
        caret.x += glyph.unpositioned().h_metrics().advance_width;
        result.push(glyph);
    }
    (result, [caret.y / 2.0, caret.x / 2.0])
}

pub fn load(
    text: &str,
    text_cache: &mut Cache,
    memoryallocator: Arc<GenericMemoryAllocator<Arc<FreeListAllocator>>>,
    device: Arc<Device>,
    uploads: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    dimensions: [f32; 2],
    descriptor_set_allocator: &StandardDescriptorSetAllocator,
    text_pipeline: Arc<GraphicsPipeline>,
) -> (
    Arc<CpuAccessibleBuffer<[TextVertex]>>,
    Arc<PersistentDescriptorSet>,
    Vec<TextVertex>,
) {
    let mut cache_pixel_buffer = vec![0; 1000 * 1000];

    let glyphs: Vec<PositionedGlyph>;
    {
        let lock = GAME.lock().unwrap();
        let font = lock.resources.fonts.get("Bani-Regular").unwrap();
        glyphs = font
            .layout(text, Scale::uniform(20.0), point(0.0, 20.0))
            .map(|x| x)
            .collect();
    }
    for glyph in &glyphs {
        text_cache.queue_glyph(0, glyph.clone());
    }

    // update texture cache
    text_cache
        .cache_queued(|rect, src_data| {
            let width = (rect.max.x - rect.min.x) as usize;
            let height = (rect.max.y - rect.min.y) as usize;
            let mut dst_index = rect.min.y as usize * 1000 + rect.min.x as usize;
            let mut src_index = 0;
            for _ in 0..height {
                let dst_slice = &mut cache_pixel_buffer[dst_index..dst_index + width];
                let src_slice = &src_data[src_index..src_index + width];
                dst_slice.copy_from_slice(src_slice);

                dst_index += 1000;
                src_index += width;
            }
        })
        .unwrap();

    let cache_texture = ImmutableImage::from_iter(
        &memoryallocator,
        cache_pixel_buffer.iter().cloned(),
        ImageDimensions::Dim2d {
            width: 1000,
            height: 1000,
            array_layers: 1,
        },
        MipmapsCount::One,
        vulkano::format::Format::R8_UNORM,
        uploads,
    )
    .unwrap();

    let sampler = Sampler::new(
        device.clone(),
        SamplerCreateInfo {
            mag_filter: Filter::Nearest,
            min_filter: Filter::Linear,
            address_mode: [
                SamplerAddressMode::Repeat,
                SamplerAddressMode::Repeat,
                SamplerAddressMode::Repeat,
            ],
            ..Default::default()
        },
    )
    .unwrap();

    let cache_texture_view = ImageView::new_default(cache_texture).unwrap();
    let text_set = PersistentDescriptorSet::new(
        descriptor_set_allocator,
        text_pipeline.layout().set_layouts().get(0).unwrap().clone(),
        [WriteDescriptorSet::image_view_sampler(
            0,
            cache_texture_view.clone(),
            sampler.clone(),
        )],
    )
    .unwrap();

    let mut text_vertices: Vec<TextVertex> = vec![];
    for _ in glyphs.clone().iter() {
        text_vertices = glyphs
            .clone()
            .iter()
            .flat_map(|g| {
                if let Ok(Some((uv_rect, screen_rect))) = text_cache.rect_for(0, g) {
                    let gl_rect = Rect {
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
    }

    let text_vertex_buffer = CpuAccessibleBuffer::from_iter(
        &memoryallocator,
        BufferUsage {
            vertex_buffer: true,
            ..Default::default()
        },
        false,
        text_vertices.clone().into_iter(),
    )
    .unwrap();
    (text_vertex_buffer, text_set, text_vertices)
}
