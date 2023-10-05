//! Default labels given by the engine.

use std::sync::Arc;

use parking_lot::Mutex;

use anyhow::Result;

use rusttype::gpu_cache::Cache;
use rusttype::{point, Font as RFont, PositionedGlyph, Scale};

use super::super::{vulkan::shaders::*, Loader, Vulkan};
use crate::{
    materials::*,
    objects::Appearance,
    objects::Data,
    objects::GameObject,
    objects::RigidBodyParent,
    resources::textures::*,
    resources::{Font, Resources, Texture},
    Transform, Vertex, WeakObject,
};
use glam::f32::{vec2, Vec2};
use rapier2d::{dynamics::RigidBodyHandle, geometry::ColliderHandle};

/// Info to create default label objects with.
#[derive(Clone)]
pub struct LabelCreateInfo {
    pub transform: Transform,
    pub appearance: Appearance,
    pub text: String,
    pub scale: Vec2,
    pub align: [f32; 2],
}
impl LabelCreateInfo {
    /// Sets the transform of the label and returns it back.
    #[inline]
    pub fn transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }
    /// Sets the appearance of the label and returns it back.
    #[inline]
    pub fn appearance(mut self, appearance: Appearance) -> Self {
        self.appearance = appearance;
        self
    }
    /// Sets the text of the label and returns it back.
    #[inline]
    pub fn text<T>(mut self, text: T) -> Self
    where
        T: Into<String>,
    {
        self.text = text.into();
        self
    }
    /// Sets the scale of the label and returns it back.
    #[inline]
    pub fn scale<T>(mut self, scale: T) -> Self
    where
        T: Into<Vec2>,
    {
        self.scale = scale.into();
        self
    }
    /// Sets the alignment of the label and returns it back.
    #[inline]
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

/// A Label object made to display text.
///
/// # note
///
/// It is recommended to sync or update the text with all other visible labels so the texture of all labels change to the same texture.
/// At the beginning of the game update all the text gets rendered if any labels changed. This produces a new texture which if not synced
/// to every label produces multiple textures, which take more memory.
#[derive(Clone)]
pub struct Label {
    pub transform: Transform,
    parent_transform: Transform,
    pub appearance: Appearance,
    id: usize,
    reference: Option<WeakObject>,
    pub font: Font,
    pub text: String,
    pub scale: Vec2,
    pub align: [f32; 2],
    labelifier: Arc<Mutex<Labelifier>>,
}
impl GameObject for Label {
    /// Returns the transform of the label.
    #[inline]
    fn transform(&self) -> Transform {
        self.transform
    }
    /// Sets the position and rotation of the label.
    #[inline]
    fn set_isometry(&mut self, position: Vec2, rotation: f32) {
        self.transform.position = position;
        self.transform.rotation = rotation;
    }
    /// Returns the public position of the label.
    #[inline]
    fn public_transform(&self) -> Transform {
        self.transform.combine(self.parent_transform)
    }
    #[inline]
    fn set_parent_transform(&mut self, transform: Transform) {
        self.parent_transform = transform;
    }
    /// Returns a reference of the appearance of the label.
    fn appearance(&self) -> &Appearance {
        &self.appearance
    }
    /// Returns the index of the label in the layer it's inside.
    #[inline]
    fn id(&self) -> usize {
        self.id
    }
    fn init_to_layer(
        &mut self,
        id: usize,
        parent: &crate::NObject,
        rigid_body_parent: RigidBodyParent,
        _layer: &crate::Layer,
    ) -> crate::NObject {
        self.id = id;
        let parent_object = &parent.lock().object;
        self.parent_transform = parent_object.public_transform();
        let node: crate::NObject = Arc::new(Mutex::new(crate::objects::Node {
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
    /// Creates a new label with the given settings.
    pub fn new(resources: &Resources, font: &Font, create_info: LabelCreateInfo) -> Self {
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
    /// Updates the local information of this label from the layer, in case it has changed if for example the parent was changed too.
    pub fn update(&mut self) {
        let arc = self.reference.clone().unwrap().upgrade().unwrap();
        let object = &arc.lock().object;
        self.transform = object.transform();
        self.appearance = object.appearance().clone();
    }
    /// Changes the text of the label and updates it on the layer.
    pub fn update_text(&mut self, text: String) {
        self.text = text;
        Self::sync(self);
    }
    /// Syncs the public layer side label to be the same as the current.
    pub fn sync(&self) {
        self.labelifier.lock().queue(self.clone());
    }
}

/// A label maker holding
pub(crate) struct Labelifier {
    /// the default material,
    material: Material,
    /// RustType font cache,
    cache: Cache<'static>,
    /// the global font texture,
    cache_pixel_buffer: Vec<u8>,
    /// yasks to be executed on next update,
    queued: Vec<DrawTask<'static>>,
    /// the index of the latest added font resource to be incremented by 1 every new font
    font_id: usize,
    /// and the boolean if it should update.
    ready: bool,
}

impl Labelifier {
    /// Makes a new label maker.
    pub fn new(vulkan: &Vulkan, loader: &mut Loader) -> Self {
        let cache = Cache::builder().build();
        let cache_pixel_buffer = vec![0; (cache.dimensions().0 * cache.dimensions().1) as usize];

        // Make the cache a texture.
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
    /// Updates the cache in case a label was changed or added.
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
        // Creates a new texture to be inserted into every syncing label.
        // Unsynced label keep holding the old texture.
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

    /// Updates the cache and grows it, in case it's too small for everything.
    fn update_and_resize_cache(&mut self, vulkan: &Vulkan, loader: &mut Loader) {
        loop {
            // Adds every queued task to the cache
            for task in self.queued.iter() {
                for glyph in task.glyphs.clone() {
                    self.cache.queue_glyph(task.label.font.fontid, glyph);
                }
            }

            match self.update_cache(vulkan, loader) {
                // Success
                Ok(_) => (),
                // Grows the cache buffer by 2x for the rest of the runtime in case too many characters were queued for the cache to handle.
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
    }

    fn update_each_object(&self) {
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
    }

    /// Updates everything.
    pub fn update(&mut self, vulkan: &Vulkan, loader: &mut Loader) {
        if !self.ready {
            return;
        }

        Self::update_and_resize_cache(self, vulkan, loader);

        Self::update_each_object(self);

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
    pub fn load_font(&mut self, font: &[u8]) -> Font {
        let font = Font {
            font: Arc::new(RFont::try_from_vec(font.to_vec()).unwrap()),
            fontid: self.font_id,
        };
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
