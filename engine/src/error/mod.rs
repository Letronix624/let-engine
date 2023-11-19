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

/// The model you are trying to load has empty data.
///
/// Use `apperance.set_visible(false)` instead.
#[derive(Debug, Error)]
#[error("The model you are trying to load has empty data.")]
pub struct NoDataError;
