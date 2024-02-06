//! Default labels given by the engine.

use ab_glyph::FontArc;
use glyph_brush::ab_glyph::PxScale;
use glyph_brush::{
    ab_glyph, BrushAction, DefaultSectionHasher, FontId, GlyphBrush, GlyphBrushBuilder,
    HorizontalAlign, Layout, OwnedSection, OwnedText, VerticalAlign,
};
use std::sync::Arc;

use anyhow::Result;

use super::super::resources::vulkan::shaders::*;
use crate::prelude::*;
use glam::f32::{vec2, Vec2};

/// Info to create default label objects with.
#[derive(Clone)]
pub struct LabelCreateInfo {
    /// Initial position.
    pub transform: Transform,
    /// The appearance of the label.
    pub appearance: Appearance,
    /// Initial text of the label.
    pub text: String,
    /// The scale of the text area.
    pub scale: Vec2,
    /// The align of where the text gets rendered. Takes either 0.0 to 1.0 uv coordinates or one of the [direction](crate::directions) presets.
    pub align: Direction,
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
        T: Into<Direction>,
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
            align: Direction::Nw,
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
pub struct Label<Object> {
    pub object: Object,
    pub font: Font,
    pub text: String,
    pub scale: Vec2,
    pub align: Direction,
    section: OwnedSection<Extra>,
}
impl Label<NewObject> {
    /// Creates a new label with the given settings.
    pub fn new(font: &Font, create_info: LabelCreateInfo) -> Self {
        let mut object = NewObject::new();
        object.transform = create_info.transform;
        object.appearance = create_info.appearance;
        Self {
            object,
            font: font.clone(),
            text: create_info.text,
            scale: create_info.scale,
            align: create_info.align,
            section: OwnedSection::default(),
        }
    }
    pub fn init(mut self, layer: &Arc<Layer>) -> Result<Label<Object>> {
        let mut labelifier = LABELIFIER.lock();
        self.update_section(
            labelifier.increment_tasks(),
            self.object.appearance.get_transform().size,
        );
        let object = self.object.init(layer)?;
        let label = Label {
            object,
            font: self.font,
            text: self.text,
            scale: self.scale,
            align: self.align,
            section: self.section,
        };
        labelifier.queue(label.clone());
        Ok(label)
    }
    pub fn init_with_parent(
        mut self,
        layer: &Arc<Layer>,
        parent: &Object,
    ) -> Result<Label<Object>> {
        let mut labelifier = LABELIFIER.lock();
        self.update_section(
            labelifier.increment_tasks(),
            self.object.appearance.get_transform().size,
        );
        let object = self.object.init_with_parent(layer, parent)?;
        let label = Label {
            object,
            font: self.font,
            text: self.text,
            scale: self.scale,
            align: self.align,
            section: self.section,
        };
        labelifier.queue(label.clone());
        Ok(label)
    }
    pub fn init_with_optional_parent(
        mut self,
        layer: &Arc<Layer>,
        parent: Option<&Object>,
    ) -> Result<Label<Object>> {
        let mut labelifier = LABELIFIER.lock();
        self.update_section(
            labelifier.increment_tasks(),
            self.object.appearance.get_transform().size,
        );
        let object = self.object.init_with_optional_parent(layer, parent)?;
        let label = Label {
            object,
            font: self.font,
            text: self.text,
            scale: self.scale,
            align: self.align,
            section: self.section,
        };
        labelifier.queue(label.clone());
        Ok(label)
    }
}

impl<T> Label<T> {
    fn update_section(&mut self, id: usize, size: Vec2) {
        let dimensions: (f32, f32) = ((1000.0 * size[0]), (1000.0 * size[1]));

        let text = OwnedText {
            text: self.text.clone(),
            scale: PxScale {
                x: self.scale.x,
                y: self.scale.y,
            },
            font_id: self.font.id(),
            extra: Extra { id },
        };

        let (h, v): (HorizontalAlign, VerticalAlign) = self.align.into();
        let x = match h {
            HorizontalAlign::Left => 0.0,
            HorizontalAlign::Center => dimensions.0 * 0.5,
            HorizontalAlign::Right => dimensions.0,
        };
        let y = match v {
            VerticalAlign::Top => 0.0,
            VerticalAlign::Center => dimensions.1 * 0.5,
            VerticalAlign::Bottom => dimensions.1,
        };

        self.section = OwnedSection::default()
            .with_bounds(dimensions)
            .with_layout(Layout::default().h_align(h).v_align(v))
            .with_screen_position((x, y))
            .add_text(text);
    }
}
impl Label<Object> {
    /// Updates the local information of this label from the layer, in case it has changed if for example the parent was changed too.
    pub fn update(&mut self) {
        self.object.update();
    }
    /// Changes the text of the label and updates it on the layer.
    pub fn update_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.sync();
    }
    /// Syncs the public layer side label to be the same as the current.
    pub fn sync(&mut self) {
        let mut labelifier = LABELIFIER.lock();
        self.update_section(
            labelifier.increment_tasks(),
            self.object.appearance.get_transform().size,
        );
        labelifier.queue(self.clone());
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TextVertex {
    rect: [Vertex; 4],
    extra: Extra,
}

impl TextVertex {
    pub fn indices(&self, id: u32) -> Vec<u32> {
        vec![id, 1 + id, 2 + id, 1 + id, 2 + id, 3 + id]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
struct Extra {
    id: usize,
}

impl std::hash::Hash for Extra {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_usize(self.id);
    }
}

/// A label maker holding
pub(crate) struct Labelifier {
    /// the default material,
    material: Material,
    /// RustType font cache,
    glyph_brush: GlyphBrush<TextVertex, Extra, FontArc, DefaultSectionHasher>,
    /// the global font texture,
    cache_pixel_buffer: Vec<u8>,
    /// tasks to be executed on next update,
    queued: Vec<DrawTask>,
    /// the amount of tasks
    tasks: usize,
    /// and the boolean if it should update.
    ready: bool,
}

impl Labelifier {
    /// Makes a new label maker.
    pub fn new() -> Result<Self> {
        let glyph_brush = GlyphBrushBuilder::using_fonts(vec![]).build(); // beginning fonts
        let cache_pixel_buffer = vec![
            0;
            (glyph_brush.texture_dimensions().0 * glyph_brush.texture_dimensions().1)
                as usize
        ];

        let dimensions = glyph_brush.texture_dimensions();
        let settings = TextureSettings {
            srgb: false,
            sampler: Sampler::default(),
        };

        // Make the cache a texture.
        let texture = Texture::from_raw(&cache_pixel_buffer, dimensions, Format::R8, 1, settings)?;

        let resources = &RESOURCES;
        let vulkan = resources.vulkan().clone();
        let text_shaders = Shaders::from_modules(
            vertex_shader(vulkan.device.clone())?,
            text_fragment_shader(vulkan.device.clone())?,
            "main",
        );

        let material_settings = MaterialSettingsBuilder::default()
            .texture(texture)
            .build()?;

        let material = Material::new_with_shaders(material_settings, &text_shaders, false, vec![])?;

        Ok(Self {
            material,
            glyph_brush,
            cache_pixel_buffer,
            queued: vec![],
            ready: false,
            tasks: 0,
        })
    }

    /// Increments the tasks number by one and returns the last id.
    fn increment_tasks(&mut self) -> usize {
        let tasks = self.tasks;
        self.tasks += 1;
        tasks
    }

    fn update_each_object(&mut self, brush_action: BrushAction<TextVertex>) -> Result<()> {
        let BrushAction::Draw(text_vertices) = brush_action else {
            return Ok(());
        };

        for text_vertex in text_vertices {
            let task = &mut self.queued[text_vertex.extra.id];
            task.indices
                .append(&mut text_vertex.indices(task.vertices.len() as u32));
            task.vertices.extend_from_slice(&text_vertex.rect);
        }

        // Creates a new texture to be inserted into every syncing label.
        // Unsynced label keep holding the old texture.

        // let dimensions = self.cache.dimensions();
        let settings = TextureSettings {
            srgb: false,
            sampler: Sampler::default(),
        };

        // Make the cache a texture.
        self.material.texture = Some(Texture::from_raw(
            &self.cache_pixel_buffer,
            self.glyph_brush.texture_dimensions(),
            Format::R8,
            1,
            settings,
        )?);

        let queued = std::mem::take(&mut self.queued);

        for task in queued.into_iter() {
            let mut label = task.label.clone();

            //temp
            let visible = if task.vertices.is_empty() {
                false
            } else {
                let model = ModelData::new(task.into_data())?;
                label.object.appearance.set_model(Model::Custom(model));
                true
            };
            label
                .object
                .appearance
                .set_material(Some(self.material.clone()));
            label.object.appearance.set_visible(visible);
            let node = label.object.as_node();
            let mut object = node.lock();
            object.object = label.object.clone();
        }
        Ok(())
    }

    /// Updates everything.
    pub fn update(&mut self) -> Result<()> {
        // Update the labelifier in case something has changed.
        if !self.ready {
            return Ok(());
        }

        let dimensions = self.glyph_brush.texture_dimensions();
        let brush_action = self.glyph_brush.process_queued(
            |rect, src_data| {
                let width = (rect.max[0] - rect.min[0]) as usize;
                let height = (rect.max[1] - rect.min[1]) as usize;
                let mut dst_index = (rect.min[1] * dimensions.0 + rect.min[0]) as usize;
                let mut src_index = 0;
                for _ in 0..height {
                    let dst_slice = &mut self.cache_pixel_buffer[dst_index..dst_index + width];
                    let src_slice = &src_data[src_index..src_index + width];
                    dst_slice.copy_from_slice(src_slice);

                    dst_index += dimensions.0 as usize;
                    src_index += width;
                }
            },
            to_vertex,
        );

        Self::update_each_object(self, brush_action.unwrap())?;

        self.tasks = 0;
        self.queued = vec![];
        self.ready = false;
        Ok(())
    }
    pub fn queue(&mut self, label: Label<Object>) {
        self.ready = true;

        self.glyph_brush.queue(label.section.to_borrowed());

        self.queued.push(DrawTask {
            label,
            vertices: vec![],
            indices: vec![],
        });
    }
}

fn to_vertex(
    glyph_brush::GlyphVertex {
        tex_coords,
        pixel_coords,
        bounds,
        extra,
    }: glyph_brush::GlyphVertex<Extra>,
) -> TextVertex {
    let rect = glyph_brush::Rectangle {
        min: [
            (pixel_coords.min.x / bounds.width() - 0.5) * 2.0,
            (pixel_coords.min.y / bounds.height() - 0.5) * 2.0,
        ],
        max: [
            (pixel_coords.max.x / bounds.width() - 0.5) * 2.0,
            (pixel_coords.max.y / bounds.height() - 0.5) * 2.0,
        ],
    };

    TextVertex {
        rect: [
            tvert(rect.min[0], rect.min[1], tex_coords.min.x, tex_coords.min.y),
            tvert(rect.min[0], rect.max[1], tex_coords.min.x, tex_coords.max.y),
            tvert(rect.max[0], rect.min[1], tex_coords.max.x, tex_coords.min.y),
            tvert(rect.max[0], rect.max[1], tex_coords.max.x, tex_coords.max.y),
        ],
        extra: *extra,
    }
    // let vertices: Vec<Vertex> = task
    //     .glyphs
    //     .clone()
    //     .iter()
    //     .flat_map(|g| {
    //         if let Ok(Some((uv_rect, screen_rect))) =
    //             self.glyph_brush.rect_for(label.font.id(), g)
    //         {
    //             let gl_rect = rusttype::Rect {
    //                 min: point(
    //                     (screen_rect.min.x as f32 / dimensions[0] - 0.5) * 2.0,
    //                     (screen_rect.min.y as f32 / dimensions[1] - 0.5) * 2.0,
    //                 ),
    //                 max: point(
    //                     (screen_rect.max.x as f32 / dimensions[0] - 0.5) * 2.0,
    //                     (screen_rect.max.y as f32 / dimensions[1] - 0.5) * 2.0,
    //                 ),
    //             };
    //             indices.extend([1 + id, 2 + id, id, 2 + id, id, 3 + id]);
    //             id += 4;
    //             vec![
    //                 Vertex {
    //                     position: vec2(gl_rect.min.x, gl_rect.max.y),
    //                     tex_position: vec2(uv_rect.min.x, uv_rect.max.y),
    //                 },
    //                 Vertex {
    //                     position: vec2(gl_rect.min.x, gl_rect.min.y),
    //                     tex_position: vec2(uv_rect.min.x, uv_rect.min.y),
    //                 },
    //                 Vertex {
    //                     position: vec2(gl_rect.max.x, gl_rect.min.y),
    //                     tex_position: vec2(uv_rect.max.x, uv_rect.min.y),
    //                 },
    //                 Vertex {
    //                     position: vec2(gl_rect.max.x, gl_rect.max.y),
    //                     tex_position: vec2(uv_rect.max.x, uv_rect.max.y),
    //                 },
    //             ]
    //             .into_iter()
    //         } else {
    //             vec![].into_iter()
    //         }
    //     })
    //     .collect();
}

struct DrawTask {
    pub label: Label<Object>,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    // pub glyphs: Vec<PositionedGlyph<'a>>,
}

impl DrawTask {
    pub fn into_data(self) -> Data {
        Data::Dynamic {
            vertices: self.vertices,
            indices: self.indices,
        }
    }
}

// fn layout_paragraph<'a>(label: &Label<Object>, dimensions: [f32; 2]) -> Vec<PositionedGlyph<'a>> {
//     if label.text.is_empty() {
//         return vec![];
//     };
//     let mut result: Vec<Vec<PositionedGlyph>> = vec![vec![]];
//     let scale = Scale {
//         x: label.scale[0],
//         y: label.scale[1],
//     };

//     let v_metrics = label.font.font().v_metrics(scale);
//     let advance_height = v_metrics.ascent - v_metrics.descent + v_metrics.line_gap;
//     let mut caret = point(0.0, v_metrics.ascent);
//     let mut last_glyph_id = None;
//     for c in label.text.chars() {
//         if c.is_control() {
//             match c {
//                 '\r' => {
//                     caret = point(0.0, caret.y + advance_height);
//                     result.push(vec![]);
//                 }
//                 '\n' => {}
//                 _ => {}
//             }
//             continue;
//         }
//         let base_glyph = label.font.font().glyph(c);
//         if let Some(id) = last_glyph_id.take() {
//             caret.x += label.font.font().pair_kerning(scale, id, base_glyph.id());
//         }
//         last_glyph_id = Some(base_glyph.id());
//         let mut glyph = base_glyph.scaled(scale).positioned(caret);
//         if let Some(bb) = glyph.pixel_bounding_box() {
//             if bb.max.x > dimensions[0] as i32 {
//                 result.push(vec![]);
//                 caret = point(0.0, caret.y + advance_height);
//                 glyph.set_position(caret);
//                 last_glyph_id = None;
//             }
//         }
//         caret.x += glyph.unpositioned().h_metrics().advance_width;
//         result.last_mut().unwrap().push(glyph);
//     }

//     let yshift = dimensions[1] - result.len() as f32 * advance_height + v_metrics.descent;
//     for line in result.clone().into_iter().enumerate() {
//         if let Some(last) = line.1.last() {
//             let xshift =
//                 dimensions[0] - last.position().x - last.unpositioned().h_metrics().advance_width;
//             for glyph in result[line.0].clone().iter().enumerate() {
//                 result[line.0][glyph.0].set_position(point(
//                     glyph.1.position().x + xshift * label.align[0],
//                     glyph.1.position().y + yshift * label.align[1],
//                 ))
//             }
//         };
//     }
//     result.into_iter().flatten().collect()
// }

/// A font to be used with the default label system.
#[derive(Clone)]
pub struct Font {
    id: FontId,
}

impl Font {
    /// Loads a font into the resources.
    ///
    /// Makes a new font using the bytes in a vec of a truetype or opentype font.
    /// Returns an error in case the given bytes do not work.
    pub fn from_vec(data: impl Into<Vec<u8>>) -> Result<Self> {
        let labelifier = &LABELIFIER;
        let font = FontArc::try_from_vec(data.into())?;
        let id = labelifier.lock().glyph_brush.add_font(font);
        Ok(Self { id })
    }
    /// Loads a font into the resources.
    ///
    /// Makes a new font using the bytes of a truetype or opentype font.
    /// Returns an error in case the given bytes do not work.
    pub fn from_slice(data: &'static [u8]) -> Result<Self> {
        let labelifier = &LABELIFIER;
        let font = FontArc::try_from_slice(data)?;
        let id = labelifier.lock().glyph_brush.add_font(font);
        Ok(Self { id })
    }
    /// Returns the font ID.
    pub fn id(&self) -> FontId {
        self.id
    }
}
