//! Texture based errors.

use thiserror::Error;

/// Errors that occur from textures.
#[derive(Error, Debug)]
pub enum TextureError {
    /// This error gets returned when you set the texture ID of an Appearance object higher than the
    /// actual frame count of the texture this object is holding.
    #[error("The layer you set for this object does not exist:\n{0}")]
    Layer(String),
    /// This error gets returned when a function gets called that requires an object to have a texture
    /// but it does not have one.
    #[error("The object you ran a function on that requires a texture does not have one.")]
    NoTexture,
    /// This error gets returned when you give the wrong format to the texture when trying to create a
    /// new texture.
    #[error("The given format does not match with the bytes provided:\n{0}")]
    InvalidFormat(String),
}
