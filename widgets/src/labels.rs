//! Default labels given by the engine.

use std::sync::LazyLock;

use ab_glyph::FontArc;
use crossbeam::channel::{unbounded, Receiver, Sender};
use glyph_brush::ab_glyph::PxScale;
use glyph_brush::{
    ab_glyph, BrushAction, BrushError, FontId, GlyphBrush, GlyphBrushBuilder, HorizontalAlign,
    Layout, Section, SectionBuilder, Text, VerticalAlign,
};
use let_engine_core::backend::graphics::{GraphicsInterface, Loaded};
use let_engine_core::objects::{AppearanceBuilder, Color, Descriptor};
use let_engine_core::resources::buffer::{
    Buffer, BufferAccess, BufferUsage, LoadedBuffer, Location,
};
use let_engine_core::resources::data::{tvert, TVert};
use let_engine_core::resources::material::{
    GraphicsShaders, Material, MaterialSettingsBuilder, Topology,
};

use anyhow::{Context, Result};

use glam::{uvec2, vec2, UVec2, Vec2};
use let_engine_core::resources::model::{LoadedModel, Model};
use let_engine_core::resources::texture::{
    LoadedTexture, Texture, TextureSettings, TextureSettingsBuilder, ViewTypeDim,
};
use let_engine_core::resources::Format;
use let_engine_core::{objects::Transform, Direction};

/// Info to create default label objects with.
#[derive(Clone)]
pub struct LabelCreateInfo {
    /// Initial position of created appearances.
    pub transform: Transform,

    /// Color of the text.
    pub text_color: Color,

    /// The extent of the label area in pixels.
    pub extent: UVec2,

    /// The scale of the text area.
    pub scale: Vec2,

    /// The font used with this label.
    pub font: Font,

    /// The align of where the text gets rendered.
    pub align: Direction,

    /// Initial text of the label.
    pub text: String,
}

impl LabelCreateInfo {
    /// Sets the transform of the label and returns it back.
    #[inline]
    pub fn transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    /// Sets the font of the label and returns it back.
    #[inline]
    pub fn font(mut self, font: Font) -> Self {
        self.font = font;
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

    /// Sets the color of the text and returns the builder back.
    #[inline]
    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = color;
        self
    }

    /// Sets the extent of the label and returns it back.
    #[inline]
    pub fn extent<T>(mut self, extent: T) -> Self
    where
        T: Into<UVec2>,
    {
        self.extent = extent.into();
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
            font: Font::default(),
            text: String::new(),
            text_color: Color::WHITE,
            extent: uvec2(1000, 1000),
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
#[derive(Clone, Debug)]
pub struct Label<T: Loaded + 'static> {
    text_color: Color,
    pub extent: UVec2,
    pub scale: Vec2,
    pub align: Direction,
    pub font: Font,
    pub transform: Transform,
    sender: Sender<Self>,
    material_id: T::MaterialId,
    buffer_id: T::BufferId<Color>,
    model_id: T::ModelId<TVert>,
    texture_id: T::TextureId,
    pub text: String,

    buffer_changed: bool,
}

impl<T: Loaded + 'static> Label<T> {
    /// Creates a new label with the given settings.
    pub fn new(
        create_info: LabelCreateInfo,
        labelifier: &mut Labelifier<T>,
        graphics_interface: &impl GraphicsInterface<T>,
    ) -> Result<Self> {
        use let_engine_core::resources::buffer::BufferAccess;

        let model = Model::<TVert>::new_maxed(
            vec![tvert(0.0, 0.0, 0.0, 0.0)],
            // TEMP: 1024 character limit, TODO: Give user choice to set.
            1024 * 6,
            BufferAccess::Staged,
        );

        let buffer = Buffer::from_data(
            BufferUsage::Uniform,
            BufferAccess::Pinned(let_engine_core::resources::buffer::PreferOperation::Write),
            create_info.text_color,
        );

        let label = Self {
            model_id: graphics_interface.load_model(&model)?,
            buffer_id: graphics_interface.load_buffer(&buffer)?,
            material_id: labelifier.material_id,
            texture_id: labelifier.texture_id,
            font: create_info.font,
            transform: create_info.transform,
            text: create_info.text,
            text_color: create_info.text_color,
            extent: create_info.extent,
            scale: create_info.scale,
            align: create_info.align,
            sender: labelifier.sender.clone(),
            buffer_changed: false,
        };

        labelifier.sender.send(label.clone())?;

        Ok(label)
    }

    /// Queues the model to be updated after running [`Labelifier::update`]
    pub fn queue(&mut self) -> Result<()> {
        self.sender
            .send(self.clone())
            .context("failed to queue label to labelifier")?;

        self.buffer_changed = false;
        Ok(())
    }

    /// Returns the appearance of this label to be used with objects for label objects.
    pub fn appearance(&self) -> AppearanceBuilder<T> {
        let mut transform = self.transform;
        transform.size *= self.extent.as_vec2();
        AppearanceBuilder::default()
            .transform(transform)
            .model(self.model_id)
            .material(self.material_id)
            .descriptors(&[
                (Location::new(0, 0), Descriptor::Mvp),
                (Location::new(1, 0), Descriptor::buffer(self.buffer_id)),
                (Location::new(2, 0), Descriptor::Texture(self.texture_id)),
            ])
    }
}

impl<T: Loaded + 'static> Label<T> {
    /// Create a GlyphBrush section out of this label description.
    pub(crate) fn create_section(&self, id: usize) -> Section<usize> {
        let extent = self.extent.as_vec2();

        let text = Text {
            text: &self.text,
            scale: PxScale {
                x: self.scale.x,
                y: self.scale.y,
            },
            font_id: self.font.id(),
            extra: id,
        };

        let (h, v): (HorizontalAlign, VerticalAlign) = glyph_direction(self.align);
        let x = match h {
            HorizontalAlign::Left => 0.0,
            HorizontalAlign::Center => extent[0] * 0.5,
            HorizontalAlign::Right => extent[0],
        };
        let y = match v {
            VerticalAlign::Top => 0.0,
            VerticalAlign::Center => extent[1] * 0.5,
            VerticalAlign::Bottom => extent[1],
        };

        SectionBuilder::default()
            .with_bounds(extent)
            .with_layout(Layout::default().h_align(h).v_align(v))
            .with_screen_position((x, y))
            .add_text(text)
    }

    /// Sets the color of this label to the given color without queuing it.
    #[inline]
    pub fn set_text_color(&mut self, color: Color) {
        self.text_color = color;
        self.buffer_changed = true;
    }

    /// Sets the color of this label and immediately queues it.
    pub fn update_text_color(&mut self, color: Color) -> Result<()> {
        self.text_color = color;
        self.buffer_changed = true;
        self.queue()
    }

    /// Sets the text of this label without queueing it.
    #[inline]
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// Sets the text of this label and immediately queues it.
    pub fn update_text(&mut self, text: impl Into<String>) -> Result<()> {
        self.set_text(text);
        self.queue()
    }

    /// Sets the extent of this label without queueing it.
    #[inline]
    pub fn set_extent(&mut self, extent: UVec2) {
        self.extent = extent;
    }

    /// Sets the extent of this label and immediately queues it.
    pub fn update_extent(&mut self, extent: UVec2) -> Result<()> {
        self.extent = extent;
        self.queue()
    }

    /// Sets the scale of this label without queueing it.
    #[inline]
    pub fn set_scale(&mut self, scale: Vec2) {
        self.scale = scale;
    }

    /// Sets the scale of this label and immediately queues it.
    pub fn update_scale(&mut self, scale: Vec2) -> Result<()> {
        self.scale = scale;
        self.queue()
    }

    /// Sets the align of this label without queueing it.
    #[inline]
    pub fn set_align(&mut self, align: Direction) {
        self.align = align;
    }

    /// Sets the align of this label and immediately queues it.
    pub fn update_align(&mut self, align: Direction) -> Result<()> {
        self.align = align;
        self.queue()
    }

    /// Sets the font of this label without queueing it.
    #[inline]
    pub fn set_font(&mut self, font: Font) {
        self.font = font;
    }

    /// Sets the font of this label and immediately queues it.
    pub fn update_font(&mut self, font: Font) -> Result<()> {
        self.font = font;
        self.queue()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TextVertex {
    rect: [TVert; 6],
    id: usize,
}

/// A label maker holding
pub struct Labelifier<T: Loaded + 'static> {
    material_id: T::MaterialId,

    texture_id: T::TextureId,

    /// glyph brush font cache,
    glyph_brush: GlyphBrush<TextVertex, usize, FontArc, foldhash::fast::RandomState>,

    sender: Sender<Label<T>>,
    receiver: Receiver<Label<T>>,
}

static TEXTURE_SETTINGS: LazyLock<TextureSettings> = LazyLock::new(|| {
    TextureSettingsBuilder::default()
        .access_pattern(BufferAccess::Staged)
        .unwrap()
        .format(Format::R8Unorm)
        .build()
        .unwrap()
});

impl<T: Loaded + 'static> Labelifier<T> {
    /// Makes a new label maker.
    pub fn new(interface: &impl GraphicsInterface<T>) -> Result<Self> {
        let glyph_brush = GlyphBrushBuilder::using_fonts(vec![])
            .section_hasher(foldhash::fast::RandomState::default())
            .build(); // beginning fonts

        let dimensions = glyph_brush.texture_dimensions();

        let material_settings = MaterialSettingsBuilder::default()
            .topology(Topology::TriangleList)
            .build()?;

        let text_shaders = GraphicsShaders::new(
            include_bytes!("shaders/text_vert.spv").to_vec(),
            "main".to_string(),
            include_bytes!("shaders/text_frag.spv").to_vec(),
            "main".to_string(),
        );

        let material = Material::new(material_settings, text_shaders);

        // Make the cache a texture.
        let texture = Texture::new_empty(dimensions.into(), TEXTURE_SETTINGS.to_owned())?;

        let material = interface.load_material::<TVert>(&material)?;
        let texture = interface.load_texture(&texture)?;

        let (sender, receiver) = unbounded();

        Ok(Self {
            material_id: material,
            texture_id: texture,
            glyph_brush,
            sender,
            receiver,
        })
    }

    /// Clears every glyph from the cache, resizing the texture cache back to 256x256 pixels.
    pub fn clear_cache(&mut self, interface: &impl GraphicsInterface<T>) -> Result<()> {
        self.glyph_brush
            .to_builder()
            .initial_cache_size((256, 256))
            .rebuild(&mut self.glyph_brush);
        let dims = self.glyph_brush.texture_dimensions();
        let new_texture = Texture::new_empty(dims.into(), TEXTURE_SETTINGS.to_owned())?;

        self.texture_id = interface.load_texture(&new_texture)?;

        Ok(())
    }

    /// Loads a font into the resources.
    ///
    /// Makes a new font using the bytes in a vec of a truetype or opentype font.
    /// Returns an error in case the given bytes do not work.
    pub fn font_from_vec(&mut self, data: impl Into<Vec<u8>>) -> Result<Font> {
        let font = FontArc::try_from_vec(data.into())?;
        let id = self.glyph_brush.add_font(font);
        Ok(Font { id })
    }

    /// Loads a font into the resources.
    ///
    /// Makes a new font using the bytes of a truetype or opentype font.
    /// Returns an error in case the given bytes do not work.
    pub fn font_from_slice(&mut self, data: &'static [u8]) -> Result<Font> {
        let font = FontArc::try_from_slice(data)?;
        let id = self.glyph_brush.add_font(font);
        Ok(Font { id })
    }

    // gets called in the `update` method
    fn update_models(
        mut queued: Vec<DrawTask<T>>,
        brush_action: BrushAction<TextVertex>,
        interface: &impl GraphicsInterface<T>,
    ) -> Result<()> {
        let BrushAction::Draw(text_vertices) = brush_action else {
            return Ok(());
        };

        for text_vertex in text_vertices {
            let task = &mut queued[text_vertex.id];
            task.vertices.extend_from_slice(&text_vertex.rect);
        }

        for DrawTask { label, vertices } in queued.into_iter() {
            let model = interface.model(label.model_id).unwrap();

            // write new vertex buffer
            if vertices.is_empty() {
                // Set a single vertex to hide
                model.write_vertices(|_| (), 1)?;
            } else {
                model.write_vertices(|write| write.copy_from_slice(&vertices), vertices.len())?;
            }
        }
        Ok(())
    }

    /// Updates the texture and model of every single label queued to be updated.
    ///
    /// Should be called every frame.
    pub fn update(&mut self, interface: &impl GraphicsInterface<T>) -> Result<()> {
        let mut queue = vec![];
        while let Ok(label) = self.receiver.try_recv() {
            let id = queue.len();
            self.glyph_brush.queue(label.create_section(id));

            if label.buffer_changed {
                let buffer = interface.buffer(label.buffer_id).unwrap();

                buffer.write_data(|color| *color = label.text_color)?;
            }

            queue.push(DrawTask {
                label,
                vertices: vec![],
            });
        }

        if queue.is_empty() {
            return Ok(());
        }

        let brush_action: glyph_brush::BrushAction<TextVertex> = loop {
            let result = self.glyph_brush.process_queued(
                |rect, src_data| {
                    let texture = interface.texture(self.texture_id).unwrap();
                    let ViewTypeDim::D2 {
                        x: texture_width, ..
                    } = texture.dimensions()
                    else {
                        return;
                    };

                    texture
                        .write_data(|texture| {
                            let width = (rect.max[0] - rect.min[0]) as usize;
                            let height = (rect.max[1] - rect.min[1]) as usize;

                            for y in 0..height {
                                let texture_row_start = (rect.min[1] as usize + y)
                                    * *texture_width as usize
                                    + rect.min[0] as usize;
                                let src_row_start = y * width;

                                texture[texture_row_start..texture_row_start + width]
                                    .copy_from_slice(
                                        &src_data[src_row_start..src_row_start + width],
                                    );
                            }
                        })
                        .unwrap();
                },
                to_vertex,
            );
            match result {
                Ok(brush_action) => {
                    break brush_action;
                }
                Err(BrushError::TextureTooSmall {
                    suggested: (width, height),
                }) => {
                    self.glyph_brush.resize_texture(width, height);
                    todo!(); // TODO
                             // self.texture.resize((width, height).into())?;
                }
            }
        };

        Self::update_models(queue, brush_action, interface)?;

        Ok(())
    }

    /// Returns the global material of all labels.
    pub fn material_id(&self) -> &T::MaterialId {
        &self.material_id
    }

    /// Returns the global shared texture of all labels.
    pub fn texture_id(&self) -> &T::TextureId {
        &self.texture_id
    }
}

fn to_vertex(
    glyph_brush::GlyphVertex {
        tex_coords,
        pixel_coords,
        bounds,
        extra,
    }: glyph_brush::GlyphVertex<usize>,
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

    let top_left = tvert(rect.min[0], rect.min[1], tex_coords.min.x, tex_coords.min.y);
    let top_right = tvert(rect.max[0], rect.min[1], tex_coords.max.x, tex_coords.min.y);
    let bottom_left = tvert(rect.min[0], rect.max[1], tex_coords.min.x, tex_coords.max.y);
    let bottom_right = tvert(rect.max[0], rect.max[1], tex_coords.max.x, tex_coords.max.y);

    TextVertex {
        rect: [
            bottom_left,
            bottom_right,
            top_right,
            bottom_left,
            top_right,
            top_left,
        ],
        id: *extra,
    }
}

struct DrawTask<T: Loaded + 'static> {
    pub label: Label<T>,
    pub vertices: Vec<TVert>,
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
///
/// Should be used with the same labelifier with which it was created.
#[derive(Copy, Clone, Debug, Default)]
pub struct Font {
    id: FontId,
}

impl Font {
    /// Returns the font ID.
    pub fn id(&self) -> FontId {
        self.id
    }
}

fn glyph_direction(value: Direction) -> (glyph_brush::HorizontalAlign, glyph_brush::VerticalAlign) {
    use glyph_brush::{HorizontalAlign, VerticalAlign};
    let horizontal = match value {
        Direction::Center => HorizontalAlign::Center,
        Direction::N => HorizontalAlign::Center,
        Direction::No => HorizontalAlign::Right,
        Direction::O => HorizontalAlign::Right,
        Direction::So => HorizontalAlign::Right,
        Direction::S => HorizontalAlign::Center,
        Direction::Sw => HorizontalAlign::Left,
        Direction::W => HorizontalAlign::Left,
        Direction::Nw => HorizontalAlign::Left,
    };

    let vertical = match value {
        Direction::Center => VerticalAlign::Center,
        Direction::N => VerticalAlign::Top,
        Direction::No => VerticalAlign::Top,
        Direction::O => VerticalAlign::Center,
        Direction::So => VerticalAlign::Bottom,
        Direction::S => VerticalAlign::Bottom,
        Direction::Sw => VerticalAlign::Bottom,
        Direction::W => VerticalAlign::Center,
        Direction::Nw => VerticalAlign::Top,
    };
    (horizontal, vertical)
}
