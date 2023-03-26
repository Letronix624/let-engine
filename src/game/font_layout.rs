use std::rc::Rc;

use rusttype::{point, PositionedGlyph, Font};

use crate::{Game, Vertex, Appearance, Data};




pub fn get_data(
    game: &mut Game,
    font: &str,
    text: &str,
    size: f32,
    color: [f32; 4],
) -> Option<Appearance> {
    let fontname = font;
    let font = game.resources.fonts.get(font).unwrap().clone();

    let glyphs: Vec<PositionedGlyph> = layout(font.0, text, size);

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
                    Vertex { // 0
                        position: [gl_rect.min.x, gl_rect.max.y],
                        tex_position: [uv_rect.min.x, uv_rect.max.y],
                    },
                    Vertex { // 1
                        position: [gl_rect.min.x, gl_rect.min.y],
                        tex_position: [uv_rect.min.x, uv_rect.min.y],
                    },
                    Vertex { // 2
                        position: [gl_rect.max.x, gl_rect.min.y],
                        tex_position: [uv_rect.max.x, uv_rect.min.y],
                    },
                    // Vertex { // 2
                    //     position: [gl_rect.max.x, gl_rect.min.y],
                    //     tex_position: [uv_rect.max.x, uv_rect.min.y],
                    // },
                    Vertex { // 3
                        position: [gl_rect.max.x, gl_rect.max.y],
                        tex_position: [uv_rect.max.x, uv_rect.max.y],
                    },
                    // Vertex { // 0
                    //     position: [gl_rect.min.x, gl_rect.max.y],
                    //     tex_position: [uv_rect.min.x, uv_rect.max.y],
                    // },
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
            vertices: vertices,
            indices: indices,
        },
        //data: Data::square(),
        color,
        material: 2,
        ..Appearance::empty()
    };
    //game.textobjects.push(object.clone());
    Some(object)
}

fn layout<'a>(    
    font: Rc<Font<'static>>,
    text: &str,
    size: f32,
) -> Vec<PositionedGlyph<'a>> {
    font
    .layout(
        text, //text,
        rusttype::Scale::uniform(size),
        point(0.0, font.v_metrics(rusttype::Scale::uniform(size)).ascent),
    )
    .collect()
}