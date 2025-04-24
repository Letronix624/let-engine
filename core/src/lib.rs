pub mod backend;
pub mod camera;
pub mod objects;
pub mod resources;

use foldhash::HashMap;
use parking_lot::Mutex;
use thiserror::Error;

pub use glam;

extern crate self as let_engine_core;

/// The game engine failed to start for the following reasons:
#[derive(Debug, Error)]
pub enum EngineError<G>
where
    G: std::error::Error,
    // A: std::error::Error,
    // N: std::error::Error,
{
    /// It is only possible to create the engine one time.
    #[error("Can not start another engine instance in the same application.")]
    Recreation,

    /// An error given by the used graphics backend upon creation.
    #[error("{0}")]
    GraphicsBackend(G),
    // /// An error given by the used audio backend upon creation.
    // #[error("{0}")]
    // AudioBackend(A),

    // /// An error given by the used networking backend upon creation.
    // #[error("{0}")]
    // NetworkingBackend(N),
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
