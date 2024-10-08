//! Engine wide settings that are applicable using the [Settings](crate::settings::Settings) struct.

#[allow(unused_imports)]
use std::sync::Arc;

use derive_builder::Builder;

#[cfg(feature = "client")]
use let_engine_core::draw::{Graphics, PresentMode};
// audio feature
#[cfg(feature = "audio")]
#[cfg(feature = "client")]
use let_engine_audio::Audio;

#[cfg(feature = "networking")]
use crate::networking::Networking;

#[cfg(feature = "client")]
use super::WindowBuilder;

use super::TickSettings;

use parking_lot::{Condvar, Mutex};

/// The initial settings of this engine.
#[derive(Clone, Builder, Default)]
pub struct EngineSettings {
    /// Settings that determines the look of the window.
    #[builder(setter(into, strip_option), default)]
    #[cfg(feature = "client")]
    pub window_settings: WindowBuilder,
    /// The initial settings of the tick system.
    #[builder(setter(into), default)]
    pub tick_settings: TickSettings,
}

/// General in game settings built into the game engine.
pub struct Settings<#[cfg(feature = "client")] G, #[cfg(feature = "audio")] A> {
    pub tick_system: TickSystem,
    #[cfg(feature = "client")]
    pub graphics: G,
    #[cfg(feature = "audio")]
    pub audio: A,
    #[cfg(feature = "networking")]
    pub networking: Networking,
}

#[cfg(feature = "client")]
macro_rules! impl_settings {
    { impl Settings $implementation:tt } => {
        #[cfg(not(feature = "audio"))]
        impl Settings<std::sync::Arc<Graphics>> $implementation

        #[cfg(feature = "audio")]
        impl Settings<std::sync::Arc<Graphics>, Audio> $implementation
    };
}

#[cfg(feature = "client")]
impl_settings! {
impl Settings {
    pub(crate) fn new() -> Self {
        Self {
            tick_system: TickSystem::new(),
            #[cfg(feature = "client")]
            graphics: Arc::new(Graphics::new(PresentMode::Fifo)),
            #[cfg(feature = "audio")]
            audio: Audio::default(),
            #[cfg(feature = "networking")]
            networking: Networking::new(),
        }
    }

    /// Cleans all caches on both ram and vram for unused data. This decreases memory usage and may not
    /// hurt to be called between levels from time to time.
    #[cfg(feature = "client")]
    pub fn clean_caches(&self) {
        #[cfg(feature = "networking")]
        super::networking::LAST_ORDS.lock().clear();

        #[cfg(feature = "asset_system")]
        asset_system::clear_cache();
    }
}
}

#[cfg(not(feature = "client"))]
impl Settings {
    pub(crate) fn new() -> Self {
        Self {
            tick_system: TickSystem::new(),
            #[cfg(feature = "networking")]
            networking: Networking::new(),
        }
    }
}

/// Engine wide tick system settings.
pub struct TickSystem {
    pub(super) tick_settings: Mutex<TickSettings>,
    pub(super) tick_pause_lock: (Mutex<bool>, Condvar),
}

impl TickSystem {
    pub(crate) fn new() -> Self {
        Self {
            tick_settings: Mutex::new(TickSettings::default()),
            tick_pause_lock: (Mutex::new(false), Condvar::new()),
        }
    }
    /// Returns the engine wide tick settings.
    pub fn get(&self) -> TickSettings {
        self.tick_settings.lock().clone()
    }
    /// Sets and applies the tick settings of the game engine.
    pub fn set(&self, settings: TickSettings) {
        *self.tick_pause_lock.0.lock() = settings.paused;
        *self.tick_settings.lock() = settings;
        self.tick_pause_lock.1.notify_all();
    }
}
