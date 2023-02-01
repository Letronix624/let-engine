use rusttype::Font;
use std::{collections::HashMap, sync::Arc};

/// Resources takes hold of all the resources into a HashMap filled with Arcs.
/// Before creating a game object with a game object builder you can make a Resources
/// struct. If you don't do that you won't be able to use textures, sounds and fonts.
#[derive(Clone)]
pub struct Resources {
    pub textures: HashMap<String, Arc<Vec<u8>>>,
    pub fonts: HashMap<String, Arc<Font<'static>>>,
    pub sounds: HashMap<String, Arc<Vec<u8>>>,
}

impl Resources {
    pub fn new() -> Self {
        let textures = HashMap::new();
        let fonts = HashMap::new();
        let sounds = HashMap::new();

        Self {
            textures,
            fonts,
            sounds,
        }
    }
    pub fn add_texture(&mut self, name: &str, texture: &[u8]) {
        let texture = Arc::new(texture.to_vec());
        self.textures.insert(name.into(), texture);
    }
    pub fn add_font(&mut self, name: &str, font: &[u8]) {
        let font = Arc::new(Font::try_from_vec(font.to_vec()).unwrap());
        self.fonts.insert(name.into(), font);
    }
    pub fn add_sound(&mut self, name: &str, sound: &[u8]) {
        let sound = Arc::new(sound.to_vec());
        self.sounds.insert(name.into(), sound);
    }
}
