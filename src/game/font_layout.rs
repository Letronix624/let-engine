use std::sync::Arc;

use parking_lot::Mutex;

use anyhow::Result;

use rusttype::gpu_cache::Cache;
use rusttype::{point, Font, PositionedGlyph, Scale};

use crate::{texture::*, Data, Vertex};

use super::{
    objects::Object,
    resources::{GameFont, Texture},
    Appearance, Draw, Vulkan,
    materials::*,
    vulkan::shaders::*
};

type AObject = Arc<Mutex<Object>>;

pub struct Labelifier {
    material: Material,
    cache: Cache<'static>,
    cache_pixel_buffer: Vec<u8>,
    queued: Vec<DrawTask<'static>>,
    ready: bool,
}

impl Labelifier {
    pub fn new(vulkan: &Vulkan, draw: &mut Draw) -> Self {
        let cache = Cache::builder().build();
        let cache_pixel_buffer = vec![0; (cache.dimensions().0 * cache.dimensions().1) as usize];
        let texture = Arc::new(Texture {
            data: cache_pixel_buffer.clone(),
            dimensions: cache.dimensions(),
            layers: 1,
            set: draw.load_texture(
                vulkan,
                cache_pixel_buffer.clone(),
                cache.dimensions(),
                1,
                Format::R8,
                TextureSettings {
                    srgb: false,
                    sampler: Sampler::default(),
                },
            ),
        });

        let text_shaders = Shaders {
            vertex: vertexshader::load(vulkan.device.clone()).unwrap(),
            fragment: text_fragmentshader::load(vulkan.device.clone()).unwrap(),
        };
        
        let material_settings = MaterialSettingsBuilder::default()
            .shaders(text_shaders)
            .texture(texture)
            .build()
            .unwrap();

        let material = draw.load_material (
            &vulkan,
            material_settings,
            vec![]
        );

        Self {
            material,
            cache,
            cache_pixel_buffer,
            queued: vec![],
            ready: false,
        }
    }
    fn update_cache(
        &mut self,
        vulkan: &Vulkan,
        draw: &mut Draw,
    ) -> Result<(), rusttype::gpu_cache::CacheWriteErr> {
        let dimensions = self.cache.dimensions().0 as usize;

        self.cache.cache_queued(|rect, src_data| {
            let width = (rect.max.x - rect.min.x) as usize;
            let height = (rect.max.y - rect.min.y) as usize;
            let mut dst_index = rect.min.y as usize * dimensions + rect.min.x as usize;
            let mut src_index = 0;
            for _ in 0..height {
                let dst_slice = &mut self.cache_pixel_buffer[dst_index..dst_index + width];
                let src_slice = &src_data[src_index..src_index + width];
                dst_slice.copy_from_slice(src_slice);

                dst_index += dimensions;
                src_index += width;
            }
        })?;
        self.material.texture = Some(Arc::new(Texture {
            data: self.cache_pixel_buffer.clone(),
            dimensions: self.cache.dimensions(),
            layers: 1,
            set: draw.load_texture(
                vulkan,
                self.cache_pixel_buffer.clone(),
                self.cache.dimensions(),
                1,
                Format::R8,
                TextureSettings {
                    srgb: false,
                    sampler: Sampler::default(),
                },
            ),
        }));
        Ok(())
    }
    pub fn update(&mut self, vulkan: &Vulkan, draw: &mut Draw) {
        if !self.ready {
            return ();
        }

        loop {
            for task in self.queued.iter() {
                for glyph in task.glyphs.clone() {
                    self.cache.queue_glyph(task.font.fontid, glyph);
                }
            }

            match self.update_cache(vulkan, draw) {
                Ok(_) => (),
                _ => {
                    let dimensions = self.cache.dimensions().0 * 2;
                    self.cache
                        .to_builder()
                        .dimensions(dimensions, dimensions)
                        .rebuild(&mut self.cache);
                    self.cache_pixel_buffer = vec![0; (dimensions * dimensions) as usize];
                    continue;
                }
            };
            break;
        }
        for task in self.queued.iter() {
            let mut object = task.object.lock();

            let size = if let Some(appearance) = object.graphics.clone() {
                appearance.size
            } else {
                object.graphics = Some(Appearance {
                    color: [1.0; 4],
                    ..Default::default()
                });
                [1.0, 1.0]
            };

            let dimensions: [f32; 2] = [(1000.0 * size[0]), (1000.0 * size[1])];

            let mut indices: Vec<u32> = vec![];

            let mut id = 0;

            let vertices: Vec<Vertex> = task
                .glyphs
                .clone()
                .iter()
                .flat_map(|g| {
                    if let Ok(Some((uv_rect, screen_rect))) =
                        self.cache.rect_for(task.font.fontid, g)
                    {
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
                        indices.extend([1 + id, 2 + id, 0 + id, 2 + id, 0 + id, 3 + id]);
                        id += 4;
                        vec![
                            Vertex {
                                position: [gl_rect.min.x, gl_rect.max.y],
                                tex_position: [uv_rect.min.x, uv_rect.max.y],
                            },
                            Vertex {
                                position: [gl_rect.min.x, gl_rect.min.y],
                                tex_position: [uv_rect.min.x, uv_rect.min.y],
                            },
                            Vertex {
                                position: [gl_rect.max.x, gl_rect.min.y],
                                tex_position: [uv_rect.max.x, uv_rect.min.y],
                            },
                            Vertex {
                                position: [gl_rect.max.x, gl_rect.max.y],
                                tex_position: [uv_rect.max.x, uv_rect.max.y],
                            },
                        ]
                        .into_iter()
                    } else {
                        vec![].into_iter()
                    }
                })
                .collect();
            let appearance = object.graphics.as_mut().unwrap();
            appearance.data = Data { vertices, indices };
            appearance.material = Some(self.material.clone());
        }
        self.queued = vec![];
        self.ready = false;
    }
    pub fn queue(
        &mut self,
        object: AObject,
        font: &Arc<GameFont>,
        text: String,
        scale: f32,
        align: [f32; 2],
    ) {
        self.ready = true;

        let obj = object.lock();

        let size = if let Some(appearance) = obj.graphics.clone() {
            appearance.size
        } else {
            [1.0, 1.0]
        };

        let dimensions: [f32; 2] = [(1000.0 * size[0]), (1000.0 * size[1])];

        let glyphs: Vec<PositionedGlyph> =
            layout_paragraph(&font.font, &text, scale, dimensions, align);

        drop(obj);

        self.queued.push(DrawTask {
            object,
            font: font.clone(),
            glyphs,
        });
    }
}

struct DrawTask<'a> {
    pub object: AObject,
    pub font: Arc<GameFont>,
    pub glyphs: Vec<PositionedGlyph<'a>>,
}

fn layout_paragraph<'a>(
    font: &Font<'static>,
    text: &str,
    scale: f32,
    dimensions: [f32; 2],
    align: [f32; 2],
) -> Vec<PositionedGlyph<'a>> {
    if text == "" {
        return vec![];
    };
    let mut result: Vec<Vec<PositionedGlyph>> = vec![vec![]];
    let scale = Scale::uniform(scale);
    let v_metrics = font.v_metrics(scale);
    let advance_height = v_metrics.ascent - v_metrics.descent + v_metrics.line_gap;
    let mut caret = point(0.0, v_metrics.ascent);
    let mut last_glyph_id = None;
    for c in text.chars() {
        if c.is_control() {
            match c {
                '\r' => {
                    caret = point(0.0, caret.y + advance_height);
                    result.push(vec![]);
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
            if bb.max.x > dimensions[0] as i32 {
                result.push(vec![]);
                caret = point(0.0, caret.y + advance_height);
                glyph.set_position(caret);
                last_glyph_id = None;
            }
        }
        caret.x += glyph.unpositioned().h_metrics().advance_width;
        result.last_mut().unwrap().push(glyph);
    }

    let yshift = dimensions[1] - result.len() as f32 * advance_height + v_metrics.descent;
    for line in result.clone().into_iter().enumerate() {
        if let Some(last) = line.1.last() {
            let xshift =
                dimensions[0] - last.position().x - last.unpositioned().h_metrics().advance_width;
            for glyph in result[line.0].clone().iter().enumerate() {
                result[line.0][glyph.0].set_position(point(
                    glyph.1.position().x + xshift * align[0],
                    glyph.1.position().y + yshift * align[1],
                ))
            }
        };
    }
    result.into_iter().flatten().collect()
}
