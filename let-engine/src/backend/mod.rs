use let_engine_core::backend;

#[cfg(feature = "default_gpu_backend")]
pub mod gpu;
#[cfg(feature = "default_networking_backend")]
pub mod networking;

/// The backends used by default.
///
/// Disabling those features leaves () in it's places, disabling the functionality entirely.
#[derive(Debug)]
pub struct DefaultBackends;

impl backend::Backends for DefaultBackends {
    #[cfg(feature = "default_gpu_backend")]
    type Gpu = gpu::DefaultGpuBackend;
    #[cfg(not(feature = "default_gpu_backend"))]
    type Gpu = ();

    type Kira = let_engine_core::backend::audio::MockBackend;

    type Networking = ();
}
