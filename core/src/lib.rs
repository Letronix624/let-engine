pub mod backend;
pub mod camera;
pub mod objects;
pub mod resources;

use backend::audio::{AudioBackend, AudioBackendError};
use backend::networking::NetworkingBackend;
use backend::{Backends, graphics::GraphicsBackend};

use parking_lot::Mutex;
use thiserror::Error;

pub use glam;

extern crate self as let_engine_core;

/// The game engine failed to start for the following reasons:
#[derive(Error)]
pub enum EngineError<B>
where
    B: Backends,
{
    /// It is only possible to create the engine one time.
    #[error("Can not start another engine instance in the same application.")]
    Recreation,

    /// An error given by the used graphics backend upon creation.
    #[error("{0:?}")]
    GraphicsBackend(<B::Graphics as GraphicsBackend>::Error),

    // /// An error given by the used audio backend.
    #[error("{0:?}")]
    AudioBackend(AudioBackendError<<B::Kira as AudioBackend>::Error>),

    /// An error given by the used networking backend upon creation.
    #[error("{0:?}")]
    NetworkingBackend(<B::Networking as NetworkingBackend>::Error),
}

impl<B: Backends> std::fmt::Debug for EngineError<B>
where
    B: Backends,
    <B::Kira as AudioBackend>::Error: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recreation => {
                write!(
                    f,
                    "Can not start another engine instance in the same application."
                )?;
            }
            Self::GraphicsBackend(e) => {
                write!(f, "{e}")?;
            }
            Self::AudioBackend(e) => {
                write!(f, "{e:?}")?;
            }
            Self::NetworkingBackend(e) => {
                write!(f, "{e:?}")?;
            }
        };
        Ok(())
    }
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
