//! Default shaders.

use std::sync::Arc;

use anyhow::{Context, Result};
use vulkano::{
    device::Device,
    shader::{spirv::bytes_to_words, ShaderModule, ShaderModuleCreateInfo},
};

fn from_bytes(bytes: &[u8], device: Arc<Device>) -> Result<Arc<ShaderModule>> {
    let code = bytes_to_words(bytes)?;
    Ok(unsafe { ShaderModule::new(device, ShaderModuleCreateInfo::new(&code))? })
}

pub fn vertex_shader(device: Arc<Device>) -> Result<Arc<ShaderModule>> {
    from_bytes(include_bytes!("shaders/default_vert.spv"), device)
        .context("There was a problem making the default vertex shader.")
}

pub fn fragment_shader(device: Arc<Device>) -> Result<Arc<ShaderModule>> {
    from_bytes(include_bytes!("shaders/default_frag.spv"), device)
        .context("There was a problem making the default fragment shader.")
}

pub fn instanced_vertex_shader(device: Arc<Device>) -> Result<Arc<ShaderModule>> {
    from_bytes(include_bytes!("shaders/instance_vert.spv"), device)
        .context("There was a problem making the default instanced vertex shader.")
}

pub fn instanced_fragment_shader(device: Arc<Device>) -> Result<Arc<ShaderModule>> {
    from_bytes(include_bytes!("shaders/instance_frag.spv"), device)
        .context("There was a problem making the default instanced fragment shader.")
}

pub fn textured_fragment_shader(device: Arc<Device>) -> Result<Arc<ShaderModule>> {
    from_bytes(include_bytes!("shaders/textured_frag.spv"), device)
        .context("There was a problem making the default textured fragment shader.")
}

pub fn texture_array_fragment_shader(device: Arc<Device>) -> Result<Arc<ShaderModule>> {
    from_bytes(include_bytes!("shaders/texture_array_frag.spv"), device)
        .context("There was a problem making the default texture array fragment shader.")
}

pub fn instanced_textured_fragment_shader(device: Arc<Device>) -> Result<Arc<ShaderModule>> {
    from_bytes(include_bytes!("shaders/textured_instance_frag.spv"), device)
        .context("There was a problem making the default instanced texture fragment shader.")
}

pub fn instanced_texture_array_fragment_shader(device: Arc<Device>) -> Result<Arc<ShaderModule>> {
    from_bytes(
        include_bytes!("shaders/texture_array_instance_frag.spv"),
        device,
    )
    .context("There was a problem making the default instanced texture array fragment shader.")
}
