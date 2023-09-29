//! Object based errors.

use thiserror::Error;

/// This error gets returned when you set the texture ID of an Appearance object higher than the
/// actual frame count of the texture this object is holding.
#[derive(Error, Debug)]
#[error("The texture does not have this ID.")]
pub struct TextureIDError;

/// This error gets returned when a function gets called that requires an object to have a texture
/// but it does not have one.
#[derive(Error, Debug)]
#[error("This object doesn't have a texture.")]
pub struct NoTextureError;

/// This error gets returned when you give the wrong format to the texture when trying to create a
/// new texture.
#[derive(Error, Debug)]
#[error("The format doesn't work with this image.")]
pub struct InvalidFormatError;
