//! Resources to be handled by the engine like textures, sounds and fonts.

use super::Labelifier;
use crate::window::Window;
use parking_lot::Mutex;
use std::sync::Arc;
use vulkano::buffer::BufferContents;
use vulkano::descriptor_set::WriteDescriptorSet;
use vulkano::pipeline::cache::PipelineCache;

mod loader;
pub(crate) mod vulkan;
pub(crate) use loader::Loader;
use vulkan::Vulkan;

pub mod textures;

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
