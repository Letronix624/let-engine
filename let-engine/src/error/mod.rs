//! All errors that can occur in this engine.

use thiserror::Error;

/// Drawing related errors.
#[cfg(feature = "client")]
pub mod draw;
/// Object and layer related errors.
pub mod objects;
/// Texture related errors.
pub mod textures;

/// The model you are trying to load has empty data.
///
/// Use `apperance.set_visible(false)` instead.
#[derive(Debug, Error)]
#[error("The model you are trying to load has empty data.")]
pub struct NoDataError;

/// The game engine failed to start for the following reasons:
#[derive(Debug, Error)]
pub enum EngineStartError {
    /// Your device's specifications do not hold up to the minimum requirements of this engine.
    #[error(
        "Your device does not fulfill the required specification to run this application:\n{0}"
    )]
    RequirementError(String),
    /// Engine can only be made once.
    #[error("You can only initialize this game engine one single time.")]
    EngineInitialized,
    #[error("Failed to initialize drawing backend:\n{0}")]
    #[cfg(feature = "client")]
    DrawingBackendError(anyhow::Error),
    #[error("Could not start the game engine for some reason:\n{0}")]
    Other(String),
}
