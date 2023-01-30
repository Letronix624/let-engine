use image::{ImageBuffer, Rgb, Rgba};
use rusttype::Font;
use std::{collections::HashMap, io::Cursor, sync::Arc};
use vulkano::image::ImageDimensions;

#[derive(Clone)]
pub struct Resources {
    pub textures: HashMap<String, Arc<(Vec<u8>, ImageDimensions)>>,
    pub fonts: HashMap<String, Arc<Font<'static>>>,
    pub sounds: HashMap<String, Arc<Vec<u8>>>,
}

impl Resources {
    pub fn load_all() -> Self {
        let mut textures = HashMap::new();
        let mut fonts = HashMap::new();
        let mut sounds = HashMap::new();

        println!("\nLoading Rusty...");
        let texture = Arc::new(load_texture(
            include_bytes!("../assets/textures/rusty.png").to_vec(),
        ));
        textures.insert("rusty".into(), texture);
        println!("Loaded Rusty!\nLoading fonts...");
        let font = Arc::new({
            let font_data = include_bytes!("../assets/fonts/Bani-Regular.ttf");
            Font::try_from_bytes(font_data).unwrap()
        });
        fonts.insert("Bani-Regular".into(), font);
        println!("Loaded fonts!\nLoading sounds...");
        let sound = Arc::new(include_bytes!("../assets/sounds/boom.mp3").to_vec());

        sounds.insert("boom".into(), sound);
        println!("Loaded sounds!\n\nLoading complete.");
        Self {
            textures,
            fonts,
            sounds,
        }
    }
    // pub fn load_texture(&mut self) {

    // }
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

fn load_texture(png_bytes: Vec<u8>) -> (Vec<u8>, ImageDimensions) {
    let cursor = Cursor::new(png_bytes);
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
