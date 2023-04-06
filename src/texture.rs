pub use vulkano::sampler::BorderColor;
use vulkano::sampler::{
    Filter as vkFilter, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode,
};

/// Formats for the texture from raw data.
pub enum Format {
    R8,
    RGBA8,
    RGBA16,
}

/// Filtering mode
pub enum Filter {
    Nearest,
    Linear,
}

/// Handling of pixels outside the position range of the texture.
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
pub struct Sampler {
    /// Way to filter the texture when the texture is bigger than it's actual resolution.
    pub mag_filter: Filter,
    /// Way to filter the texture when it's smaller than the actual texture.
    pub min_filter: Filter,
    /// How the final sampled value should be calculated from the samples of individual mipmaps.
    pub mipmap_mode: Filter,
    /// How out of range texture coordinates should be handled.
    pub address_mode: [AddressMode; 3],
    /// Color for the border when the address mode is on ClampToBorder.
    pub border_color: BorderColor,
}

/// The main texture settings.
pub struct TextureSettings {
    /// Raw image format
    pub format: Format,
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
            address_mode: [
                AddressMode::ClampToBorder,
                AddressMode::ClampToBorder,
                AddressMode::ClampToBorder,
            ],
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
            format: Format::RGBA8,
            srgb: true,
            sampler: Sampler::default(),
        }
    }
}

impl TextureSettings {
    pub fn format(mut self, format: Format) -> Self {
        self.format = format;
        self
    }
    pub fn srgb(mut self, srgb: bool) -> Self {
        self.srgb = srgb;
        self
    }
    pub fn sampler(mut self, sampler: Sampler) -> Self {
        self.sampler = sampler;
        self
    }
}
