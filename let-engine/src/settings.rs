//! Engine wide settings that are applicable using the [Settings](crate::settings::Settings) struct.

use let_engine_core::backend::{
    Backends,
    audio::{AudioBackend, AudioSettings},
    gpu::GpuBackend,
    networking::NetworkingBackend,
};

use crate::tick_system::TickSettings;

/// The initial settings of this engine.
pub struct EngineSettings<B: Backends> {
    /// Settings that determines the look of the window.
    #[cfg(feature = "client")]
    pub window: crate::window::WindowBuilder,

    /// The initial settings of the tick system.
    pub tick_system: TickSettings,

    pub gpu: <B::Gpu as GpuBackend>::Settings,
    pub audio: AudioSettings<B::Kira>,
    pub networking: <B::Networking as NetworkingBackend>::Settings,
}

impl<B: Backends> Default for EngineSettings<B>
where
    <B::Kira as AudioBackend>::Settings: Default,
{
    fn default() -> Self {
        Self {
            #[cfg(feature = "client")]
            window: crate::window::WindowBuilder::default(),
            tick_system: TickSettings::default(),
            gpu: <B::Gpu as GpuBackend>::Settings::default(),
            audio: AudioSettings::default(),
            networking: <B::Networking as NetworkingBackend>::Settings::default(),
        }
    }
}

impl<B: Backends> Clone for EngineSettings<B>
where
    <B::Kira as AudioBackend>::Settings: Clone,
{
    fn clone(&self) -> Self {
        Self {
            #[cfg(feature = "client")]
            window: self.window.clone(),
            tick_system: self.tick_system.clone(),
            gpu: self.gpu.clone(),
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

    /// Sets the value `gpu` and returns self.
    pub fn gpu(mut self, gpu: <B::Gpu as GpuBackend>::Settings) -> Self {
        self.gpu = gpu;
        self
    }

    /// Sets the value `networking` and returns self.
    pub fn networking(
        mut self,
        networking: <B::Networking as NetworkingBackend>::Settings,
    ) -> Self {
        self.networking = networking;
        self
    }
}
