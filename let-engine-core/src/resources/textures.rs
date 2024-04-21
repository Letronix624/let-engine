//! Texture related options.

pub use image::ImageFormat;
use image::{load_from_memory_with_format, DynamicImage};

use derive_builder::Builder;
use std::sync::Arc;
use vulkano::descriptor_set::DescriptorSet;
pub use vulkano::image::sampler::BorderColor;
use vulkano::image::sampler::{
    Filter as vkFilter, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode,
};

use super::resources;
use crate::utils::u16tou8vec;

/// Formats for the texture from raw data.
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum Format {
    /// 8 bits red
    R8 = 1,
    /// 8 bits red green blue alpha
    RGBA8 = 4,
    /// 16 bits red green blue alpha
    RGBA16 = 8,
}

/// Filtering mode
#[derive(Clone, Copy, Debug)]
pub enum Filter {
    Nearest,
    Linear,
}

/// Handling of pixels outside the position range of the texture.
#[derive(Clone, Copy, Debug)]
pub enum AddressMode {
    /// Repeats the texture.
    Repeat,
    /// Repeats the texture mirrored.
    Mirrored,
    /// Coordinates outside 0 - 1 are clamped to 0 - 1.
    ClampToEdge,
    /// Coordinates outside 0 - 1 are colored to the border color settable in the Sampler struct.
    ClampToBorder,
}

/// The sampler of the texture that determines how the shader should handle textures.
#[derive(Debug, Builder, Clone)]
#[builder(setter(into))]
pub struct Sampler {
    /// Way to filter the texture when the texture is bigger than it's actual resolution.
    #[builder(setter(into), default = "Filter::Nearest")]
    pub mag_filter: Filter,
    /// Way to filter the texture when it's smaller than the actual texture.
    #[builder(setter(into), default = "Filter::Linear")]
    pub min_filter: Filter,
    /// How the final sampled value should be calculated from the samples of individual mipmaps.
    #[builder(setter(into), default = "Filter::Nearest")]
    pub mipmap_mode: Filter,
    /// How out of range texture coordinates should be handled.
    #[builder(setter(into), default = "[AddressMode::ClampToBorder; 3]")]
    pub address_mode: [AddressMode; 3],
    /// Color for the border when the address mode is on ClampToBorder.
    #[builder(setter(into), default = "BorderColor::FloatTransparentBlack")]
    pub border_color: BorderColor,
}

/// The main texture settings.
#[derive(Clone, Debug)]
pub struct TextureSettings {
    /// SRGB mode.
    pub srgb: bool,
    /// Image sampler
    pub sampler: Sampler,
}

impl Default for Sampler {
    fn default() -> Self {
        Self {
            mag_filter: Filter::Nearest,
            min_filter: Filter::Linear,
            mipmap_mode: Filter::Nearest,
            address_mode: [AddressMode::ClampToBorder; 3],
            border_color: BorderColor::FloatTransparentBlack,
        }
    }
}

impl Sampler {
    pub fn to_vulkano(&self) -> SamplerCreateInfo {
        SamplerCreateInfo {
            mag_filter: match self.mag_filter {
                Filter::Nearest => vkFilter::Nearest,
                Filter::Linear => vkFilter::Linear,
            },
            min_filter: match self.mag_filter {
                Filter::Nearest => vkFilter::Nearest,
                Filter::Linear => vkFilter::Linear,
            },
            mipmap_mode: match self.mag_filter {
                Filter::Nearest => SamplerMipmapMode::Nearest,
                Filter::Linear => SamplerMipmapMode::Linear,
            },
            // improvable.
            address_mode: [
                match self.address_mode[0] {
                    AddressMode::Repeat => SamplerAddressMode::Repeat,
                    AddressMode::Mirrored => SamplerAddressMode::MirroredRepeat,
                    AddressMode::ClampToEdge => SamplerAddressMode::ClampToEdge,
                    AddressMode::ClampToBorder => SamplerAddressMode::ClampToBorder,
                },
                match self.address_mode[1] {
                    AddressMode::Repeat => SamplerAddressMode::Repeat,
                    AddressMode::Mirrored => SamplerAddressMode::MirroredRepeat,
                    AddressMode::ClampToEdge => SamplerAddressMode::ClampToEdge,
                    AddressMode::ClampToBorder => SamplerAddressMode::ClampToBorder,
                },
                match self.address_mode[2] {
                    AddressMode::Repeat => SamplerAddressMode::Repeat,
                    AddressMode::Mirrored => SamplerAddressMode::MirroredRepeat,
                    AddressMode::ClampToEdge => SamplerAddressMode::ClampToEdge,
                    AddressMode::ClampToBorder => SamplerAddressMode::ClampToBorder,
                },
            ],
            border_color: self.border_color,
            ..Default::default()
        }
    }
}

impl Default for TextureSettings {
    fn default() -> Self {
        Self {
            srgb: true,
            sampler: Sampler::default(),
        }
    }
}

impl TextureSettings {
    pub fn srgb(mut self, srgb: bool) -> Self {
        self.srgb = srgb;
        self
    }
    pub fn sampler(mut self, sampler: Sampler) -> Self {
        self.sampler = sampler;
        self
    }
}

/// A texture to be used with materials.
#[derive(Clone)]
pub struct Texture {
    data: Arc<[u8]>,
    dimensions: (u32, u32),
    layers: u32,
    set: Arc<DescriptorSet>,
}

/// Making
impl Texture {
    /// Loads a texture to the GPU using a raw image.
    pub fn from_raw(
        data: &[u8],
        dimensions: (u32, u32),
        format: Format,
        layers: u32,
        settings: TextureSettings,
    ) -> Result<Texture, TextureError> {
        let data: Arc<[u8]> = Arc::from(data.to_vec().into_boxed_slice());
        Ok(Texture {
            data: data.clone(),
            dimensions,
            layers,
            set: resources()
                .map_err(|e| TextureError::Other(e.into()))?
                .loader()
                .lock()
                .load_texture(
                    resources()
                        .map_err(|e| TextureError::Other(e.into()))?
                        .vulkan(),
                    data,
                    dimensions,
                    layers,
                    format,
                    settings,
                )
                .map_err(TextureError::Other)?,
        })
    }

    /// Loads a texture to the GPU using the given image format.
    ///
    /// Returns an error in case the given format does not work with the given bytes.
    pub fn from_bytes(
        data: &[u8],
        image_format: ImageFormat,
        layers: u32,
        settings: TextureSettings,
    ) -> Result<Texture, TextureError> {
        // Turn image to a vector of u8 first.
        let image = match load_from_memory_with_format(data, image_format) {
            Err(_) => {
                return Err(TextureError::InvalidFormat(format!(
                    "Faulty format: {:?}",
                    image_format
                )))
            }
            Ok(v) => v,
        };

        let mut dimensions: (u32, u32);

        let mut format = Format::RGBA8;
        let image: Vec<u8> = match image {
            DynamicImage::ImageLuma8(image) => {
                format = Format::R8;
                dimensions = image.dimensions();
                image.into_vec()
            }
            DynamicImage::ImageLumaA8(_) => {
                let image = image.to_rgba8();
                dimensions = image.dimensions();
                image.into_vec()
            }
            DynamicImage::ImageLuma16(_) => {
                let image = image.to_luma8();
                dimensions = image.dimensions();
                format = Format::R8;
                image.into_vec()
            }
            DynamicImage::ImageLumaA16(_) => {
                let image = image.to_rgba16();
                dimensions = image.dimensions();
                format = Format::RGBA16;
                u16tou8vec(image.into_vec())
            }
            DynamicImage::ImageRgb8(_) => {
                let image = image.to_rgba8();
                dimensions = image.dimensions();
                image.into_vec()
            }
            DynamicImage::ImageRgba8(image) => {
                dimensions = image.dimensions();
                image.into_vec()
            }
            DynamicImage::ImageRgb16(_) => {
                let image = image.to_rgba16();
                dimensions = image.dimensions();
                format = Format::RGBA16;
                u16tou8vec(image.into_vec())
            }
            DynamicImage::ImageRgba16(image) => {
                format = Format::RGBA16;
                dimensions = image.dimensions();
                u16tou8vec(image.into_vec())
            }
            DynamicImage::ImageRgb32F(_) => {
                let image = image.to_rgba16();
                dimensions = image.dimensions();
                format = Format::RGBA16;
                u16tou8vec(image.into_vec())
            }
            DynamicImage::ImageRgba32F(_) => {
                let image = image.to_rgba16();
                dimensions = image.dimensions();
                format = Format::RGBA16;
                u16tou8vec(image.into_vec())
            }
            _ => {
                let image = image.to_rgba8();
                dimensions = image.dimensions();
                image.into_vec()
            }
        };

        dimensions.1 /= layers;

        Self::from_raw(&image, dimensions, format, layers, settings)
    }
}
/// Accessing
impl Texture {
    pub fn data(&self) -> &Arc<[u8]> {
        &self.data
    }
    pub fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }
    pub fn layers(&self) -> u32 {
        self.layers
    }
    pub(crate) fn set(&self) -> &Arc<DescriptorSet> {
        &self.set
    }
}

impl PartialEq for Texture {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
            && self.dimensions == other.dimensions
            && Arc::ptr_eq(&self.set, &other.set)
    }
}

impl std::fmt::Debug for Texture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Texture")
            .field("size", &self.data.len())
            .field("dimensions", &self.dimensions)
            .field("frames", &self.layers)
            .finish()
    }
}

// Texture based errors.

use thiserror::Error;

/// Errors that occur from textures.
#[derive(Error, Debug)]
pub enum TextureError {
    /// This error gets returned when you set the texture ID of an Appearance object higher than the
    /// actual frame count of the texture this object is holding.
    #[error("The layer you set for this object does not exist:\n{0}")]
    Layer(String),
    /// This error gets returned when a function gets called that requires an object to have a textured material
    /// but it does not have one.
    #[error(
        "The object you ran a function on that requires a textured material does not have one."
    )]
    NoTexture,
    /// This error gets returned when you give the wrong format to the texture when trying to create a
    /// new texture.
    #[error("The given format does not match with the bytes provided:\n{0}")]
    InvalidFormat(String),
    /// If the texture for some reason can not be made.
    #[error("There was an error loading this texture:\n{0}")]
    Other(anyhow::Error),
}
