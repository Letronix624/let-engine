//! Resources to be handled by the engine like textures, sounds and fonts.
//!
//! Panics the program in case the system is not capable of running the game engine.

use crate::objects::Color;

pub mod buffer;
pub mod data;
pub mod material;
pub mod model;
pub mod texture;

/// Specifies the formats for the texture when loaded from raw data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// 4 bits for red and green UNORM
    Rg4Unorm = 1,

    /// 4 bits for red, green, blue and alpha UNORM
    Rgba4Unorm = 2,

    /// 5 bits for red and blue, 6 bits for green UNORM
    R5G6B5Unorm = 4,

    /// 5 bits for red, green and blue, 1 bit for alpha UNORM
    Rgb5A1Unorm = 6,

    /// 8 bits red SRGB
    Sr8 = 15,

    /// 8 bits red and green SRGB
    Srg8 = 22,

    /// 8 bits red green blue SRGB
    Srgb8 = 29,

    /// 8 bits red green blue alpha SRGB
    Srgba8 = 43,

    /// 8 bits red UNORM
    R8Unorm = 9,

    /// 8 bits red green UNORM
    Rg8Unorm = 16,

    /// 8 bits red green blue UNORM
    Rgb8Unorm = 23,

    /// 8 bits red green blue alpha UNORM
    Rgba8Unorm = 37,

    /// 8 bit red unsigned integer
    R8Uint = 13,

    /// 8 bit red signed integer
    R8Sint = 14,

    /// 8 bit red green blue alpha unsigned integer
    Rgba8Uint = 41,

    /// 8 bit red green blue alpha signed integer
    Rgba8Sint = 42,

    /// 2 bits for alpha and 10 bits each for red, green and blue
    A2Rgb10Unorm = 58,

    /// 16 bit red float
    R16Float = 76,

    /// 16 bit red green float
    Rg16Float = 83,

    /// 16 bit red green blue alpha float
    Rgba16Float = 97,

    /// 16 bits red UNORM
    R16Unorm = 70,

    /// 16 bits red green UNORM
    Rg16Unorm = 77,

    /// 16 bits red green blue UNORM
    Rgb16Unorm = 84,

    /// 16 bits red green blue alpha UNORM
    Rgba16Unorm = 91,

    /// 32 bits red float
    R32Float = 100,

    /// 32 bits red green float
    Rg32Float = 103,

    /// 32 bits red green blue float
    Rgb32Float = 106,

    /// 32 bits red green blue alpha float
    Rgba32Float = 109,

    // /// 32 bit depth float
    // D32Float = 126,

    // /// 24 bit depth UNORM with 8 bit unsigned integer
    // D24UnormS8Uint = 129,
    /// BC1 block with no alpha
    Bc1RgbUnormBlock = 131,

    /// BC1 block with no alpha and SRGB encoding
    Bc1RgbSrgbBlock = 132,

    /// BC1 block with alpha
    Bc1RgbaUnormBlock = 133,

    /// BC1 block with alpha as SRGB encoding
    Bc1RgbaSrgbBlock = 134,

    /// BC2 block
    Bc2UnormBlock = 135,

    /// BC2 block with SRGB encoding
    Bc2SrgbBlock = 136,

    /// BC3 block
    Bc3UnormBlock = 137,

    /// BC3 block with SRGB encoding
    Bc3SrgbBlock = 138,

    /// Unsigned BC4 block
    Bc4UnormBlock = 139,

    /// Unsigned BC5 block
    Bc5UnormBlock = 141,

    /// BC7 block
    Bc7UnormBlock = 145,

    /// BC7 block with SRGB encoding
    Bc7SrgbBlock = 146,

    /// RGB ETC2 block
    Etc2Rgb8UnormBlock = 147,

    /// RGB ETC2 block with SRGB encoding
    Etc2Rgb8SrgbBlock = 148,

    /// RGB ETC2 block with punch-through alpha
    Etc2Rgb8A1UnormBlock = 149,

    /// RGB ETC2 block with punch-through alpha and SRGB encoding
    Etc2Rgb8A1SrgbBlock = 150,

    /// RGBA ETC2 block
    Etc2Rgb8A8UnormBlock = 151,

    /// RGBA ETC2 block with SRGB encoding
    Etc2Rgb8A8SrgbBlock = 152,

    /// Unsigned R11 EAC block
    EacR11UnormBlock = 153,

    /// Unsigned RG11 EAC block
    EacRg11UnormBlock = 155,

    /// 4x4 ASTC block
    Astc4x4UnormBlock = 157,

    /// 4x4 ASTC block with SRGB encoding
    Astc4x4SrgbBlock = 158,

    /// 5x4 ASTC block
    Astc5x4UnormBlock = 159,

    /// 5x4 ASTC block with SRGB encoding
    Astc5x4SrgbBlock = 160,

    /// 5x5 ASTC block
    Astc5x5UnormBlock = 161,

    /// 5x5 ASTC block with SRGB encoding
    Astc5x5SrgbBlock = 162,

    /// 6x5 ASTC block
    Astc6x5UnormBlock = 163,

    /// 6x5 ASTC block with SRGB encoding
    Astc6x5SrgbBlock = 164,

    /// 6x6 ASTC block
    Astc6x6UnormBlock = 165,

    /// 6x6 ASTC block with SRGB encoding
    Astc6x6SrgbBlock = 166,

    /// 8x5 ASTC block
    Astc8x5UnormBlock = 167,

    /// 8x5 ASTC block with SRGB encoding
    Astc8x5SrgbBlock = 168,

    /// 8x6 ASTC block
    Astc8x6UnormBlock = 169,

    /// 8x6 ASTC block with SRGB encoding
    Astc8x6SrgbBlock = 170,

    /// 8x8 ASTC block
    Astc8x8UnormBlock = 171,

    /// 8x8 ASTC block with SRGB encoding
    Astc8x8SrgbBlock = 172,

    /// 10x5 ASTC block
    Astc10x5UnormBlock = 173,

    /// 10x5 ASTC block with SRGB encoding
    Astc10x5SrgbBlock = 174,

    /// 10x6 ASTC block
    Astc10x6UnormBlock = 175,

    /// 10x6 ASTC block with SRGB encoding
    Astc10x6SrgbBlock = 176,

    /// 10x8 ASTC block
    Astc10x8UnormBlock = 177,

    /// 10x8 ASTC block with SRGB encoding
    Astc10x8SrgbBlock = 178,

    /// 10x10 ASTC block
    Astc10x10UnormBlock = 179,

    /// 10x10 ASTC block with SRGB encoding
    Astc10x10SrgbBlock = 180,

    /// 12x10 ASTC block
    Astc12x10UnormBlock = 181,

    /// 12x10 ASTC block with SRGB encoding
    Astc12x10SrgbBlock = 182,

    /// 12x12 ASTC block
    Astc12x12UnormBlock = 183,

    /// 12x12 ASTC block with SRGB encoding
    Astc12x12SrgbBlock = 184,
}

impl Format {
    pub fn block_extent(&self) -> [u32; 2] {
        match self {
            Self::Bc1RgbUnormBlock
            | Self::Bc1RgbSrgbBlock
            | Self::Bc1RgbaUnormBlock
            | Self::Bc1RgbaSrgbBlock
            | Self::Bc2UnormBlock
            | Self::Bc2SrgbBlock
            | Self::Bc3UnormBlock
            | Self::Bc3SrgbBlock
            | Self::Bc4UnormBlock
            | Self::Bc5UnormBlock
            | Self::Bc7UnormBlock
            | Self::Bc7SrgbBlock
            | Self::Etc2Rgb8UnormBlock
            | Self::Etc2Rgb8SrgbBlock
            | Self::Etc2Rgb8A1UnormBlock
            | Self::Etc2Rgb8A1SrgbBlock
            | Self::Etc2Rgb8A8UnormBlock
            | Self::Etc2Rgb8A8SrgbBlock
            | Self::EacR11UnormBlock
            | Self::EacRg11UnormBlock
            | Self::Astc4x4UnormBlock
            | Self::Astc4x4SrgbBlock => [4, 4],
            Self::Astc5x5UnormBlock | Self::Astc5x5SrgbBlock => [5, 5],
            Self::Astc6x5UnormBlock | Self::Astc6x5SrgbBlock => [6, 5],
            Self::Astc6x6UnormBlock | Self::Astc6x6SrgbBlock => [6, 6],
            Self::Astc8x5UnormBlock | Self::Astc8x5SrgbBlock => [8, 5],
            Self::Astc8x6UnormBlock | Self::Astc8x6SrgbBlock => [8, 6],
            Self::Astc8x8UnormBlock | Self::Astc8x8SrgbBlock => [8, 8],
            Self::Astc10x5UnormBlock | Self::Astc10x5SrgbBlock => [10, 5],
            Self::Astc10x6UnormBlock | Self::Astc10x6SrgbBlock => [10, 6],
            Self::Astc10x8UnormBlock | Self::Astc10x8SrgbBlock => [10, 8],
            Self::Astc10x10UnormBlock | Self::Astc10x10SrgbBlock => [10, 10],
            Self::Astc12x10UnormBlock | Self::Astc12x10SrgbBlock => [12, 10],
            Self::Astc12x12UnormBlock | Self::Astc12x12SrgbBlock => [12, 12],
            _ => [1, 1],
        }
    }

    /// Returns the amount of bytes present per pixel in the given format.
    pub fn block_size(&self) -> usize {
        match self {
            Self::Rg4Unorm => 1,
            Self::Rgba4Unorm => 2,
            Self::R5G6B5Unorm => 2,
            Self::Rgb5A1Unorm => 2,
            Self::Sr8 => 1,
            Self::Srg8 => 2,
            Self::Srgb8 => 3,
            Self::Srgba8 => 4,
            Self::R8Unorm => 1,
            Self::Rg8Unorm => 2,
            Self::Rgb8Unorm => 3,
            Self::Rgba8Unorm => 4,
            Self::R8Uint => 1,
            Self::R8Sint => 1,
            Self::Rgba8Uint => 4,
            Self::Rgba8Sint => 4,
            Self::A2Rgb10Unorm => 4,
            Self::R16Float => 2,
            Self::Rg16Float => 4,
            Self::Rgba16Float => 8,
            Self::R16Unorm => 2,
            Self::Rg16Unorm => 4,
            Self::Rgb16Unorm => 6,
            Self::Rgba16Unorm => 8,
            Self::R32Float => 4,
            Self::Rg32Float => 8,
            Self::Rgb32Float => 12,
            Self::Rgba32Float => 16,
            // Self::D32Float => 4,
            // Self::D24UnormS8Uint => 4,
            Self::Bc1RgbUnormBlock => 8,
            Self::Bc1RgbSrgbBlock => 8,
            Self::Bc1RgbaUnormBlock => 8,
            Self::Bc1RgbaSrgbBlock => 8,
            Self::Bc2UnormBlock => 16,
            Self::Bc2SrgbBlock => 16,
            Self::Bc3UnormBlock => 16,
            Self::Bc3SrgbBlock => 16,
            Self::Bc4UnormBlock => 8,
            Self::Bc5UnormBlock => 16,
            Self::Bc7UnormBlock => 16,
            Self::Bc7SrgbBlock => 16,
            Self::Etc2Rgb8UnormBlock => 8,
            Self::Etc2Rgb8SrgbBlock => 8,
            Self::Etc2Rgb8A1UnormBlock => 8,
            Self::Etc2Rgb8A1SrgbBlock => 8,
            Self::Etc2Rgb8A8UnormBlock => 16,
            Self::Etc2Rgb8A8SrgbBlock => 16,
            Self::EacR11UnormBlock => 8,
            Self::EacRg11UnormBlock => 16,
            Self::Astc4x4UnormBlock => 16,
            Self::Astc4x4SrgbBlock => 16,
            Self::Astc5x4UnormBlock => 16,
            Self::Astc5x4SrgbBlock => 16,
            Self::Astc5x5UnormBlock => 16,
            Self::Astc5x5SrgbBlock => 16,
            Self::Astc6x5UnormBlock => 16,
            Self::Astc6x5SrgbBlock => 16,
            Self::Astc6x6UnormBlock => 16,
            Self::Astc6x6SrgbBlock => 16,
            Self::Astc8x5UnormBlock => 16,
            Self::Astc8x5SrgbBlock => 16,
            Self::Astc8x6UnormBlock => 16,
            Self::Astc8x6SrgbBlock => 16,
            Self::Astc8x8UnormBlock => 16,
            Self::Astc8x8SrgbBlock => 16,
            Self::Astc10x5UnormBlock => 16,
            Self::Astc10x5SrgbBlock => 16,
            Self::Astc10x6UnormBlock => 16,
            Self::Astc10x6SrgbBlock => 16,
            Self::Astc10x8UnormBlock => 16,
            Self::Astc10x8SrgbBlock => 16,
            Self::Astc10x10UnormBlock => 16,
            Self::Astc10x10SrgbBlock => 16,
            Self::Astc12x10UnormBlock => 16,
            Self::Astc12x10SrgbBlock => 16,
            Self::Astc12x12UnormBlock => 16,
            Self::Astc12x12SrgbBlock => 16,
        }
    }

    /// Returns the amount of bits per component of this format.
    ///
    /// Components that are not present in the format have 0 bits.
    pub fn components(&self) -> [u8; 4] {
        match self {
            Self::Rg4Unorm => [4, 4, 0, 0],
            Self::Rgba4Unorm => [4; 4],
            Self::R5G6B5Unorm => [5, 6, 5, 0],
            Self::Rgb5A1Unorm => [5, 5, 5, 1],
            Self::Sr8 => [8, 0, 0, 0],
            Self::Srg8 => [8, 8, 0, 0],
            Self::Srgb8 => [8, 8, 8, 0],
            Self::Srgba8 => [8; 4],
            Self::R8Unorm => [8, 0, 0, 0],
            Self::Rg8Unorm => [8, 8, 0, 0],
            Self::Rgb8Unorm => [8, 8, 8, 0],
            Self::Rgba8Unorm => [8; 4],
            Self::R8Uint => [8, 0, 0, 0],
            Self::R8Sint => [8, 0, 0, 0],
            Self::Rgba8Uint => [8; 4],
            Self::Rgba8Sint => [8; 4],
            Self::A2Rgb10Unorm => [2, 10, 10, 10],
            Self::R16Float => [16, 0, 0, 0],
            Self::Rg16Float => [16, 16, 0, 0],
            Self::Rgba16Float => [16; 4],
            Self::R16Unorm => [16, 0, 0, 0],
            Self::Rg16Unorm => [16, 16, 0, 0],
            Self::Rgb16Unorm => [16, 16, 16, 0],
            Self::Rgba16Unorm => [16; 4],
            Self::R32Float => [32, 0, 0, 0],
            Self::Rg32Float => [32, 32, 0, 0],
            Self::Rgb32Float => [32, 32, 32, 0],
            Self::Rgba32Float => [32; 4],
            // Self::D32Float => [32, 0, 0, 0],
            // Self::D24UnormS8Uint => [24, 8, 0, 0],
            Self::Bc1RgbUnormBlock => [1, 1, 1, 0],
            Self::Bc1RgbSrgbBlock => [1, 1, 1, 0],
            Self::Bc1RgbaUnormBlock => [1; 4],
            Self::Bc1RgbaSrgbBlock => [1; 4],
            Self::Bc2UnormBlock => [1; 4],
            Self::Bc2SrgbBlock => [1; 4],
            Self::Bc3UnormBlock => [1; 4],
            Self::Bc3SrgbBlock => [1; 4],
            Self::Bc4UnormBlock => [1, 0, 0, 0],
            Self::Bc5UnormBlock => [1, 1, 0, 0],
            Self::Bc7UnormBlock => [1; 4],
            Self::Bc7SrgbBlock => [1; 4],
            Self::Etc2Rgb8UnormBlock => [1, 1, 1, 0],
            Self::Etc2Rgb8SrgbBlock => [1, 1, 1, 0],
            Self::Etc2Rgb8A1UnormBlock => [1; 4],
            Self::Etc2Rgb8A1SrgbBlock => [1; 4],
            Self::Etc2Rgb8A8UnormBlock => [1; 4],
            Self::Etc2Rgb8A8SrgbBlock => [1; 4],
            Self::EacR11UnormBlock => [11, 0, 0, 0],
            Self::EacRg11UnormBlock => [11, 11, 0, 0],
            Self::Astc4x4UnormBlock => [1; 4],
            Self::Astc4x4SrgbBlock => [1; 4],
            Self::Astc5x4UnormBlock => [1; 4],
            Self::Astc5x4SrgbBlock => [1; 4],
            Self::Astc5x5UnormBlock => [1; 4],
            Self::Astc5x5SrgbBlock => [1; 4],
            Self::Astc6x5UnormBlock => [1; 4],
            Self::Astc6x5SrgbBlock => [1; 4],
            Self::Astc6x6UnormBlock => [1; 4],
            Self::Astc6x6SrgbBlock => [1; 4],
            Self::Astc8x5UnormBlock => [1; 4],
            Self::Astc8x5SrgbBlock => [1; 4],
            Self::Astc8x6UnormBlock => [1; 4],
            Self::Astc8x6SrgbBlock => [1; 4],
            Self::Astc8x8UnormBlock => [1; 4],
            Self::Astc8x8SrgbBlock => [1; 4],
            Self::Astc10x5UnormBlock => [1; 4],
            Self::Astc10x5SrgbBlock => [1; 4],
            Self::Astc10x6UnormBlock => [1; 4],
            Self::Astc10x6SrgbBlock => [1; 4],
            Self::Astc10x8UnormBlock => [1; 4],
            Self::Astc10x8SrgbBlock => [1; 4],
            Self::Astc10x10UnormBlock => [1; 4],
            Self::Astc10x10SrgbBlock => [1; 4],
            Self::Astc12x10UnormBlock => [1; 4],
            Self::Astc12x10SrgbBlock => [1; 4],
            Self::Astc12x12UnormBlock => [1; 4],
            Self::Astc12x12SrgbBlock => [1; 4],
        }
    }

    pub fn format_type(&self) -> FormatType {
        match self {
            Self::Rg4Unorm => FormatType::UnsignedInt,
            Self::Rgba4Unorm => FormatType::UnsignedInt,
            Self::R5G6B5Unorm => FormatType::UnsignedInt,
            Self::Rgb5A1Unorm => FormatType::UnsignedInt,
            Self::Sr8 => FormatType::UnsignedInt,
            Self::Srg8 => FormatType::UnsignedInt,
            Self::Srgb8 => FormatType::UnsignedInt,
            Self::Srgba8 => FormatType::UnsignedInt,
            Self::R8Unorm => FormatType::UnsignedInt,
            Self::Rg8Unorm => FormatType::UnsignedInt,
            Self::Rgb8Unorm => FormatType::UnsignedInt,
            Self::Rgba8Unorm => FormatType::UnsignedInt,
            Self::R8Uint => FormatType::UnsignedInt,
            Self::R8Sint => FormatType::SignedInt,
            Self::Rgba8Uint => FormatType::UnsignedInt,
            Self::Rgba8Sint => FormatType::SignedInt,
            Self::A2Rgb10Unorm => FormatType::SignedInt,
            Self::R16Float => FormatType::Float,
            Self::Rg16Float => FormatType::Float,
            Self::Rgba16Float => FormatType::Float,
            Self::R16Unorm => FormatType::UnsignedInt,
            Self::Rg16Unorm => FormatType::UnsignedInt,
            Self::Rgb16Unorm => FormatType::UnsignedInt,
            Self::Rgba16Unorm => FormatType::UnsignedInt,
            Self::R32Float => FormatType::Float,
            Self::Rg32Float => FormatType::Float,
            Self::Rgb32Float => FormatType::Float,
            Self::Rgba32Float => FormatType::Float,
            // Self::D32Float => FormatType::Float,
            // Self::D24UnormS8Uint => FormatType::UnsignedInt,
            Self::Bc1RgbUnormBlock => FormatType::UnsignedInt,
            Self::Bc1RgbSrgbBlock => FormatType::UnsignedInt,
            Self::Bc1RgbaUnormBlock => FormatType::UnsignedInt,
            Self::Bc1RgbaSrgbBlock => FormatType::UnsignedInt,
            Self::Bc2UnormBlock => FormatType::UnsignedInt,
            Self::Bc2SrgbBlock => FormatType::UnsignedInt,
            Self::Bc3UnormBlock => FormatType::UnsignedInt,
            Self::Bc3SrgbBlock => FormatType::UnsignedInt,
            Self::Bc4UnormBlock => FormatType::UnsignedInt,
            Self::Bc5UnormBlock => FormatType::UnsignedInt,
            Self::Bc7UnormBlock => FormatType::UnsignedInt,
            Self::Bc7SrgbBlock => FormatType::UnsignedInt,
            Self::Etc2Rgb8UnormBlock => FormatType::UnsignedInt,
            Self::Etc2Rgb8SrgbBlock => FormatType::UnsignedInt,
            Self::Etc2Rgb8A1UnormBlock => FormatType::UnsignedInt,
            Self::Etc2Rgb8A1SrgbBlock => FormatType::UnsignedInt,
            Self::Etc2Rgb8A8UnormBlock => FormatType::UnsignedInt,
            Self::Etc2Rgb8A8SrgbBlock => FormatType::UnsignedInt,
            Self::EacR11UnormBlock => FormatType::UnsignedInt,
            Self::EacRg11UnormBlock => FormatType::UnsignedInt,
            Self::Astc4x4UnormBlock => FormatType::UnsignedInt,
            Self::Astc4x4SrgbBlock => FormatType::UnsignedInt,
            Self::Astc5x4UnormBlock => FormatType::UnsignedInt,
            Self::Astc5x4SrgbBlock => FormatType::UnsignedInt,
            Self::Astc5x5UnormBlock => FormatType::UnsignedInt,
            Self::Astc5x5SrgbBlock => FormatType::UnsignedInt,
            Self::Astc6x5UnormBlock => FormatType::UnsignedInt,
            Self::Astc6x5SrgbBlock => FormatType::UnsignedInt,
            Self::Astc6x6UnormBlock => FormatType::UnsignedInt,
            Self::Astc6x6SrgbBlock => FormatType::UnsignedInt,
            Self::Astc8x5UnormBlock => FormatType::UnsignedInt,
            Self::Astc8x5SrgbBlock => FormatType::UnsignedInt,
            Self::Astc8x6UnormBlock => FormatType::UnsignedInt,
            Self::Astc8x6SrgbBlock => FormatType::UnsignedInt,
            Self::Astc8x8UnormBlock => FormatType::UnsignedInt,
            Self::Astc8x8SrgbBlock => FormatType::UnsignedInt,
            Self::Astc10x5UnormBlock => FormatType::UnsignedInt,
            Self::Astc10x5SrgbBlock => FormatType::UnsignedInt,
            Self::Astc10x6UnormBlock => FormatType::UnsignedInt,
            Self::Astc10x6SrgbBlock => FormatType::UnsignedInt,
            Self::Astc10x8UnormBlock => FormatType::UnsignedInt,
            Self::Astc10x8SrgbBlock => FormatType::UnsignedInt,
            Self::Astc10x10UnormBlock => FormatType::UnsignedInt,
            Self::Astc10x10SrgbBlock => FormatType::UnsignedInt,
            Self::Astc12x10UnormBlock => FormatType::UnsignedInt,
            Self::Astc12x10SrgbBlock => FormatType::UnsignedInt,
            Self::Astc12x12UnormBlock => FormatType::UnsignedInt,
            Self::Astc12x12SrgbBlock => FormatType::UnsignedInt,
        }
    }

    pub fn sampled_format_type(&self) -> SampledFormatType {
        match self {
            Self::Rg4Unorm => SampledFormatType::Float,
            Self::Rgba4Unorm => SampledFormatType::Float,
            Self::R5G6B5Unorm => SampledFormatType::Float,
            Self::Rgb5A1Unorm => SampledFormatType::Float,
            Self::Sr8 => SampledFormatType::Float,
            Self::Srg8 => SampledFormatType::Float,
            Self::Srgb8 => SampledFormatType::Float,
            Self::Srgba8 => SampledFormatType::Float,
            Self::R8Unorm => SampledFormatType::Float,
            Self::Rg8Unorm => SampledFormatType::Float,
            Self::Rgb8Unorm => SampledFormatType::Float,
            Self::Rgba8Unorm => SampledFormatType::Float,
            Self::R8Uint => SampledFormatType::Int,
            Self::R8Sint => SampledFormatType::Int,
            Self::Rgba8Uint => SampledFormatType::Int,
            Self::Rgba8Sint => SampledFormatType::Int,
            Self::A2Rgb10Unorm => SampledFormatType::Float,
            Self::R16Float => SampledFormatType::Float,
            Self::Rg16Float => SampledFormatType::Float,
            Self::Rgba16Float => SampledFormatType::Float,
            Self::R16Unorm => SampledFormatType::Float,
            Self::Rg16Unorm => SampledFormatType::Float,
            Self::Rgb16Unorm => SampledFormatType::Float,
            Self::Rgba16Unorm => SampledFormatType::Float,
            Self::R32Float => SampledFormatType::Float,
            Self::Rg32Float => SampledFormatType::Float,
            Self::Rgb32Float => SampledFormatType::Float,
            Self::Rgba32Float => SampledFormatType::Float,
            // Self::D32Float => SampledFormatType::Float,
            // Self::D24UnormS8Uint => SampledFormatType::Int, // TODO: Oops
            Self::Bc1RgbUnormBlock => SampledFormatType::Float,
            Self::Bc1RgbSrgbBlock => SampledFormatType::Float,
            Self::Bc1RgbaUnormBlock => SampledFormatType::Float,
            Self::Bc1RgbaSrgbBlock => SampledFormatType::Float,
            Self::Bc2UnormBlock => SampledFormatType::Float,
            Self::Bc2SrgbBlock => SampledFormatType::Float,
            Self::Bc3UnormBlock => SampledFormatType::Float,
            Self::Bc3SrgbBlock => SampledFormatType::Float,
            Self::Bc4UnormBlock => SampledFormatType::Float,
            Self::Bc5UnormBlock => SampledFormatType::Float,
            Self::Bc7UnormBlock => SampledFormatType::Float,
            Self::Bc7SrgbBlock => SampledFormatType::Float,
            Self::Etc2Rgb8UnormBlock => SampledFormatType::Float,
            Self::Etc2Rgb8SrgbBlock => SampledFormatType::Float,
            Self::Etc2Rgb8A1UnormBlock => SampledFormatType::Float,
            Self::Etc2Rgb8A1SrgbBlock => SampledFormatType::Float,
            Self::Etc2Rgb8A8UnormBlock => SampledFormatType::Float,
            Self::Etc2Rgb8A8SrgbBlock => SampledFormatType::Float,
            Self::EacR11UnormBlock => SampledFormatType::Float,
            Self::EacRg11UnormBlock => SampledFormatType::Float,
            Self::Astc4x4UnormBlock => SampledFormatType::Float,
            Self::Astc4x4SrgbBlock => SampledFormatType::Float,
            Self::Astc5x4UnormBlock => SampledFormatType::Float,
            Self::Astc5x4SrgbBlock => SampledFormatType::Float,
            Self::Astc5x5UnormBlock => SampledFormatType::Float,
            Self::Astc5x5SrgbBlock => SampledFormatType::Float,
            Self::Astc6x5UnormBlock => SampledFormatType::Float,
            Self::Astc6x5SrgbBlock => SampledFormatType::Float,
            Self::Astc6x6UnormBlock => SampledFormatType::Float,
            Self::Astc6x6SrgbBlock => SampledFormatType::Float,
            Self::Astc8x5UnormBlock => SampledFormatType::Float,
            Self::Astc8x5SrgbBlock => SampledFormatType::Float,
            Self::Astc8x6UnormBlock => SampledFormatType::Float,
            Self::Astc8x6SrgbBlock => SampledFormatType::Float,
            Self::Astc8x8UnormBlock => SampledFormatType::Float,
            Self::Astc8x8SrgbBlock => SampledFormatType::Float,
            Self::Astc10x5UnormBlock => SampledFormatType::Float,
            Self::Astc10x5SrgbBlock => SampledFormatType::Float,
            Self::Astc10x6UnormBlock => SampledFormatType::Float,
            Self::Astc10x6SrgbBlock => SampledFormatType::Float,
            Self::Astc10x8UnormBlock => SampledFormatType::Float,
            Self::Astc10x8SrgbBlock => SampledFormatType::Float,
            Self::Astc10x10UnormBlock => SampledFormatType::Float,
            Self::Astc10x10SrgbBlock => SampledFormatType::Float,
            Self::Astc12x10UnormBlock => SampledFormatType::Float,
            Self::Astc12x10SrgbBlock => SampledFormatType::Float,
            Self::Astc12x12UnormBlock => SampledFormatType::Float,
            Self::Astc12x12SrgbBlock => SampledFormatType::Float,
        }
    }

    /// Takes a color and creates a buffer as close as possible to the given color.
    pub fn color_to_buffer(&self, color: &Color) -> Vec<u8> {
        let mut buffer: Vec<u8> = Vec::with_capacity(self.block_size());

        const MAXU8: f32 = u8::MAX as f32;
        const MAXU16: f32 = u16::MAX as f32;
        const MAXU24: f32 = 16777215.0;

        let components = self.components();

        let format = self.format_type();

        for (component, color) in components.iter().zip(color.rgba().into_iter()) {
            match (&format, component) {
                (FormatType::UnsignedInt, 8) => {
                    buffer.push((color * MAXU8) as u8);
                }
                (FormatType::SignedInt, 8) => {
                    buffer.push((color * MAXU8) as u8);
                }
                (FormatType::UnsignedInt, 16) => {
                    buffer.extend(((color * MAXU16) as u16).to_le_bytes());
                }
                (FormatType::UnsignedInt, 24) => {
                    let bytes = ((color.clamp(0.0, 1.0) * MAXU24) as u32).to_le_bytes();

                    buffer.extend([bytes[0], bytes[1], bytes[2]]);
                }
                (FormatType::UnsignedInt, 32) => {
                    buffer.extend((color as u32).to_le_bytes());
                }
                (FormatType::Float, 16) => {
                    buffer.extend(half::f16::from_f32(color).to_le_bytes());
                }
                (FormatType::Float, 32) => {
                    buffer.extend(color.to_le_bytes());
                }
                _ => (),
            }
        }

        buffer
    }
}

/// The type of the format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatType {
    UnsignedInt,
    SignedInt,
    Float,
}

/// The representation of this format in shader code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampledFormatType {
    Int,
    Float,
}

// TODO
// /// Merges a pipeline cache into the resources potentially making the creation of materials faster.
// ///
// /// # Safety
// ///
// /// Unsafe because vulkan blindly trusts that this data comes from the `get_pipeline_binary` function.
// /// The program will panic if the data provided is not right.
// ///
// /// The binary given to the function must be made with the same hardware and vulkan driver version.
// pub unsafe fn load_pipeline_cache(data: &[u8]) -> Result<()> {
//     let cache = PipelineCache::new(
//         resources()?.vulkan().device.clone(),
//         PipelineCacheCreateInfo {
//             initial_data: data.to_vec(),
//             ..Default::default()
//         },
//     )?;
//     resources()?
//         .vulkan()
//         .lock()
//         .pipeline_cache
//         .merge([cache.as_ref()])?;
//     Ok(())
// }

// /// Returns the binary of the pipeline cache.
// ///
// /// Allows this binary to be loaded with the `load_pipeline_cache` function to make loading materials potentially faster.
// pub fn pipeline_binary() -> Result<Vec<u8>> {
//     Ok(resources()?.vulkan().lock().pipeline_cache.get_data()?)
// }

// /// Loads a new write operation for a shader.
// pub fn new_descriptor_write<T: BufferContents>(buf: T, set: u32) -> Result<WriteDescriptorSet> {
//     let vulkan = resources()?.vulkan().lock();
//     vulkan.write_descriptor(buf, set)
// }
