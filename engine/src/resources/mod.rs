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

#[cfg(feature = "audio")]
pub mod sounds;
pub use model::*;

pub(crate) static RESOURCES: Lazy<Resources> = Lazy::new(|| {
    let vulkan = Vulkan::init().map_err(|e| panic!("{e}")).unwrap();
    Resources::new(vulkan)
});
#[cfg(feature = "labels")]
pub(crate) static LABELIFIER: Lazy<Mutex<Labelifier>> = Lazy::new(|| Mutex::new(Labelifier::new()));

/// All the resources kept in the game engine like textures, fonts, sounds and models.
#[derive(Clone)]
pub(crate) struct Resources {
    pub vulkan: Vulkan,
    pub loader: Arc<Mutex<Loader>>,
    pub shapes: BasicShapes,
    #[cfg(feature = "audio")]
    pub audio_server: crossbeam::channel::Sender<AudioUpdate>,
}

impl Resources {
    pub(crate) fn new(vulkan: Vulkan) -> Self {
        let loader = Arc::new(Mutex::new(Loader::init(&vulkan).unwrap()));
        let shapes = BasicShapes::new(&loader);
        #[cfg(feature = "audio")]
        let audio_server = sounds::audio_server();
        Self {
            vulkan,
            loader,
            shapes,
            #[cfg(feature = "audio")]
            audio_server,
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

/// Merges a pipeline cache into the resources potentially making the creation of materials faster.
///
/// # Safety
///
/// Unsafe because vulkan blindly trusts that this data comes from the `get_pipeline_binary` function.
/// The program will panic if the data provided is not right.
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
