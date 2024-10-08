//! Resources to be handled by the engine like textures, sounds and fonts.
//!
//! Panics the program in case the system is not capable of running the game engine.

use anyhow::{Context, Result};
use parking_lot::Mutex;
use std::sync::{Arc, OnceLock};
use vulkano::buffer::BufferContents;
use vulkano::descriptor_set::WriteDescriptorSet;
use vulkano::pipeline::cache::{PipelineCache, PipelineCacheCreateInfo};
use winit::event_loop::EventLoop;

mod loader;
pub(crate) mod vulkan;
pub(crate) use loader::Loader;
use vulkan::Vulkan;

pub mod textures;

pub mod data;
pub mod materials;
mod model;

pub use model::*;

use crate::EngineError;

use self::data::BasicShapes;

pub static RESOURCES: OnceLock<Resources> = OnceLock::new();
pub fn resources<'a>() -> Result<&'a Resources, EngineError> {
    RESOURCES.get().ok_or(EngineError::NotReady)
}

/// All the resources kept in the game engine like textures, fonts, sounds and models.
#[derive(Clone)]
pub struct Resources {
    pub vulkan: Vulkan,
    pub loader: Arc<Mutex<Loader>>,
    pub shapes: BasicShapes,
}

impl Resources {
    pub fn new(event_loop: &EventLoop<()>) -> Result<Self, EngineError> {
        let (materials, vulkan) =
            Vulkan::init(event_loop).map_err(|e| EngineError::RequirementError(e.to_string()))?;

        let loader = Arc::new(Mutex::new(
            Loader::init(&vulkan, materials)
                .context("Failed to create the graphics loading environment for the game engine.")
                .map_err(EngineError::Other)?,
        ));
        let shapes = BasicShapes::new(&loader)
            .context("Failed to load default shapes into the GPU memory.")
            .map_err(EngineError::Other)?;
        Ok(Self {
            vulkan,
            loader,
            shapes,
        })
    }

    pub fn vulkan(&self) -> &Vulkan {
        &self.vulkan
    }
    pub fn loader(&self) -> &Arc<Mutex<Loader>> {
        &self.loader
    }
    pub fn shapes(&self) -> &BasicShapes {
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
pub unsafe fn load_pipeline_cache(data: &[u8]) -> Result<()> {
    let cache = PipelineCache::new(
        resources()?.vulkan().device.clone(),
        PipelineCacheCreateInfo {
            initial_data: data.to_vec(),
            ..Default::default()
        },
    )?;
    resources()?
        .loader()
        .lock()
        .pipeline_cache
        .merge([cache.as_ref()])?;
    Ok(())
}

/// Returns the binary of the pipeline cache.
///
/// Allows this binary to be loaded with the `load_pipeline_cache` function to make loading materials potentially faster.
pub fn pipeline_binary() -> Result<Vec<u8>> {
    Ok(resources()?.loader().lock().pipeline_cache.get_data()?)
}

/// Loads a new write operation for a shader.
pub fn new_descriptor_write<T: BufferContents>(buf: T, set: u32) -> Result<WriteDescriptorSet> {
    let loader = resources()?.loader().lock();
    loader.write_descriptor(buf, set)
}
