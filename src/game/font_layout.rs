use std::rc::Rc;

use rusttype::{point, Font, PositionedGlyph, Scale};

use crate::{Appearance, Data, Game, Vertex};

pub fn get_data(
    game: &mut Game,
    font: &str,
    text: &str,
    size: f32,
    color: [f32; 4],
    binding: [f32; 2],
) -> Option<Appearance> {
    let fontname = font;
    let font = game.resources.fonts.get(font).unwrap().clone();

    let glyphs: Vec<PositionedGlyph> = layout_paragraph(font.0, text, size, binding);

    game.resources.update_cache(fontname, glyphs.clone());

    let dimensions: [u32; 2] = [1000; 2];

    let mut indices: Vec<u32> = vec![];

    let mut id = 0;

    let vertices: Vec<Vertex> = glyphs
        .clone()
        .iter()
        .flat_map(|g| {
            if let Ok(Some((uv_rect, screen_rect))) = game.resources.cache.rect_for(font.1, g) {
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
                        // 0
                        position: [gl_rect.min.x, gl_rect.max.y],
                        tex_position: [uv_rect.min.x, uv_rect.max.y],
                    },
                    Vertex {
                        // 1
                        position: [gl_rect.min.x, gl_rect.min.y],
                        tex_position: [uv_rect.min.x, uv_rect.min.y],
                    },
                    Vertex {
                        // 2
                        position: [gl_rect.max.x, gl_rect.min.y],
                        tex_position: [uv_rect.max.x, uv_rect.min.y],
                    },
                    Vertex {
                        // 3
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
    game.draw.update_font_objects(&game.vulkan, &game.resources);
    let object = Appearance {
        texture: Some("fontatlas".to_string()),
        data: Data {
            vertices,
            indices,
        },
        color,
        material: 2,
        ..Appearance::empty()
    };
    Some(object)
}

fn layout_paragraph<'a>(
    //NW
    font: Rc<Font<'static>>,
    text: &str,
    size: f32,
    binding: [f32; 2],
) -> Vec<PositionedGlyph<'a>> {
    if text == "" {
        return vec![];
    };
    let mut result: Vec<Vec<PositionedGlyph>> = vec![vec![]];
    let scale = Scale::uniform(size);
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
            if bb.max.x > 1000 as i32 {
                result.push(vec![]);
                caret = point(0.0, caret.y + advance_height);
                glyph.set_position(caret);
                last_glyph_id = None;
            }
        }
        caret.x += glyph.unpositioned().h_metrics().advance_width;
        result.last_mut().unwrap().push(glyph);
    }

    let yshift = 1000.0 - result.len() as f32 * advance_height + v_metrics.descent;
    for line in result.clone().into_iter().enumerate() {
        if let Some(last) = line.1.last() {
            let xshift = 1000.0 - last.position().x - last.unpositioned().h_metrics().advance_width;
            for glyph in result[line.0].clone().iter().enumerate() {
                result[line.0][glyph.0].set_position(point(
                    glyph.1.position().x + xshift * binding[0],
                    glyph.1.position().y + yshift * binding[1],
                ))
            }
        };
    }
    result.into_iter().flatten().collect()
}
