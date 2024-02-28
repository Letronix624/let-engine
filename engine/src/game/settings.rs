//! Engine wide settings that are applicable using the [Settings](crate::settings::Settings) struct.

use derive_builder::Builder;

// audio feature
#[cfg(feature = "audio")]
#[cfg(feature = "client")]
use crate::resources::{
    sounds::{AudioSettings, NoAudioServerError},
    RESOURCES,
};

#[cfg(feature = "client")]
use super::{Window, WindowBuilder};

use super::TickSettings;

use parking_lot::{Condvar, Mutex};
#[cfg(feature = "client")]
use std::{
    sync::{atomic::AtomicBool, Arc, OnceLock},
    time::Duration,
};

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
pub struct Settings {
    #[cfg(feature = "client")]
    window: Mutex<std::sync::OnceLock<Arc<Window>>>,
    pub tick_system: TickSystem,
    #[cfg(feature = "client")]
    pub graphics: Graphics,
    #[cfg(feature = "audio")]
    pub audio: Audio,
}

impl Settings {
    pub(crate) fn new() -> Self {
        Self {
            #[cfg(feature = "client")]
            window: Mutex::new(std::sync::OnceLock::new()),
            tick_system: TickSystem::new(),
            #[cfg(feature = "client")]
            graphics: Graphics::new(PresentMode::Fifo),
            #[cfg(feature = "audio")]
            audio: Audio::new(),
        }
    }
    #[cfg(feature = "client")]
    pub(crate) fn set_window(&self, window: Arc<Window>) -> Result<(), Arc<Window>> {
        self.window.lock().set(window)?;
        Ok(())
    }
    #[cfg(feature = "client")]
    /// Returns the window of the game in case it is initialized.
    pub fn window(&self) -> Option<Arc<Window>> {
        self.window.lock().get().cloned()
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

/// Engine wide audio settings.
#[cfg(feature = "audio")]
pub struct Audio {
    audio_settings: Mutex<AudioSettings>,
}

#[cfg(feature = "audio")]
impl Audio {
    pub(crate) fn new() -> Self {
        Self {
            audio_settings: Mutex::new(AudioSettings::new()),
        }
    }
    /// Returns the audio settings.
    #[cfg(feature = "audio")]
    pub fn get(&self) -> AudioSettings {
        *self.audio_settings.lock()
    }
    /// Sets and applies the audio settings and therefore refreshes the engine side audio server to use them.
    #[cfg(feature = "audio")]
    pub fn set(&self, settings: AudioSettings) -> Result<(), NoAudioServerError> {
        *self.audio_settings.lock() = settings;
        RESOURCES
            .audio_server
            .send(crate::resources::sounds::AudioUpdate::SettingsChange(
                settings,
            ))
            .ok()
            .ok_or(NoAudioServerError)
    }
}

/// Engine wide Graphics settings.
///
/// By default the present mode is determined by this order based on availability on the device:
///
/// 1. `Mailbox`
/// 2. `Immediate`
/// 3. `Fifo`
///
/// The framerate limit is `None`, so off.
///
/// Only alter settings after the game engine has been initialized. The initialisation of the game engine also
/// initializes the settings.
#[cfg(feature = "client")]
pub struct Graphics {
    /// An option that determines something called "VSync".
    pub(crate) present_mode: Mutex<PresentMode>,
    /// Time waited before each frame.
    framerate_limit: Mutex<Duration>,
    pub(crate) available_present_modes: OnceLock<Vec<PresentMode>>,
    pub(crate) recreate_swapchain: AtomicBool,
}

#[cfg(feature = "client")]
impl Graphics {
    pub(crate) fn new(present_mode: PresentMode) -> Self {
        Self {
            present_mode: Mutex::new(present_mode),
            framerate_limit: Mutex::new(Duration::from_secs(0)),
            available_present_modes: OnceLock::new(),
            recreate_swapchain: false.into(),
        }
    }

    /// Returns the present mode of the game.
    pub fn present_mode(&self) -> PresentMode {
        *self.present_mode.lock()
    }

    /// Sets and applies the present mode of the game.
    ///
    /// Returns an error in case the present mode given is not supported by the device.
    ///
    /// Find out which present modes work using the [get_supported_present_modes](Graphics::get_supported_present_modes) function.
    pub fn set_present_mode(&self, mode: PresentMode) -> anyhow::Result<()> {
        if self.get_supported_present_modes().contains(&mode) {
            *self.present_mode.lock() = mode;
            self.recreate_swapchain
                .store(true, std::sync::atomic::Ordering::Release);
            Ok(())
        } else {
            Err(anyhow::Error::msg(format!(
                "This present mode \"{:?}\" is not available on this device.\nAvailable modes on this device are {:?}",
                mode, self.get_supported_present_modes()
            )))
        }
    }

    /// Returns waiting time between frames to wait.
    pub fn framerate_limit(&self) -> Duration {
        *self.framerate_limit.lock()
    }

    /// Sets the framerate limit as waiting time between frames.
    ///
    /// This should be able to be changed by the user in case they have a device with limited power capacity like a laptop with a battery.
    ///
    /// Setting the duration to no wait time at all will turn off the limit.
    pub fn set_framerate_limit(&self, limit: Duration) {
        *self.framerate_limit.lock() = limit;
    }

    /// Sets the cap for the max frames per second the game should be able to output.
    ///
    /// This method is the same as setting the `set_framerate_limit` of this setting to `1.0 / cap` in seconds.
    ///
    /// Turns off the framerate cap if 0 was given.
    pub fn set_fps_cap(&self, cap: u64) {
        if cap == 0 {
            self.set_framerate_limit(Duration::from_secs(cap));
            return;
        }
        self.set_framerate_limit(Duration::from_secs_f64(1.0 / cap as f64));
    }

    /// Returns all the present modes this device supports.
    ///
    /// If the vec is empty the engine has not been initialized and the settings should not be changed at this state.
    pub fn get_supported_present_modes(&self) -> Vec<PresentMode> {
        self.available_present_modes
            .get()
            .cloned()
            .unwrap_or(vec![])
    }
}

/// The presentation action to take when presenting images to the window.
///
/// In game engine terms this affects "VSync".
///
/// `Immediate` mode is the only one that does not have "VSync".
///
/// When designing in game graphics settings this is the setting that gets changed when users select the VSync option.
///
/// The vsync options may include higher latency than the other ones.
///
/// It is not recommended dynamically switching between those during the game, as they may cause visual artifacts or noticable changes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
#[cfg(feature = "client")]
pub enum PresentMode {
    /// This one has no vsync and presents the image as soon as it is available.
    ///
    /// This may happen while the image is presenting, so it may cause tearing.
    ///
    /// This present mode has the lowest latency compared to every other mode, so this is the option for most fast paced games where latency matters.
    ///
    /// This present mode may not be available on every device.
    Immediate,
    /// This present mode has a waiting slot for the next image to be presented after the current one has finished presenting.
    /// This mode also does not block the drawing thread, drawing images, even when they will not get presented.
    ///
    /// This means there is no tearing and with just one waiting slot also not that much latency.
    ///
    /// This option is recommended if `Immediate` is not available and also for games that focus visual experience over latency, as this one does not have tearing.
    ///
    /// It may also not be available on every device.
    Mailbox,
    /// Means first in first out.
    ///
    /// This present mode is also known as "vsync on". It blocks the thread and only draws and presents images if the present buffer is finished drawing to the screen.
    ///
    /// It is guaranteed to be available on every device.
    Fifo,
}

#[cfg(feature = "client")]
impl From<PresentMode> for vulkano::swapchain::PresentMode {
    fn from(value: PresentMode) -> vulkano::swapchain::PresentMode {
        use vulkano::swapchain::PresentMode as Pm;
        match value {
            PresentMode::Immediate => Pm::Immediate,
            PresentMode::Mailbox => Pm::Mailbox,
            PresentMode::Fifo => Pm::Fifo,
        }
    }
}

#[cfg(feature = "client")]
impl From<vulkano::swapchain::PresentMode> for PresentMode {
    fn from(value: vulkano::swapchain::PresentMode) -> PresentMode {
        use vulkano::swapchain::PresentMode as Pm;
        match value {
            Pm::Immediate => PresentMode::Immediate,
            Pm::Mailbox => PresentMode::Mailbox,
            _ => PresentMode::Fifo,
        }
    }
}
