//! Engine wide settings that are applicable using the [Settings](crate::settings::Settings) struct.

use let_engine_core::backend::{
    audio::AudioSettings, graphics::GraphicsBackend, networking::NetworkingBackend, Backends,
};

use crate::tick_system::TickSettings;

/// The initial settings of this engine.
pub struct EngineSettings<B: Backends> {
    /// Settings that determines the look of the window.
    #[cfg(feature = "client")]
    pub window: crate::window::WindowBuilder,

    /// The initial settings of the tick system.
    pub tick_system: TickSettings,

    pub graphics: <B::Graphics as GraphicsBackend>::Settings,
    // #[cfg(feature = "client")]
    pub audio: AudioSettings,
    pub networking: <B::Networking as NetworkingBackend>::Settings,
}

impl<B: Backends> Default for EngineSettings<B> {
    fn default() -> Self {
        Self {
            #[cfg(feature = "client")]
            window: crate::window::WindowBuilder::default(),
            tick_system: TickSettings::default(),
            graphics: <B::Graphics as GraphicsBackend>::Settings::default(),
            audio: AudioSettings::default(),
            networking: <B::Networking as NetworkingBackend>::Settings::default(),
        }
    }
}

impl<B: Backends> Clone for EngineSettings<B> {
    fn clone(&self) -> Self {
        Self {
            #[cfg(feature = "client")]
            window: self.window.clone(),
            tick_system: self.tick_system.clone(),
            graphics: self.graphics.clone(),
            audio: self.audio.clone(),
            networking: self.networking.clone(),
        }
    }
}

impl<B: Backends> EngineSettings<B> {
    /// Sets the value `window` and returns self.
    #[cfg(feature = "client")]
    pub fn window(mut self, window: crate::window::WindowBuilder) -> Self {
        self.window = window;
        self
    }

    /// Sets the value `tick_system` and returns self.
    pub fn tick_system(mut self, tick_system: TickSettings) -> Self {
        self.tick_system = tick_system;
        self
    }

    /// Sets the value `graphics` and returns self.
    pub fn graphics(mut self, graphics: <B::Graphics as GraphicsBackend>::Settings) -> Self {
        self.graphics = graphics;
        self
    }

    // /// Sets the value `networking` and returns self.
    // pub fn networking(
    //     mut self,
    //     networking: <B::Networking as NetworkingBackend>::Settings,
    // ) -> Self {
    //     self.networking = networking;
    //     self
    // }
}
