//! Default shaders.

use std::sync::Arc;

use vulkano::{
    device::Device,
    shader::{spirv::bytes_to_words, ShaderModule, ShaderModuleCreateInfo},
};

fn from_bytes(bytes: &[u8], device: Arc<Device>) -> Arc<ShaderModule> {
    let code = bytes_to_words(bytes).unwrap();
    unsafe { ShaderModule::new(device, ShaderModuleCreateInfo::new(&code)).unwrap() }
}

pub fn vertexshader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(include_bytes!("shaders/default_vert.spv"), device)
}

pub fn fragmentshader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(include_bytes!("shaders/default_frag.spv"), device)
}

pub fn textured_fragmentshader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(include_bytes!("shaders/default_textured_frag.spv"), device)
}

pub fn texture_array_fragmentshader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(
        include_bytes!("shaders/default_texture_array_frag.spv"),
        device,
    )
}

pub fn text_fragmentshader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(include_bytes!("shaders/text_frag.spv"), device)
}
