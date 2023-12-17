//! Resources to be handled by the engine like textures, sounds and fonts.

use crate::prelude::*;

use parking_lot::Mutex;
use std::sync::Arc;
use vulkano::buffer::BufferContents;
use vulkano::descriptor_set::WriteDescriptorSet;
use vulkano::pipeline::cache::{PipelineCache, PipelineCacheCreateInfo};

mod loader;
pub(crate) mod vulkan;
pub(crate) use loader::Loader;
use vulkan::Vulkan;

pub mod textures;

pub mod data;
pub mod materials;
mod model;
pub use model::*;

/// Trait that allows this to be used to load data.
pub trait Resource {
    fn resources(&self) -> &Resources;
}

/// All the resources kept in the game engine like textures, fonts, sounds and models.
#[derive(Clone)]
pub struct Resources {
    pub(crate) vulkan: Vulkan,
    pub(crate) loader: Arc<Mutex<Loader>>,

    pub(crate) shapes: BasicShapes,
    pub(crate) labelifier: Arc<Mutex<Labelifier>>,
}

impl Resource for Resources {
    fn resources(&self) -> &Resources {
        self
    }
}

impl Resources {
    pub(crate) fn new(vulkan: Vulkan) -> Self {
        let loader = Arc::new(Mutex::new(Loader::init(&vulkan).unwrap()));
        let shapes = BasicShapes::new(&loader);
        let labelifier = Arc::new(Mutex::new(Labelifier::new(&vulkan, &loader)));
        Self {
            vulkan,
            loader,
            shapes,
            labelifier,
        }
    }

    pub(crate) fn vulkan(&self) -> &Vulkan {
        &self.vulkan
    }
    pub(crate) fn loader(&self) -> &Arc<Mutex<Loader>> {
        &self.loader
    }
    pub(crate) fn labelifier(&self) -> &Arc<Mutex<Labelifier>> {
        &self.labelifier
    }
    pub(crate) fn shapes(&self) -> &BasicShapes {
        &self.shapes
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
        let cache = PipelineCache::new(
            self.vulkan().device.clone(),
            PipelineCacheCreateInfo {
                initial_data: data.to_vec(),
                ..Default::default()
            },
        )
        .unwrap();
        self.loader()
            .lock()
            .pipeline_cache
            .merge([cache.as_ref()])
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
    pub fn from_bytes(data: &'static [u8], resources: &impl Resource) -> Option<Self> {
        let labelifier = resources.resources().labelifier().lock();
        let font = Arc::new(rusttype::Font::try_from_bytes(data)?);
        let id = labelifier.increment_id();
        Some(Self { font, id })
    }

    /// Loads a font into the resources.
    ///
    /// Makes a new font using the bytes in a vec of a truetype or opentype font.
    /// Returns `None` in case the given bytes don't work.
    pub fn from_vec(data: impl Into<Vec<u8>>, resources: &impl Resource) -> Option<Self> {
        let labelifier = resources.resources().labelifier().lock();
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
