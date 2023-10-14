//! Macros that make it easier to load resources.
//!
//! All require the [crate::start_engine] macro to be executed before usage.
//!
//! All the macros are just a shortening of `any Resource`::new(data, &[Resources](crate::prelude::Resources)).

/// Loads a model to the engine using the vertex and index data of the inserted
/// [Data](crate::prelude::Data) and returns a [Model](crate::prelude::Model).
#[macro_export]
macro_rules! model {
    ($data:expr) => {{
        let_engine::prelude::Model::new($data, &RESOURCES)
    }};
}

/// Loads a font to the engine using binary true type font data of the inserted
/// `&[u8]` and returns an [`Option<Font>`](crate::prelude::Font).
///
/// Returns `None` in case the provided bytes don't work.
#[macro_export]
macro_rules! font {
    ($data:expr) => {{
        let_engine::prelude::Font::from_bytes($data, &RESOURCES);
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
        let_engine::prelude::Texture::from_bytes($data, $image_format, 1, $settings, &RESOURCES)
    }};
    (
        $data:expr,
        $image_format:expr,
        $layers:expr,
        $settings:expr,
    ) => {{
        let_engine::prelude::Texture::from_bytes(
            $data,
            $image_format,
            $layers,
            $settings,
            &RESOURCES,
        )
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
        let_engine::prelude::Texture::from_raw(
            $data,
            $dimensions,
            $format,
            1,
            $settings,
            &RESOURCES,
        )
    }};
    (
        $data:expr,
        $dimensions:expr,
        $format:expr,
        $layers:expr,
        $settings:expr,
    ) => {{
        let_engine::prelude::Texture::from_raw(
            $data,
            $dimensions,
            $format,
            $layers,
            $settings,
            &RESOURCES,
        )
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
        let_engine::prelude::Shaders::from_bytes($vertex_data, $fragment_data, &RESOURCES);
    }};
}

/// Loads a new material.
#[macro_export]
macro_rules! material {
    (
        $settings:expr,
    ) => {{
        let_engine::prelude::Material::new($settings, &RESOURCES);
    }};
    (
        $settings:expr,
        $shaders:expr,
        $descriptor_bindings:expr,
    ) => {{
        let_engine::prelude::Material.new_with_shaders(
            $settings,
            $shaders,
            $descriptor_bindings,
            &RESOURCES,
        )
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
