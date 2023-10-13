//! Macros that make it easier to load resources.
//!
//! All require the [crate::start_engine] macro to be executed before usage.

/// Loads a model to the engine using the vertex and index data of the inserted
/// [Data](super::Data) and returns a [Model](super::Model).
#[macro_export]
macro_rules! model {
    ($data:expr) => {{
        RESOURCES.load_model($data)
    }};
}

/// Loads a font to the engine using binary true type font data of the inserted
/// `&[u8]` and returns a [Font](super::Font).
#[macro_export]
macro_rules! font {
    ($data:expr) => {{
        RESOURCES.load_font($data)
    }};
}

/// Loads a texture to the engine using one of
/// the supported file formats and returns a [Texture](super::Texture).
#[macro_export]
macro_rules! texture {
    (
        $data:expr,
        $image_format:expr,
        $settings:expr,
    ) => {{
        RESOURCES.load_texture($data, $image_format, 1, $settings)
    }};
    (
        $data:expr,
        $image_format:expr,
        $layers:expr,
        $settings:expr,
    ) => {{
        RESOURCES.load_texture($data, $image_format, $layers, $settings)
    }};
}

/// Loads a texture to the engine using raw image bytes and context and returns a [Texture](super::Texture).
#[macro_export]
macro_rules! texture_from_raw {
    (
        $data:expr,
        $dimensions:expr,
        $format:expr,
        $settings:expr,
    ) => {{
        RESOURCES.load_texture_from_raw($data, $dimensions, $format, 1, $settings)
    }};
    (
        $data:expr,
        $dimensions:expr,
        $format:expr,
        $layers:expr,
        $settings:expr,
    ) => {{
        RESOURCES.load_texture_from_raw($data, $dimensions, $format, $layers, $settings)
    }};
}

/// Loads a shader from glsl bytes. Takes `&[u8]` and returns [Shaders](crate::materials::Shaders).
/// Those shaders can be used when making materials.
///
/// # Safety
///
/// Doesn't validate the rightness of the given data.
/// Crashes the program in case the bytes provided aren't spirv.
#[macro_export]
macro_rules! raw_shader {
    (
        $vertex_data:expr,
        $fragment_data:expr,
    ) => {{
        RESOURCES.new_shader_from_raw($vertex_data, $fragment_data)
    }};
}

/// Loads a new material.
#[macro_export]
macro_rules! material {
    (
        $settings:expr,
    ) => {{
        RESOURCES.new_material($settings)
    }};
    (
        $settings:expr,
        $shaders:expr,
        $descriptor_bindings:expr,
    ) => {{
        RESOURCES.new_material_with_shaders($settings, $shaders, $descriptor_bindings)
    }};
}

/// Describes a write operation for a descriptor.
/// Used with materials to interact with custom shaders inside them.
#[macro_export]
macro_rules! write_descriptor {
    (
        $buf:expr,
        $set:expr,
    ) => {{
        RESOURCES.new_descriptor_write($buf, $set)
    }};
}
