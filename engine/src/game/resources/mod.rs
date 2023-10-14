//! Resources to be handled by the engine like textures, sounds and fonts.

use super::Labelifier;
use crate::window::Window;
use crate::{error::textures::*, utils::u16tou8vec};
use image::{load_from_memory_with_format, DynamicImage, ImageFormat};
use parking_lot::Mutex;
use std::sync::Arc;
use vulkano::buffer::BufferContents;
use vulkano::descriptor_set::persistent::PersistentDescriptorSet;
use vulkano::descriptor_set::WriteDescriptorSet;
use vulkano::pipeline::cache::PipelineCache;

mod loader;
pub(crate) mod vulkan;
pub(crate) use loader::Loader;
use vulkan::Vulkan;

pub mod textures;
use textures::*;

pub mod data;
pub mod materials;
mod model;
pub use model::Model;
mod macros;
pub use macros::*;

const NOT_INITIALIZED_MSG: &str = "Resources are not initialized to a game.";

/// All the resources kept in the game engine like textures, fonts, sounds and models.
#[derive(Clone)]
pub struct Resources {
    pub(crate) vulkan: Option<Vulkan>,
    pub(crate) loader: Option<Arc<Mutex<Loader>>>,
    pub(crate) labelifier: Option<Arc<Mutex<Labelifier>>>,
}

impl Resources {
    pub fn new() -> Self {
        Self {
            vulkan: None,
            loader: None,
            labelifier: None,
        }
    }

    /// Initialisation
    pub(crate) fn init(&mut self, vulkan: Vulkan) {
        let loader = Loader::init(&vulkan);
        let labelifier = Some(Arc::new(Mutex::new(Labelifier::new(self))));
        *self = Self {
            vulkan: Some(vulkan),
            loader: Some(Arc::new(Mutex::new(loader))),
            labelifier,
        }
    }

    pub(crate) fn vulkan(&self) -> &Vulkan {
        self.vulkan.as_ref().expect(NOT_INITIALIZED_MSG)
    }
    pub(crate) fn loader(&self) -> &Arc<Mutex<Loader>> {
        self.loader.as_ref().expect(NOT_INITIALIZED_MSG)
    }
    pub(crate) fn labelifier(&self) -> &Arc<Mutex<Labelifier>> {
        self.labelifier.as_ref().expect(NOT_INITIALIZED_MSG)
    }
    //redraw
    pub(crate) fn update(&self) {
        // swap with layers
        let mut labelifier = self.labelifier().lock();
        labelifier.update(self);
    }

    /// Merges a pipeline cache into the resources potentially making the creation of materials faster.
    ///
    /// # Safety
    ///
    /// Unsafe because vulkan blindly trusts that this data comes from the `get_pipeline_binary` function.
    /// The program will crash if the data provided is not right.
    ///
    /// The binary given to the function must be made with the same hardware and vulkan driver version.
    pub unsafe fn load_pipeline_cache(&self, data: &[u8]) {
        let cache = PipelineCache::with_data(self.vulkan().device.clone(), data).unwrap();
        self.loader()
            .lock()
            .pipeline_cache
            .merge([&cache].iter())
            .unwrap();
    }

    /// Returns the binary of the pipeline cache.
    ///
    /// Allows this binary to be loaded with the `load_pipeline_cache` function to make loading materials potentially faster.
    pub fn get_pipeline_binary(&self) -> Vec<u8> {
        self.loader().lock().pipeline_cache.get_data().unwrap()
    }

    /// Loads a new write operation for a shader.
    pub fn new_descriptor_write<T: BufferContents>(&self, buf: T, set: u32) -> WriteDescriptorSet {
        let loader = self.loader().lock();
        loader.write_descriptor(buf, set)
    }

    /// Returns the window instance from resources.
    pub fn get_window(&self) -> Window {
        self.vulkan().window.clone()
    }
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}

/// A texture to be used with materials.
#[derive(Clone)]
pub struct Texture {
    data: Arc<[u8]>,
    dimensions: (u32, u32),
    layers: u32,
    set: Arc<PersistentDescriptorSet>,
}

/// Making
impl Texture {
    /// Loads a texture to the GPU using a raw image.
    pub fn from_raw(
        data: &[u8],
        dimensions: (u32, u32),
        format: Format,
        layers: u32,
        settings: TextureSettings,
        resources: &Resources,
    ) -> Texture {
        let loader = resources.loader().lock();
        let data: Arc<[u8]> = Arc::from(data.to_vec().into_boxed_slice());
        Texture {
            data: data.clone(),
            dimensions,
            layers,
            set: loader.load_texture(
                resources.vulkan(),
                data,
                dimensions,
                layers,
                format,
                settings,
            ),
        }
    }

    /// Loads a texture to the GPU using the given image format.
    pub fn from_bytes(
        data: &[u8],
        image_format: ImageFormat,
        layers: u32,
        settings: TextureSettings,
        resources: &Resources,
    ) -> Result<Texture, InvalidFormatError> {
        // Turn image to a vector of u8 first.
        let image = match load_from_memory_with_format(data, image_format) {
            Err(_) => return Err(InvalidFormatError),
            Ok(v) => v,
        };

        let mut dimensions: (u32, u32);

        let mut format = Format::RGBA8;
        let image: Vec<u8> = match image {
            DynamicImage::ImageLuma8(image) => {
                format = Format::R8;
                dimensions = image.dimensions();
                image.into_vec()
            }
            DynamicImage::ImageLumaA8(_) => {
                let image = image.to_rgba8();
                dimensions = image.dimensions();
                image.into_vec()
            }
            DynamicImage::ImageLuma16(_) => {
                let image = image.to_luma8();
                dimensions = image.dimensions();
                format = Format::R8;
                image.into_vec()
            }
            DynamicImage::ImageLumaA16(_) => {
                let image = image.to_rgba16();
                dimensions = image.dimensions();
                format = Format::RGBA16;
                u16tou8vec(image.into_vec())
            }
            DynamicImage::ImageRgb8(_) => {
                let image = image.to_rgba8();
                dimensions = image.dimensions();
                image.into_vec()
            }
            DynamicImage::ImageRgba8(image) => {
                dimensions = image.dimensions();
                image.into_vec()
            }
            DynamicImage::ImageRgb16(_) => {
                let image = image.to_rgba16();
                dimensions = image.dimensions();
                format = Format::RGBA16;
                u16tou8vec(image.into_vec())
            }
            DynamicImage::ImageRgba16(image) => {
                format = Format::RGBA16;
                dimensions = image.dimensions();
                u16tou8vec(image.into_vec())
            }
            DynamicImage::ImageRgb32F(_) => {
                let image = image.to_rgba16();
                dimensions = image.dimensions();
                format = Format::RGBA16;
                u16tou8vec(image.into_vec())
            }
            DynamicImage::ImageRgba32F(_) => {
                let image = image.to_rgba16();
                dimensions = image.dimensions();
                format = Format::RGBA16;
                u16tou8vec(image.into_vec())
            }
            _ => {
                let image = image.to_rgba8();
                dimensions = image.dimensions();
                image.into_vec()
            }
        };

        dimensions.1 /= layers;

        Ok(Self::from_raw(
            &image, dimensions, format, layers, settings, resources,
        ))
    }
}
/// Accessing
impl Texture {
    pub fn data(&self) -> &Arc<[u8]> {
        &self.data
    }
    pub fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }
    pub fn layers(&self) -> u32 {
        self.layers
    }
    pub(crate) fn set(&self) -> &Arc<PersistentDescriptorSet> {
        &self.set
    }
}
/// A font to be used with the default label system.
#[derive(Clone)]
pub struct Font {
    font: Arc<rusttype::Font<'static>>,
    id: usize,
}

impl Font {
    /// Loads a font into the resources.
    ///
    /// Makes a new font using the bytes of a truetype or opentype font.
    /// Returns `None` in case the given bytes don't work.
    pub fn from_bytes(data: &'static [u8], resources: &Resources) -> Option<Self> {
        let labelifier = resources.labelifier().lock();
        let font = Arc::new(rusttype::Font::try_from_bytes(data)?);
        let id = labelifier.increment_id();
        Some(Self { font, id })
    }

    /// Loads a font into the resources.
    ///
    /// Makes a new font using the bytes in a vec of a truetype or opentype font.
    /// Returns `None` in case the given bytes don't work.
    pub fn from_vec(data: impl Into<Vec<u8>>, resources: &Resources) -> Option<Self> {
        let labelifier = resources.labelifier().lock();
        let font = Arc::new(rusttype::Font::try_from_vec(data.into())?);
        let id = labelifier.increment_id();
        Some(Self { font, id })
    }
    /// Returns the font ID.
    pub fn id(&self) -> usize {
        self.id
    }
    /// Returns the rusttype font.
    pub(crate) fn font(&self) -> &Arc<rusttype::Font<'static>> {
        &self.font
    }
}

pub struct Sound {
    pub data: Arc<[u8]>,
}

/// Not done.
#[allow(dead_code)]
pub fn load_sound(sound: &[u8]) -> Sound {
    Sound {
        data: Arc::from(sound.to_vec().into_boxed_slice()),
    }
}

impl PartialEq for Texture {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
            && self.dimensions == other.dimensions
            && Arc::ptr_eq(&self.set, &other.set)
    }
}

impl std::fmt::Debug for Texture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Texture")
            .field("size", &self.data.len())
            .field("dimensions", &self.dimensions)
            .field("frames", &self.layers)
            .finish()
    }
}
