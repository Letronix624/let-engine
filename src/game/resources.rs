use hashbrown::HashMap;
use rusttype::{gpu_cache::Cache, Font, PositionedGlyph};
use std::rc::Rc;

/// Resources takes hold of all the resources into a HashMap filled with Rcs.
/// Before creating a game object with a game object builder you can make a Resources
/// struct. If you don't do that you won't be able to use textures, sounds and fonts.
pub struct Resources {
    pub textures: HashMap<String, (Rc<Vec<u8>>, (u32, u32), u32, u8)>, //bytes, (width, height), material, format
    pub fonts: HashMap<String, (Rc<Font<'static>>, usize)>,            //name -> font, fontid
    fontid: usize,
    pub cache: Cache<'static>,
    pub cache_pixel_buffer: Vec<u8>,
    pub sounds: HashMap<String, Rc<Vec<u8>>>,
}

impl Resources {
    pub fn new() -> Self {
        let textures = HashMap::new();
        let fonts = HashMap::new();
        let cache = Cache::builder().dimensions(512, 512).build();
        let cache_pixel_buffer = vec![0; (cache.dimensions().0 * cache.dimensions().1) as usize];
        let sounds = HashMap::new();

        Self {
            textures,
            fonts,
            fontid: 0,
            cache,
            cache_pixel_buffer,
            sounds,
        }
    }
    pub fn add_texture(&mut self, name: &str, texture: Vec<u8>, width: u32, height: u32) {
        let texture = (Rc::new(texture), (width, height), 1, 0);
        self.textures.insert(name.into(), texture);
    }
    pub fn add_font_bytes(&mut self, name: &str, font: &[u8]) {
        let font = Rc::new(Font::try_from_vec(font.to_vec()).unwrap());

        self.fonts.insert(name.into(), (font.clone(), self.fontid));

        self.fontid += 1;
        //Self::update_cache(self);
    }
    pub fn update_cache(&mut self, font: &str, glyphs: Vec<PositionedGlyph<'static>>) {
        let dimensions = 512;

        let font = self.fonts.get(font).unwrap().clone();

        for glyph in glyphs {
            self.cache.queue_glyph(font.1, glyph);
        }

        self.cache
            .cache_queued(|rect, src_data| {
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
            })
            .unwrap();

        self.textures.insert(
            "fontatlas".into(),
            (
                Rc::new(self.cache_pixel_buffer.clone()),
                (dimensions as u32, dimensions as u32),
                1,
                1,
            ),
        );
    }
    pub fn add_sound(&mut self, name: &str, sound: &[u8]) {
        let sound = Rc::new(sound.to_vec());
        self.sounds.insert(name.into(), sound);
    }
    pub fn remove_font(&mut self, name: &str) {
        self.fonts.remove(name);
    }
    pub fn remove_sound(&mut self, name: &str) {
        self.textures.remove(name);
    }
    pub fn remove_texture(&mut self, name: &str) {
        self.sounds.remove(name);
    }
}
