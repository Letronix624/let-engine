//! All errors that can occur in this engine.

use thiserror::Error;

/// Drawing related errors.
pub mod draw;
/// Object and layer related errors.
pub mod objects;
/// Texture related errors.
pub mod textures;

/// Your device's specifications do not hold up to the minimum requirements of this engine.
#[derive(Debug, Error)]
#[error("Your device does not fulfill the required specification to run this application:\n{0}")]
pub struct RequirementError(pub String);

/// Your shaders input into the material have an error.
#[derive(Debug, Error)]
pub enum ShaderError {
    // Make the entry point chossable.
    /// Your shader is missing a function called `main`.
    #[error("The main function was not found in this shader")]
    EntryPoint,
}
