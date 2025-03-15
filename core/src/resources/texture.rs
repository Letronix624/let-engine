use anyhow::Result;
use derive_builder::Builder;
use image::{DynamicImage, GenericImageView, ImageReader};

use crate::objects::Color;

use super::{buffer::BufferAccess, Format};
pub use image::ImageFormat;

/// An unloaded texture instance.
///
/// This does not contain a reference and is slow to clone, if textures are big.
#[derive(Debug, Clone, PartialEq)]
pub struct Texture {
    data: Vec<u8>,
    dimensions: ViewTypeDim,
    settings: TextureSettings,
}

pub trait LoadedTexture: Clone + Send + Sync {
    type Error: std::error::Error + Send + Sync;

    /// Return the data of the image.
    fn data(&self) -> Result<Vec<u8>, Self::Error>;

    fn dimensions(&self) -> ViewTypeDim;

    fn resize(&self, new_dimensions: ViewTypeDim) -> Result<(), Self::Error>;

    fn write_data<F: FnMut(&mut [u8])>(&self, f: F) -> Result<(), Self::Error>;
}

impl LoadedTexture for () {
    type Error = std::io::Error;

    fn data(&self) -> Result<Vec<u8>, Self::Error> {
        Ok(vec![])
    }

    fn dimensions(&self) -> ViewTypeDim {
        ViewTypeDim::D1 { x: 0 }
    }

    fn resize(&self, _new_dimensions: ViewTypeDim) -> Result<(), Self::Error> {
        Ok(())
    }

    fn write_data<F: FnMut(&mut [u8])>(&self, _f: F) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Texture {
    pub fn calculate_buffer_size(dimensions: &ViewTypeDim, settings: &TextureSettings) -> usize {
        let mut total_size = 0;

        let format = &settings.format;
        let mip_levels = settings.mip_levels;
        let extent = dimensions.extent();

        let block_extent = format.block_extent(); // [width, height, depth]
        let block_size = format.block_size(); // Bytes per compressed block

        for mip in 0..mip_levels {
            let mip_width = (extent[0] >> mip).max(1);
            let mip_height = (extent[1] >> mip).max(1);
            let mip_depth = (extent[2] >> mip).max(1);

            let num_blocks_x = mip_width.div_ceil(block_extent[0]);
            let num_blocks_y = mip_height.div_ceil(block_extent[1]);

            let mip_size = num_blocks_x * num_blocks_y * mip_depth * block_size as u32;
            total_size += mip_size * dimensions.array_layers();
        }

        total_size as usize
    }
}

impl Texture {
    /// Creates a CPU-side texture definition that can be loaded by the graphics backend.
    ///
    /// # Parameters
    /// - `data`: A vector of raw bytes containing the texture data.
    /// - `dimensions`: A type specifying the texture dimensions and view type.
    /// - `settings`: The texture settings, including format, sampler, and access preferences.
    ///
    /// # Errors
    /// - [`TextureError::InvalidSize`] if the size of `data` does not match the expected size.
    /// - [`TextureError::ZeroedDimension`] if any dimension provided in `dimensions` equals zero.
    /// - [`TextureError::InvalidMipLevel`] if the mip level is out of accepted range.
    pub fn from_raw(
        data: Vec<u8>,
        dimensions: ViewTypeDim,
        settings: TextureSettings,
    ) -> Result<Texture, TextureError> {
        let expected_size = Self::calculate_buffer_size(&dimensions, &settings);

        // Error at zeroed dimension.
        if expected_size == 0 {
            return Err(TextureError::ZeroedDimension);
        }

        // Error at dimensions and data size mismatch
        if data.len() != expected_size {
            return Err(TextureError::InvalidSize(expected_size, data.len()));
        }

        // Error at out of range mip level
        let max_mip_level = dimensions.max_mip_level();
        if settings.mip_levels == 0 {
            return Err(TextureError::InvalidMipLevel(0, max_mip_level));
        } else if settings.mip_levels > max_mip_level {
            return Err(TextureError::InvalidMipLevel(
                settings.mip_levels,
                max_mip_level,
            ));
        };

        Ok(Texture {
            data,
            dimensions,
            settings,
        })
    }

    /// Creates a new texture repetitively filled with the same data.
    ///
    /// Returns an error in case the length of the provided data does not match up with the format block size in the settings.
    pub fn new_filled(
        data: Vec<u8>,
        dimensions: ViewTypeDim,
        settings: TextureSettings,
    ) -> Result<Self, TextureError> {
        let block_size = settings.format.block_size();
        if data.len() != block_size {
            return Err(TextureError::InvalidSize(block_size, data.len()));
        };

        let length = Self::calculate_buffer_size(&dimensions, &settings) / block_size;

        let data: Vec<Vec<u8>> = vec![data; length];

        Self::from_raw(data.into_iter().flatten().collect(), dimensions, settings)
    }

    /// Creates a new texture consisting of only one color with the provided dimensions and format.
    pub fn new_colored(
        color: &Color,
        dimensions: ViewTypeDim,
        settings: TextureSettings,
    ) -> Result<Self, TextureError> {
        let format = &settings.format;

        let length = dimensions.elements_num() as usize;

        let color_buffer = format.color_to_buffer(color);

        let data: Vec<Vec<u8>> = vec![color_buffer; length];

        Self::from_raw(data.into_iter().flatten().collect(), dimensions, settings)
    }

    /// Creates a new texture made completely out of zeroes and depending on the format either black or transparent.
    pub fn new_empty(
        dimensions: ViewTypeDim,
        settings: TextureSettings,
    ) -> Result<Self, TextureError> {
        Self::from_raw(
            vec![0u8; Self::calculate_buffer_size(&dimensions, &settings)],
            dimensions,
            settings,
        )
    }

    /// Creates a texture from the bytes of a decoded image of the given image format.
    ///
    /// The view type equals [`ViewType::D2`].
    pub fn from_bytes(
        data: Vec<u8>,
        image_format: ImageFormat,
        mut settings: TextureSettings,
    ) -> Result<Self, TextureError> {
        let cursor = std::io::Cursor::new(data);

        let reader = ImageReader::with_format(cursor, image_format);

        let data = match reader.decode() {
            Ok(data) => data,
            Err(_) => {
                println!("Hello");
                return Err(TextureError::InvalidFormat);
            }
        };

        let dimensions = data.dimensions();

        let data = match data {
            DynamicImage::ImageLuma8(data) => {
                settings.format = Format::Sr8;
                data.to_vec()
            }
            DynamicImage::ImageLumaA8(data) => {
                settings.format = Format::Srg8;
                data.to_vec()
            }
            DynamicImage::ImageRgb8(data) => {
                settings.format = Format::Srgb8;
                data.to_vec()
            }
            DynamicImage::ImageRgba8(data) => {
                settings.format = Format::Srgba8;
                data.to_vec()
            }
            DynamicImage::ImageLuma16(data) => {
                settings.format = Format::R16Unorm;
                data.iter().flat_map(|x| x.to_le_bytes()).collect()
            }
            DynamicImage::ImageLumaA16(data) => {
                settings.format = Format::Rg16Unorm;
                data.iter().flat_map(|x| x.to_le_bytes()).collect()
            }
            DynamicImage::ImageRgb16(data) => {
                settings.format = Format::Rgb16Unorm;
                data.iter().flat_map(|x| x.to_le_bytes()).collect()
            }
            DynamicImage::ImageRgba16(data) => {
                settings.format = Format::Rgba16Unorm;
                data.iter().flat_map(|x| x.to_le_bytes()).collect()
            }
            DynamicImage::ImageRgb32F(data) => {
                settings.format = Format::Rgb32Float;
                data.iter().flat_map(|x| x.to_le_bytes()).collect()
            }
            DynamicImage::ImageRgba32F(data) => {
                settings.format = Format::Rgba32Float;
                data.iter().flat_map(|x| x.to_le_bytes()).collect()
            }
            _ => return Err(TextureError::InvalidFormat),
        };

        Self::from_raw(
            data,
            ViewTypeDim::D2 {
                x: dimensions.0,
                y: dimensions.1,
            },
            settings,
        )
    }
}

impl Texture {
    /// Returns a texture data slice.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Returns a mutable slice containing the data of this texture.
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Returns the dimensions of this texture.
    pub fn dimensions(&self) -> &ViewTypeDim {
        &self.dimensions
    }

    /// Resizes this texture to the given dimensions, leaving the original image at the top-left corner
    /// and filling the new area with zeroed data.
    ///
    /// - If the `target_dimensions` are **larger** than the current texture, the additional area is filled with zeros.
    /// - If the `target_dimensions` are **smaller**, the out-of-bounds data is discarded.
    pub fn resize(&mut self, target_dimensions: ViewTypeDim) {
        let old = self.dimensions.extent();
        let new = target_dimensions.extent();

        let copy_extent = [
            old[0].min(new[0]) as usize,
            old[1].min(new[1]) as usize,
            old[2].min(new[2]) as usize,
        ];

        let block_size = self.settings.format.block_size();

        let new_expected_size = Self::calculate_buffer_size(&target_dimensions, &self.settings);

        let mut new_texture = vec![0; new_expected_size];

        for z in 0..copy_extent[2] {
            for y in 0..copy_extent[1] {
                let old_row_start = (z * old[0] as usize * old[1] as usize * block_size)
                    + (y * old[0] as usize * block_size);
                let new_row_start = (z * new[0] as usize * new[1] as usize * block_size)
                    + (y * new[0] as usize * block_size);

                new_texture[new_row_start..new_row_start + copy_extent[0] * block_size]
                    .copy_from_slice(
                        &self.data[old_row_start..old_row_start + copy_extent[0] * block_size],
                    );
            }
        }

        self.data = new_texture;
        self.dimensions = target_dimensions;
    }

    /// Returns the settings of this texture.
    pub fn settings(&self) -> &TextureSettings {
        &self.settings
    }

    /// Sets the sampler of the settings of this texture.
    pub fn set_sampler(&mut self, sampler: Sampler) {
        self.settings.sampler = sampler;
    }
}

/// Errors that are possble when creating textures.
#[derive(thiserror::Error, Debug)]
pub enum TextureError {
    /// The texture data does not match with the provided format in the texture from bytes method.
    #[error("The format of the texture is not equivalent to the provided format.")]
    InvalidFormat,

    /// The size of the texture does not match with the given dimensions or format size.
    #[error(
        "The texture size does not match the given dimensions or format size.
        Expected size: {0}, Given size: {1},
        "
    )]
    InvalidSize(usize, usize),

    /// Gets returned when trying to create a texture with either width, height, depth or layer size set to 0.
    #[error("The size of a dimension can not equal zero.")]
    ZeroedDimension,

    /// Gets returned when trying to create a texture with a mip level not in valid range.
    #[error("The provided mip level of {0} is not in the valid range of accepted levels. Min is 1, Max is {1}")]
    InvalidMipLevel(u32, u32),
}

/// Represents the dimensions of a texture in the form of a view type.
///
/// This enum is used within the `Texture` struct to define the dimensionality
/// of the texture.
///
/// When using a cube map, the order in which the sides get defined is as follows:
///
/// 1. +x (right)
/// 2. -x (left)
/// 3. +y (up)
/// 4. -y (down)
/// 5. +z (front)
/// 6. -z (back)
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewTypeDim {
    /// One dimensional image where `x` symbolizes the width.
    D1 { x: u32 },
    /// Two dimensional image where `x` symbolizes the width and `y` the height.
    D2 { x: u32, y: u32 },
    /// Three dimensional image where `x` symbolizes the width, `y` the height and `z` the depth.
    D3 { x: u32, y: u32, z: u32 },
    /// Two dimensional image where `x` symbolizes the width of a cube map side and `y` the height.
    ///
    /// When this is used, six times the data is needed for each side of the texture.
    CubeMap { x: u32, y: u32 },
    /// One dimensional image array where `x` symbolizes the width and `layers` the amount of elements
    /// in the texture array.
    D1Array { x: u32, layers: u32 },
    /// One dimensional image array where `x` symbolizes the width, `y` the height and `layers` the amount of elements
    /// in the texture array.
    D2Array { x: u32, y: u32, layers: u32 },
    /// One dimensional image array where `x` symbolizes the width of all cube map sides, `y` the height and `layers`
    /// the amount of elements in the texture array.
    ///
    /// When this is used, six times the data is needed for each side of the texture.
    CubeArray { x: u32, y: u32, layers: u32 },
}

impl ViewTypeDim {
    /// Returns the amount of colors needed to represent a given view type.
    pub fn elements_num(&self) -> u64 {
        match *self {
            Self::D1 { x } => x as u64,
            Self::D2 { x, y } => x as u64 * y as u64,
            Self::D3 { x, y, z } => x as u64 * y as u64 * z as u64,
            Self::CubeMap { x, y } => x as u64 * y as u64 * 6,
            Self::D1Array { x, layers } => x as u64 * layers as u64,
            Self::D2Array { x, y, layers } => x as u64 * y as u64 * layers as u64,
            ViewTypeDim::CubeArray { x, y, layers } => x as u64 * y as u64 * layers as u64 * 6,
        }
    }

    /// Returns the three dimensional extent of this image.
    pub fn extent(&self) -> [u32; 3] {
        match *self {
            Self::D1 { x } | Self::D1Array { x, .. } => [x, 1, 1],
            Self::D2 { x, y }
            | Self::CubeMap { x, y }
            | Self::D2Array { x, y, .. }
            | ViewTypeDim::CubeArray { x, y, .. } => [x, y, 1],
            Self::D3 { x, y, z } => [x, y, z],
        }
    }

    /// Returns the amount of layers the array version of this texture has.
    /// If this texture is not an array this will return 1.
    pub fn array_layers(&self) -> u32 {
        match self {
            Self::D1Array { layers, .. }
            | Self::D2Array { layers, .. }
            | Self::CubeArray { layers, .. } => *layers,
            _ => 1,
        }
    }

    /// Returns the maximum applicable mip leveles possible for a texture of this dimension.
    pub fn max_mip_level(&self) -> u32 {
        match self {
            Self::D1 { x } | Self::D1Array { x, .. } => x.ilog2() + 1,
            Self::D2 { x, y }
            | Self::CubeMap { x, y }
            | Self::D2Array { x, y, .. }
            | ViewTypeDim::CubeArray { x, y, .. } => (x.max(y)).ilog2() + 1,
            Self::D3 { x, y, z } => (x.max(y).max(z)).ilog2() + 1,
        }
    }
}

/// One dimensional texture from `u32`, where x is the value.
impl From<u32> for ViewTypeDim {
    fn from(value: u32) -> Self {
        Self::D1 { x: value }
    }
}

/// Two dimensional texture from `(u32, u32)` where 0 is x and 1 is y.
impl From<(u32, u32)> for ViewTypeDim {
    fn from(value: (u32, u32)) -> Self {
        Self::D2 {
            x: value.0,
            y: value.1,
        }
    }
}

/// Three dimensional texture from `(u32, u32, u32)` where 0 is x, 1 is y and 2 is z.
impl From<(u32, u32, u32)> for ViewTypeDim {
    fn from(value: (u32, u32, u32)) -> Self {
        Self::D3 {
            x: value.0,
            y: value.1,
            z: value.2,
        }
    }
}

impl From<ViewTypeDim> for [u32; 3] {
    fn from(value: ViewTypeDim) -> Self {
        value.extent()
    }
}

/// Defines the filtering method used when sampling a texture.
///
/// Filtering determines how texels are interpolated when the texture is scaled up or down. It affects
/// the visual quality and performance when rendering a texture at a non-native resolution.
/// The `Filter` enum is used to select between different filtering techniques for minification, magnification,
/// and mipmap sampling in the `Sampler` struct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    Nearest,
    Linear,
}

/// Defines how texture coordinates outside the range 0 to 1 should behave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressMode {
    /// Repeats the texture in both directions x and y infinitely.
    /// This is used for tiling textures in repeating patterns.
    Repeat,

    /// Repeats the texture in both directions x and y, but mirrors the texture
    /// along the axes for every repeat. This is useful for creating mirrored effects.
    Mirrored,

    /// Clamps texture coordinates to the [0, 1] range.
    /// Coordinates outside this range are mapped to the nearest valid coordinate,
    /// causing the edges of the texture to appear static.
    ClampToEdge,

    /// When texture coordinates fall outside the range [0, 1],
    /// the color specified in the `Sampler`'s `border_color` is used.
    /// This allows for custom handling of out-of-bounds areas, such as transparent or black borders.
    ClampToBorder,
}

/// Specifies the color to use when the texture address mode is `ClampToBorder`
/// and the texture coordinates fall outside the [0, 1] range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderColor {
    /// The out-of-bounds area is rendered as transparent.
    Transparent,

    /// The out-of-bounds area is rendered as black.
    Black,

    /// The out-of-bounds area is rendered as white.
    White,
}

/// Defines a sampler for a texture, determining how the shader should sample the texture during rendering.
///
/// The `Sampler` struct encapsulates the filtering and addressing settings that affect how textures are
/// sampled in shaders. This includes settings for magnification and minification filters, mipmap filtering,
/// address modes for texture coordinates, and border colors for out-of-range coordinates. The sampler
/// settings are essential for controlling the appearance of textures in different rendering scenarios.
#[derive(Debug, Builder, Clone, PartialEq)]
#[builder(setter(into))]
pub struct Sampler {
    /// The filtering method used when the texture is scaled up.
    #[builder(default = "Filter::Nearest")]
    pub mag_filter: Filter,

    /// The filtering method used when the texture is scaled down.
    #[builder(default = "Filter::Linear")]
    pub min_filter: Filter,

    /// The filtering method used when sampling from a mipmap level (used for textures that have multiple levels).
    #[builder(default = "Filter::Nearest")]
    pub mipmap_mode: Filter,

    /// How out of range texture coordinates should be handled.
    #[builder(default = "[AddressMode::ClampToBorder; 3]")]
    pub address_mode: [AddressMode; 3],

    /// Color used for the border when the address mode is on `ClampToBorder`.
    #[builder(default = "BorderColor::Black")]
    pub border_color: BorderColor,
}

impl Default for Sampler {
    fn default() -> Self {
        Self {
            mag_filter: Filter::Nearest,
            min_filter: Filter::Linear,
            mipmap_mode: Filter::Nearest,
            address_mode: [AddressMode::ClampToBorder; 3],
            border_color: BorderColor::Black,
        }
    }
}

/// Configuration settings for a texture in the let-engine.
///
/// This struct contains settings that influence how a texture is managed and accessed,
/// including its sampler configuration, format, and access preferences.
///
/// # Usage
///
/// This struct is can be created using a builder pattern like this:
///
/// ```rust
/// let settings = TextureSettingsBuilder::default()
///     .sampler(my_sampler)
///     .format(my_format)
///     .preferred_access(Access::Reading)
///     .build()
///     .unwrap();
/// ```
///
/// `format` is the only field that is not initialized by default, so the build method can return
/// an uninitialized error if this is not set.
#[derive(Debug, Builder, Clone, PartialEq)]
#[builder(pattern = "owned", setter(into))]
pub struct TextureSettings {
    /// Specifies the sampling behavior for the texture.
    #[builder(default)]
    pub sampler: Sampler,

    #[builder(default = "1")]
    pub mip_levels: u32,

    /// Defines the format of the texture data.
    pub format: Format,

    /// Indicates the preferred access operation for the texture.
    /// Defaults to [`BufferAccess::Fixed`], a one time write with no reading or writing.
    ///
    /// The only allowed access patterns accepted for textures are
    /// [`BufferAccess::Fixed`] and [`BufferAccess::Staged`].
    #[builder(setter(custom), default = "BufferAccess::Fixed")]
    pub access_pattern: BufferAccess,
}

impl TextureSettingsBuilder {
    pub fn access_pattern(
        mut self,
        pattern: impl Into<BufferAccess>,
    ) -> Result<Self, TextureSettingsBuilderError> {
        let pattern: BufferAccess = pattern.into();
        self.access_pattern = match pattern {
            BufferAccess::Fixed | BufferAccess::Staged => Some(pattern),
            _ => {
                return Err(TextureSettingsBuilderError::ValidationError(
                    "Unsupported BufferAccess variant".to_string(),
                ))
            }
        };
        Ok(self)
    }
}
