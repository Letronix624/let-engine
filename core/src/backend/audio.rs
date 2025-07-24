/// The audio backend using Kira.
use std::sync::Arc;

use glam::{Quat, Vec3};
pub use kira::{
    backend::{mock::MockBackend, Backend as AudioBackend, DefaultBackend as DefaultAudioBackend},
    Capacities, PlaySoundError,
};
use kira::{
    sound::SoundData, track::MainTrackBuilder, AudioManager, AudioManagerSettings, Decibels,
    ResourceLimitReached, Value,
};
use parking_lot::Mutex;
use thiserror::Error;

#[derive(Error)]
#[error("{0}")]
pub struct AudioBackendError<B: std::fmt::Debug>(B);

impl<B: std::fmt::Debug> std::fmt::Debug for AudioBackendError<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

pub struct AudioInterface<B: AudioBackend> {
    manager: Arc<Mutex<AudioManager<B>>>,
}

impl<B: AudioBackend> Clone for AudioInterface<B> {
    fn clone(&self) -> Self {
        Self {
            manager: self.manager.clone(),
        }
    }
}

impl<B: AudioBackend> AudioInterface<B>
where
    B::Settings: Default,
    B::Error: std::fmt::Debug,
{
    /// Creates a new audio interface. This is only meant for engine implementations.
    #[doc(hidden)]
    pub fn new(settings: &AudioSettings) -> Result<Self, AudioBackendError<B::Error>> {
        let manager_settings = AudioManagerSettings {
            capacities: Capacities {
                sub_track_capacity: settings.sub_track_capacity,
                send_track_capacity: settings.send_track_capacity,
                clock_capacity: settings.clock_capacity,
                modulator_capacity: settings.modulator_capacity,
                listener_capacity: settings.listener_capacity,
            },
            main_track_builder: MainTrackBuilder::new()
                .volume(settings.initial_volume)
                .sound_capacity(settings.sound_capacity),
            internal_buffer_size: settings.internal_buffer_size,
            ..AudioManagerSettings::default()
        };

        let manager = AudioManager::new(manager_settings).map_err(AudioBackendError)?;

        Ok(Self {
            manager: Arc::new(Mutex::new(manager)),
        })
    }

    /// Restarts the audio system with the new given settings.
    pub fn restart(&self, settings: &AudioSettings) -> Result<(), AudioBackendError<B::Error>> {
        let manager_settings = AudioManagerSettings {
            capacities: Capacities {
                sub_track_capacity: settings.sub_track_capacity,
                send_track_capacity: settings.send_track_capacity,
                clock_capacity: settings.clock_capacity,
                modulator_capacity: settings.modulator_capacity,
                listener_capacity: settings.listener_capacity,
            },
            main_track_builder: MainTrackBuilder::new()
                .volume(settings.initial_volume)
                .sound_capacity(settings.sound_capacity),
            internal_buffer_size: settings.internal_buffer_size,
            ..AudioManagerSettings::default()
        };

        let manager = AudioManager::new(manager_settings).map_err(AudioBackendError)?;

        *self.manager.lock() = manager;

        Ok(())
    }

    /// Directly plays a sound.
    #[inline]
    pub fn play<D: SoundData>(&self, sound_data: D) -> Result<D::Handle, PlaySoundError<D::Error>> {
        let mut manager = self.manager.lock();
        manager.play(sound_data)
    }

    /// Sets the master volume.
    #[inline]
    pub fn set_volume(&self, volume: impl Into<Value<Decibels>>, tween: kira::Tween) {
        let mut manager = self.manager.lock();
        manager.main_track().set_volume(volume, tween)
    }

    /// Creates a mixer sub-track.
    #[inline]
    pub fn add_sub_track(
        &self,
        builder: kira::track::TrackBuilder,
    ) -> Result<kira::track::TrackHandle, ResourceLimitReached> {
        let mut manager = self.manager.lock();
        manager.add_sub_track(builder)
    }

    /// Add a spatial mixer track.
    #[inline]
    pub fn add_spatial_sub_track(
        &self,
        listener: impl Into<kira::listener::ListenerId>,
        position: Value<Vec3>,
        builder: kira::track::SpatialTrackBuilder,
    ) -> Result<kira::track::SpatialTrackHandle, ResourceLimitReached> {
        let mut manager = self.manager.lock();

        manager.add_spatial_sub_track(listener, position.to_(), builder)
    }

    /// Creates a mixer send track.
    #[inline]
    pub fn add_send_track(
        &self,
        builder: kira::track::SendTrackBuilder,
    ) -> Result<kira::track::SendTrackHandle, ResourceLimitReached> {
        let mut manager = self.manager.lock();
        manager.add_send_track(builder)
    }

    /// Creates a clock.
    #[inline]
    pub fn add_clock(
        &self,
        speed: impl Into<Value<kira::clock::ClockSpeed>>,
    ) -> Result<kira::clock::ClockHandle, ResourceLimitReached> {
        let mut manager = self.manager.lock();
        manager.add_clock(speed)
    }

    /// Creates a modulator.
    #[inline]
    pub fn add_modulator<Builder: kira::modulator::ModulatorBuilder>(
        &self,
        builder: Builder,
    ) -> Result<Builder::Handle, ResourceLimitReached> {
        let mut manager = self.manager.lock();
        manager.add_modulator(builder)
    }

    /// Creates a listener.
    #[inline]
    pub fn add_listener(
        &self,
        position: Value<Vec3>,
        orientation: Value<Quat>,
    ) -> Result<kira::listener::ListenerHandle, ResourceLimitReached> {
        let mut manager = self.manager.lock();

        manager.add_listener(position.to_(), orientation.to_())
    }

    /// Returns the maximum number of sounds that can play simultaneously.
    #[inline]
    pub fn sound_capacity(&self) -> usize {
        let mut manager = self.manager.lock();
        manager.main_track().sound_capacity()
    }

    /// Returns the number of mixer sub-tracks that can exist at a time.
    #[inline]
    pub fn sub_track_capacity(&self) -> usize {
        let manager = self.manager.lock();
        manager.sub_track_capacity()
    }

    /// Returns the number of mixer send tracks that can exist at a time.
    #[inline]
    pub fn send_track_capacity(&self) -> usize {
        let manager = self.manager.lock();
        manager.send_track_capacity()
    }

    /// Returns the number of clocks that can exist at a time.
    #[inline]
    pub fn clock_capacity(&self) -> usize {
        let manager = self.manager.lock();
        manager.clock_capacity()
    }

    /// Returns the number of modulators that can exist at a time.
    #[inline]
    pub fn modulator_capacity(&self) -> usize {
        let manager = self.manager.lock();
        manager.modulator_capacity()
    }

    /// Returns the number of currently playing sounds.
    #[inline]
    pub fn num_sounds(&self) -> usize {
        let mut manager = self.manager.lock();
        manager.main_track().num_sounds()
    }

    /// Returns the number of mixer sub-tracks that currently exist.
    #[inline]
    pub fn num_sub_tracks(&self) -> usize {
        let manager = self.manager.lock();
        manager.num_sub_tracks()
    }

    /// Returns the number of mixer send tracks that currently exist.
    #[inline]
    pub fn num_send_tracks(&self) -> usize {
        let manager = self.manager.lock();
        manager.num_send_tracks()
    }

    /// Returns the number of clocks that currently exist.
    #[inline]
    pub fn num_clocks(&self) -> usize {
        let manager = self.manager.lock();
        manager.num_clocks()
    }

    /// Returns the number of modulators that currently exist.
    #[inline]
    pub fn num_modulators(&self) -> usize {
        let manager = self.manager.lock();
        manager.num_modulators()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioSettings {
    pub sub_track_capacity: usize,

    pub send_track_capacity: usize,

    pub clock_capacity: usize,

    pub modulator_capacity: usize,

    pub listener_capacity: usize,

    pub sound_capacity: usize,

    pub initial_volume: Value<Decibels>,

    /// Determines how often modulators and clocks will be updated (in samples).
    ///
    /// At the default size of 128 samples, at a sample rate of 44100hz,
    /// modulators and clocks will update about every 3 milliseconds.
    ///
    /// Decreasing this value increases the precision of clocks and modulators
    /// at the expense of higher CPU usage.
    pub internal_buffer_size: usize,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            sub_track_capacity: 128,
            send_track_capacity: 16,
            clock_capacity: 8,
            modulator_capacity: 16,
            listener_capacity: 8,
            sound_capacity: 256,
            initial_volume: Value::Fixed(Decibels(0.0)),
            internal_buffer_size: 128,
        }
    }
}
