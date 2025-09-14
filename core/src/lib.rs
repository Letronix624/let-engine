pub mod backend;
pub mod camera;
pub mod objects;
pub mod resources;

use backend::audio::{AudioBackend, AudioBackendError};
use backend::networking::NetworkingBackend;
use backend::{Backends, gpu::GpuBackend};

use parking_lot::Mutex;
use thiserror::Error;

pub use glam;

extern crate self as let_engine_core;

pub trait CustomError: Send + Sync + 'static {}

impl<T: Send + Sync + 'static> CustomError for T {}

type KiraError<B> = AudioBackendError<<<B as Backends>::Kira as AudioBackend>::Error>;

#[derive(Error)]
pub enum EngineError<E, B>
where
    E: CustomError,
    B: Backends,
{
    /// It is only possible to create the engine one time.
    #[error("Can not start another engine instance in the same application.")]
    Recreation,

    /// An error given by the used gpu backend upon creation.
    #[error(transparent)]
    GpuBackend(<B::Gpu as GpuBackend>::Error),

    /// An error given by the used audio backend.
    #[error(transparent)]
    AudioBackend(KiraError<B>),
    /// An error given by the used networking backend.
    #[error(transparent)]
    NetworkingBackend(<B::Networking as NetworkingBackend>::Error),

    /// Custom user error that can return from executing a game state update.
    #[error(transparent)]
    Custom(E),
}

// TEMP
unsafe impl<E: CustomError, B: Backends> Send for EngineError<E, B> {}
unsafe impl<E: CustomError, B: Backends> Sync for EngineError<E, B> {}

impl<B: Backends, E: CustomError> std::fmt::Debug for EngineError<E, B>
where
    B: Backends,
    E: std::fmt::Debug,
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
            Self::GpuBackend(e) => {
                write!(f, "{e}")?;
            }
            Self::AudioBackend(e) => {
                write!(f, "{e:?}")?;
            }
            Self::NetworkingBackend(e) => {
                write!(f, "{e:?}")?;
            }
            Self::Custom(e) => {
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
