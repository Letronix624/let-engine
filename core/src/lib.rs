pub mod backend;
pub mod camera;
pub mod objects;
pub mod resources;
pub mod utils;

use foldhash::HashMap;
use parking_lot::Mutex;
use thiserror::Error;

pub use glam;

extern crate self as let_engine_core;

/// The game engine failed to start for the following reasons:
#[derive(Debug, Error)]
pub enum EngineError {
    /// Your device's specifications do not hold up to the minimum requirements of this engine.
    #[error(
        "Your device does not fulfill the required specification to run this application: {0}"
    )]
    RequirementError(String),
    /// Engine can only be made once.
    #[error("You can only initialize this game engine one single time.")]
    EngineInitialized,
    /// Failed to initialize drawing backend of the game engine.
    #[error("Failed to initialize drawing backend: {0}")]
    DrawingBackendError(anyhow::Error),
    /// The game engine is not ready to load resources.
    #[error("The game engine is not ready to load resources right now. You have to initialize the game engine first.")]
    NotReady,
    #[error("Could not start the game engine for some reason: {0}")]
    Other(anyhow::Error),
}

/// Cardinal direction
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Center,
    N,
    No,
    O,
    So,
    S,
    Sw,
    W,
    Nw,
}
