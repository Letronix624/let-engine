use let_engine_core::backend;

#[cfg(feature = "default_graphics_backend")]
pub mod graphics;
#[cfg(feature = "default_networking_backend")]
pub mod networking;

/// The backends used by default.
///
/// Disabling those features leaves () in it's places, disabling the functionality entirely.
#[derive(Debug)]
pub struct DefaultBackends;

impl backend::Backends for DefaultBackends {
    #[cfg(feature = "default_graphics_backend")]
    type Graphics = graphics::DefaultGraphicsBackend;
    #[cfg(not(feature = "default_graphics_backend"))]
    type Graphics = ();

    type Kira = let_engine_core::backend::audio::MockBackend;

    type Networking = ();
}
