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

pub fn vertex_shader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(include_bytes!("shaders/default_vert.spv"), device)
}

pub fn fragment_shader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(include_bytes!("shaders/default_frag.spv"), device)
}

pub fn instanced_vertex_shader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(include_bytes!("shaders/instance_vert.spv"), device)
}

pub fn instanced_fragment_shader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(include_bytes!("shaders/instance_frag.spv"), device)
}

pub fn textured_fragment_shader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(include_bytes!("shaders/textured_frag.spv"), device)
}

pub fn texture_array_fragment_shader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(include_bytes!("shaders/texture_array_frag.spv"), device)
}

pub fn instanced_textured_fragment_shader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(include_bytes!("shaders/textured_instance_frag.spv"), device)
}

pub fn instanced_texture_array_fragment_shader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(
        include_bytes!("shaders/texture_array_instance_frag.spv"),
        device,
    )
}

pub fn text_fragment_shader(device: Arc<Device>) -> Arc<ShaderModule> {
    from_bytes(include_bytes!("shaders/text_frag.spv"), device)
}
