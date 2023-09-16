use std::sync::Arc;

use parking_lot::Mutex;

use anyhow::Result;

use rusttype::gpu_cache::Cache;
use rusttype::{point, Font, PositionedGlyph, Scale};

use crate::{texture::*, Data, RigidBodyParent, Vertex, WeakObject};

use super::{
    materials::*,
    objects::GameObject,
    resources::{GameFont, Resources, Texture},
    vulkan::shaders::*,
    Appearance, Loader, Transform, Vulkan,
};
use glam::f32::{vec2, Vec2};
use rapier2d::{dynamics::RigidBodyHandle, geometry::ColliderHandle};

#[derive(Clone)]
pub struct LabelCreateInfo {
    pub transform: Transform,
    pub appearance: Appearance,
    pub text: String,
    pub scale: Vec2,
    pub align: [f32; 2],
}
impl LabelCreateInfo {
    pub fn transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }
    pub fn appearance(mut self, appearance: Appearance) -> Self {
        self.appearance = appearance;
        self
    }
    pub fn text<T>(mut self, text: T) -> Self
    where
        T: Into<String>,
    {
        self.text = text.into();
        self
    }
    pub fn scale<T>(mut self, scale: T) -> Self
    where
        T: Into<Vec2>,
    {
        self.scale = scale.into();
        self
    }
    pub fn align<T>(mut self, align: T) -> Self
    where
        T: Into<[f32; 2]>,
    {
        self.align = align.into();
        self
    }
}
impl Default for LabelCreateInfo {
    fn default() -> Self {
        Self {
            transform: Transform::default(),
            appearance: Appearance::default(),
            text: String::new(),
            scale: vec2(25.0, 25.0),
            align: [0.0; 2],
        }
    }
}
#[derive(Clone)]
pub struct Label {
    pub transform: Transform,
    parent_transform: Transform,
    pub appearance: Appearance,
    id: usize,
    reference: Option<WeakObject>,
    pub font: Arc<GameFont>,
    pub text: String,
    pub scale: Vec2,
    pub align: [f32; 2],
    labelifier: Arc<Mutex<Labelifier>>,
}
impl GameObject for Label {
    fn transform(&self) -> Transform {
        self.transform
    }
    fn set_isometry(&mut self, position: Vec2, rotation: f32) {
        self.transform.position = position;
        self.transform.rotation = rotation;
    }
    fn public_transform(&self) -> Transform {
        self.transform.combine(self.parent_transform)
    }
    fn set_parent_transform(&mut self, transform: Transform) {
        self.parent_transform = transform;
    }
    fn appearance(&self) -> &Appearance {
        &self.appearance
    }
    fn id(&self) -> usize {
        self.id
    }
    fn init_to_layer(
        &mut self,
        id: usize,
        parent: &crate::NObject,
        rigid_body_parent: RigidBodyParent,
        _layer: &super::Layer,
    ) -> crate::NObject {
        self.id = id;
        let parent_object = &parent.lock().object;
        self.parent_transform = parent_object.public_transform();
        let node: crate::NObject = Arc::new(Mutex::new(crate::Node {
            object: Box::new(self.clone()),
            parent: Some(Arc::downgrade(parent)),
            rigid_body_parent,
            children: vec![],
        }));
        self.reference = Some(Arc::downgrade(&node));
        self.labelifier.lock().queue(self.clone());
        node
    }
    fn remove_event(&mut self) {}
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn collider_handle(&self) -> Option<ColliderHandle> {
        None
    }
    fn rigidbody_handle(&self) -> Option<RigidBodyHandle> {
        None
    }
}
impl Label {
    pub fn new(resources: &Resources, font: &Arc<GameFont>, create_info: LabelCreateInfo) -> Self {
        let labelifier = resources.labelifier.clone();
        Self {
            transform: create_info.transform,
            parent_transform: Transform::default(),
            appearance: create_info.appearance,
            id: 0,
            reference: None,
            font: font.clone(),
            text: create_info.text,
            scale: create_info.scale,
            align: create_info.align,
            labelifier,
        }
    }
    pub fn update(&mut self) {
        let arc = self.reference.clone().unwrap().upgrade().unwrap();
        let object = &arc.lock().object;
        self.transform = object.transform();
        self.appearance = object.appearance().clone();
    }
    pub fn update_text(&mut self, text: String) {
        self.text = text;
        Self::sync(self);
    }
    pub fn sync(&self) {
        self.labelifier.lock().queue(self.clone());
    }
}

pub(crate) struct Labelifier {
    material: Material,
    cache: Cache<'static>,
    cache_pixel_buffer: Vec<u8>,
    queued: Vec<DrawTask<'static>>,
    font_id: usize,
    ready: bool,
}

impl Labelifier {
    pub fn new(vulkan: &Vulkan, loader: &mut Loader) -> Self {
        let cache = Cache::builder().build();
        let cache_pixel_buffer = vec![0; (cache.dimensions().0 * cache.dimensions().1) as usize];
        let texture = Texture {
            data: Arc::from(cache_pixel_buffer.clone().into_boxed_slice()),
            dimensions: cache.dimensions(),
            layers: 1,
            set: loader.load_texture(
                vulkan,
                &cache_pixel_buffer,
                cache.dimensions(),
                1,
                Format::R8,
                TextureSettings {
                    srgb: false,
                    sampler: Sampler::default(),
                },
            ),
        };

        let text_shaders = Shaders {
            vertex: vertexshader::load(vulkan.device.clone()).unwrap(),
            fragment: text_fragmentshader::load(vulkan.device.clone()).unwrap(),
        };

        let material_settings = MaterialSettingsBuilder::default()
            .texture(texture)
            .build()
            .unwrap();

        let material = loader.load_material(vulkan, &text_shaders, material_settings, vec![]);

        Self {
            material,
            cache,
            cache_pixel_buffer,
            queued: vec![],
            font_id: 0,
            ready: false,
        }
    }
    fn update_cache(
        &mut self,
        vulkan: &Vulkan,
        loader: &mut Loader,
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
        self.material.texture = Some(Texture {
            data: Arc::from(self.cache_pixel_buffer.clone().into_boxed_slice()),
            dimensions: self.cache.dimensions(),
            layers: 1,
            set: loader.load_texture(
                vulkan,
                &self.cache_pixel_buffer,
                self.cache.dimensions(),
                1,
                Format::R8,
                TextureSettings {
                    srgb: false,
                    sampler: Sampler::default(),
                },
            ),
        });
        Ok(())
    }
    pub fn update(&mut self, vulkan: &Vulkan, loader: &mut Loader) {
        if !self.ready {
            return;
        }

        loop {
            for task in self.queued.iter() {
                for glyph in task.glyphs.clone() {
                    self.cache.queue_glyph(task.label.font.fontid, glyph);
                }
            }

            match self.update_cache(vulkan, loader) {
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
            let mut label = task.label.clone();

            let size = label.appearance.transform.size;

            let dimensions: [f32; 2] = [(1000.0 * size[0]), (1000.0 * size[1])];

            let mut indices: Vec<u32> = vec![];

            let mut id = 0;

            let vertices: Vec<Vertex> = task
                .glyphs
                .clone()
                .iter()
                .flat_map(|g| {
                    if let Ok(Some((uv_rect, screen_rect))) =
                        self.cache.rect_for(label.font.fontid, g)
                    {
                        let gl_rect = rusttype::Rect {
                            min: point(
                                (screen_rect.min.x as f32 / dimensions[0] - 0.5) * 2.0,
                                (screen_rect.min.y as f32 / dimensions[1] - 0.5) * 2.0,
                            ),
                            max: point(
                                (screen_rect.max.x as f32 / dimensions[0] - 0.5) * 2.0,
                                (screen_rect.max.y as f32 / dimensions[1] - 0.5) * 2.0,
                            ),
                        };
                        indices.extend([1 + id, 2 + id, id, 2 + id, id, 3 + id]);
                        id += 4;
                        vec![
                            Vertex {
                                position: vec2(gl_rect.min.x, gl_rect.max.y),
                                tex_position: vec2(uv_rect.min.x, uv_rect.max.y),
                            },
                            Vertex {
                                position: vec2(gl_rect.min.x, gl_rect.min.y),
                                tex_position: vec2(uv_rect.min.x, uv_rect.min.y),
                            },
                            Vertex {
                                position: vec2(gl_rect.max.x, gl_rect.min.y),
                                tex_position: vec2(uv_rect.max.x, uv_rect.min.y),
                            },
                            Vertex {
                                position: vec2(gl_rect.max.x, gl_rect.max.y),
                                tex_position: vec2(uv_rect.max.x, uv_rect.max.y),
                            },
                        ]
                        .into_iter()
                    } else {
                        vec![].into_iter()
                    }
                })
                .collect();
            label.appearance.data = Data { vertices, indices };
            label.appearance.material = Some(self.material.clone());
            //label.sync();
            let arc = label.reference.clone().unwrap().upgrade().unwrap();
            let mut object = arc.lock();
            object.object = Box::new(label.clone());
        }
        self.queued = vec![];
        self.ready = false;
    }
    pub fn queue(&mut self, label: Label) {
        self.ready = true;

        let size = label.appearance().transform.size;

        let dimensions: [f32; 2] = [(1000.0 * size[0]), (1000.0 * size[1])];

        let glyphs: Vec<PositionedGlyph> = layout_paragraph(&label, dimensions);

        self.queued.push(DrawTask { label, glyphs });
    }
    /// Loads a font ready to get layed out and rendered.
    pub fn load_font(&mut self, font: &[u8]) -> Arc<GameFont> {
        let font = Arc::new(GameFont {
            font: Font::try_from_vec(font.to_vec()).unwrap(),
            fontid: self.font_id,
        });
        self.font_id += 1;
        font
    }
}

struct DrawTask<'a> {
    pub label: Label,
    pub glyphs: Vec<PositionedGlyph<'a>>,
}

fn layout_paragraph<'a>(label: &Label, dimensions: [f32; 2]) -> Vec<PositionedGlyph<'a>> {
    if label.text.is_empty() {
        return vec![];
    };
    let mut result: Vec<Vec<PositionedGlyph>> = vec![vec![]];
    let scale = Scale {
        x: label.scale[0],
        y: label.scale[1],
    };
    let v_metrics = label.font.font.v_metrics(scale);
    let advance_height = v_metrics.ascent - v_metrics.descent + v_metrics.line_gap;
    let mut caret = point(0.0, v_metrics.ascent);
    let mut last_glyph_id = None;
    for c in label.text.chars() {
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
        let base_glyph = label.font.font.glyph(c);
        if let Some(id) = last_glyph_id.take() {
            caret.x += label.font.font.pair_kerning(scale, id, base_glyph.id());
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
                    glyph.1.position().x + xshift * label.align[0],
                    glyph.1.position().y + yshift * label.align[1],
                ))
            }
        };
    }
    result.into_iter().flatten().collect()
}
