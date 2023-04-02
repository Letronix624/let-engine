use rusttype::Font;
use std::sync::Arc;
use vulkano::descriptor_set::persistent::PersistentDescriptorSet;

/// Resources takes hold of all the resources into a HashMap filled with Rcs.
pub struct Resources {
    fontid: usize,
}

impl Resources {
    pub fn new() -> Self {
        Self { fontid: 0 }
    }
    /// Loads a font ready to get layed out and rendered.
    pub fn load_font(&mut self, font: &[u8]) -> Arc<GameFont> {
        let font = Arc::new(GameFont {
            font: Font::try_from_vec(font.to_vec()).unwrap(),
            fontid: self.fontid,
        });
        self.fontid += 1;
        font
    }
}

#[derive(Clone)]
pub struct Texture {
    pub data: Vec<u8>,
    pub dimensions: (u32, u32),
    pub material: u32,
    pub set: Arc<PersistentDescriptorSet>,
}

pub struct GameFont {
    pub font: Font<'static>,
    pub fontid: usize,
}

pub struct Sound {
    pub data: Vec<u8>,
}

impl Texture {
    /// Loads the texture with given rgba8 data and dimensions to the gpu
    /// ready to be used with an object appearance.
    pub fn new(
        texture: Vec<u8>,
        dimensions: (u32, u32),
        set: Arc<PersistentDescriptorSet>,
        material: u32,
    ) -> Arc<Texture> {
        Arc::new(Texture {
            data: texture,
            dimensions,
            material,
            set: set,
        })
    }
}

impl PartialEq for Texture {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
            && self.dimensions == other.dimensions
            && self.material == other.material
            && Arc::ptr_eq(&self.set, &other.set)
    }
    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

impl std::fmt::Debug for Texture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Texture")
            .field("size", &self.data.len())
            .field("dimensions", &self.dimensions)
            .field("material", &self.material)
            .finish()
    }
}

/// Not done.
#[allow(dead_code)]
pub fn load_sound(sound: &[u8]) -> Arc<Sound> {
    Arc::new(Sound {
        data: sound.to_vec(),
    })
}
