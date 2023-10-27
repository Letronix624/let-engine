//! Default shaders.

use std::sync::Arc;

use vulkano::{shader::{ShaderModule, spirv::bytes_to_words, ShaderModuleCreateInfo}, device::Device};

fn from_bytes(bytes: &[u8], device: Arc<Device>) -> Arc<ShaderModule> {
    let code = bytes_to_words(bytes).unwrap();
    unsafe {
        ShaderModule::new(device, ShaderModuleCreateInfo::new(&code)).unwrap()
    }
}

pub fn vertexshader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(include_bytes!("shaders/default_vert.spv"), device)
}

pub fn fragmentshader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(include_bytes!("shaders/default_frag.spv"), device)
}

pub fn textured_fragmentshader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(include_bytes!("shaders/default_textured.spv"), device)
}

pub fn texture_array_fragmentshader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(include_bytes!("shaders/default_texture_array.spv"), device)
}

pub fn text_fragmentshader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(include_bytes!("shaders/text.spv"), device)
}
