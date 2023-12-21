//! Resources to be handled by the engine like textures, sounds and fonts.

use crate::prelude::*;

use core::panic;
use once_cell::sync::Lazy;
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

pub(crate) static RESOURCES: Lazy<Resources> = Lazy::new(|| {
    let vulkan = Vulkan::init().map_err(|e| panic!("{e}")).unwrap();
    Resources::new(vulkan)
});
pub(crate) static LABELIFIER: Lazy<Mutex<Labelifier>> = Lazy::new(|| Mutex::new(Labelifier::new()));

/// All the resources kept in the game engine like textures, fonts, sounds and models.
#[derive(Clone)]
pub(crate) struct Resources {
    pub(crate) vulkan: Vulkan,
    pub(crate) loader: Arc<Mutex<Loader>>,

    pub(crate) shapes: BasicShapes,
}

impl Resources {
    pub(crate) fn new(vulkan: Vulkan) -> Self {
        let loader = Arc::new(Mutex::new(Loader::init(&vulkan).unwrap()));
        let shapes = BasicShapes::new(&loader);
        Self {
            vulkan,
            loader,
            shapes,
        }
    }

    pub(crate) fn vulkan(&self) -> &Vulkan {
        &self.vulkan
    }
    pub(crate) fn loader(&self) -> &Arc<Mutex<Loader>> {
        &self.loader
    }
    pub(crate) fn shapes(&self) -> &BasicShapes {
        &self.shapes
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
    pub fn from_bytes(data: &'static [u8]) -> Option<Self> {
        let labelifier = &LABELIFIER;
        let font = Arc::new(rusttype::Font::try_from_bytes(data)?);
        let id = labelifier.lock().increment_id();
        Some(Self { font, id })
    }

    /// Loads a font into the resources.
    ///
    /// Makes a new font using the bytes in a vec of a truetype or opentype font.
    /// Returns `None` in case the given bytes don't work.
    pub fn from_vec(data: impl Into<Vec<u8>>) -> Option<Self> {
        let labelifier = &LABELIFIER;
        let font = Arc::new(rusttype::Font::try_from_vec(data.into())?);
        let id = labelifier.lock().increment_id();
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

// pub struct Sound {
//     pub data: Arc<[u8]>,
// }

// /// Not done.
// #[allow(dead_code)]
// pub fn load_sound(sound: &[u8]) -> Sound {
//     Sound {
//         data: Arc::from(sound.to_vec().into_boxed_slice()),
//     }
// }

/// Merges a pipeline cache into the resources potentially making the creation of materials faster.
///
/// # Safety
///
/// Unsafe because vulkan blindly trusts that this data comes from the `get_pipeline_binary` function.
/// The program will crash if the data provided is not right.
///
/// The binary given to the function must be made with the same hardware and vulkan driver version.
pub unsafe fn load_pipeline_cache(data: &[u8]) {
    let cache = PipelineCache::new(
        RESOURCES.vulkan().device.clone(),
        PipelineCacheCreateInfo {
            initial_data: data.to_vec(),
            ..Default::default()
        },
    )
    .unwrap();
    RESOURCES
        .loader()
        .lock()
        .pipeline_cache
        .merge([cache.as_ref()])
        .unwrap();
}

/// Returns the binary of the pipeline cache.
///
/// Allows this binary to be loaded with the `load_pipeline_cache` function to make loading materials potentially faster.
pub fn get_pipeline_binary() -> Vec<u8> {
    RESOURCES.loader().lock().pipeline_cache.get_data().unwrap()
}

/// Loads a new write operation for a shader.
pub fn new_descriptor_write<T: BufferContents>(buf: T, set: u32) -> WriteDescriptorSet {
    let loader = RESOURCES.loader().lock();
    loader.write_descriptor(buf, set)
}
