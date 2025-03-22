use let_engine_core::backend::Backends;

#[cfg(feature = "default_graphics_backend")]
pub mod graphics;

// #[cfg(feature = "default_networking_backend")]
// pub mod networking;

#[cfg(feature = "default_audio_backend")]
pub mod audio;

/// The backends used by default.
///
/// Disabling those features leaves () in it's places, disabling the functionality entirely.
pub struct DefaultBackends;

impl Backends for DefaultBackends {
    #[cfg(feature = "default_graphics_backend")]
    type Graphics = graphics::DefaultGraphicsBackend;
    #[cfg(not(feature = "default_graphics_backend"))]
    type Graphics = ();

    // type Audio = ();

    // type Networking = ();
}
