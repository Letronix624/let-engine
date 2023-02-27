use hashbrown::HashMap;
use rusttype::{gpu_cache::Cache, point, Font, PositionedGlyph};
use std::sync::Arc;

/// Resources takes hold of all the resources into a HashMap filled with Arcs.
/// Before creating a game object with a game object builder you can make a Resources
/// struct. If you don't do that you won't be able to use textures, sounds and fonts.
pub struct Resources {
    pub textures: HashMap<String, (Arc<Vec<u8>>, u32, u32)>, //bytes, width, height
    pub fonts: HashMap<String, (String, Arc<Font<'static>>, f32, usize)>, //name -> characters, font, size, fontid
    pub cache: Cache<'static>,
    pub cache_texture: Vec<u8>,
    pub sounds: HashMap<String, Arc<Vec<u8>>>,
}

impl Resources {
    pub fn new() -> Self {
        let textures = HashMap::new();
        let fonts = HashMap::new();
        let cache = Cache::builder().dimensions(256, 256).build();
        let cache_texture = vec![0u8; 256 * 256];
        let sounds = HashMap::new();

        Self {
            textures,
            fonts,
            cache,
            cache_texture,
            sounds,
        }
    }
    pub fn add_texture(&mut self, name: &str, texture: Vec<u8>, width: u32, height: u32) {
        let texture = (Arc::new(texture), width, height);
        self.textures.insert(name.into(), texture);
    }
    pub fn add_font_bytes(&mut self, name: &str, size: f32, font: &[u8], characters: Vec<char>) {
        let mut string = String::new();

        let fontid = self.fonts.len();

        for i in 0 as char..255 as char {
            string.push(i)
        }
        for i in characters {
            if !string.contains(i) {
                string.push(i)
            }
        }

        let font = Arc::new(Font::try_from_vec(font.to_vec()).unwrap());

        self.fonts
            .insert(name.into(), (string, font.clone(), size, fontid));
        Self::update_cache(self);
    }
    fn update_cache(&mut self) {
        let mut dimensions = 256;
        let mut cache_pixel_buffer: Vec<u8> = vec![0; dimensions * dimensions];

        loop {
            self.cache = Cache::builder()
                .dimensions(dimensions as u32, dimensions as u32)
                .build();

            for (string, font, size, fontid) in self.fonts.values() {
                let glyphs: Vec<PositionedGlyph> = font
                    .layout(&string, rusttype::Scale::uniform(*size), point(0.0, 0.0))
                    .collect();

                for glyph in &glyphs {
                    self.cache.queue_glyph(*fontid, glyph.clone());
                }
            }

            match self.cache.cache_queued(|rect, src_data| {
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
            }) {
                Ok(_) => break,
                Err(rusttype::gpu_cache::CacheWriteErr::NoRoomForWholeQueue) => {
                    dimensions *= 2;
                }
                Err(e) => panic!("{e}"),
            };
        }
        self.cache_texture = cache_pixel_buffer;
    }
    pub fn add_sound(&mut self, name: &str, sound: &[u8]) {
        let sound = Arc::new(sound.to_vec());
        self.sounds.insert(name.into(), sound);
    }
    pub fn remove_font(&mut self, name: &str) {
        self.fonts.remove(name);
        Self::update_cache(self)
    }
}
