use std::{
    f64::consts::PI,
    io::Cursor,
    path::Path,
    sync::{Arc, OnceLock},
    thread,
    time::Duration,
};

use crossbeam::channel::{unbounded, Sender};
use glam::{Quat, Vec3};
use kira::{
    manager::{backend::DefaultBackend, AudioManager, AudioManagerSettings},
    sound::{
        static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings},
        FromFileError, IntoOptionalRegion, PlaybackState,
    },
    spatial::{
        emitter::{EmitterHandle, EmitterSettings},
        listener::ListenerSettings,
        scene::SpatialSceneSettings,
    },
    tween::Value,
};

pub(crate) fn audio_server() -> Sender<AudioUpdate> {
    let (send, recv) = unbounded();
    thread::spawn(|| {
        let mut audio_manager =
            AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()).unwrap();
        let mut spacial_scene = audio_manager
            .add_spatial_scene(SpatialSceneSettings::default())
            .unwrap();
        let recv = recv;
        loop {
            let update: AudioUpdate = recv.recv().unwrap();
            match update {
                AudioUpdate::Play(sound) => {
                    let mut emitter = sound.emitter.lock();
                    let mut settings: StaticSoundSettings = sound.settings.into();
                    if let Some(spatial_emitter) = emitter.get() {
                        if sound.object.is_none() {
                            emitter.take();
                        } else {
                            settings = settings.output_destination(spatial_emitter);
                        };
                    }
                    // if the sound contains an object then add a spatial emitter
                    if let (None, Some(object)) = (emitter.get(), &sound.object) {
                        let spatial_emitter = spacial_scene
                            .add_emitter(
                                object.transform.position.extend(0.0),
                                EmitterSettings::default(),
                            )
                            .unwrap();
                        settings = settings.output_destination(&spatial_emitter);
                        let _ = emitter.set(spatial_emitter);
                    }
                    let handle = audio_manager
                        .play(StaticSoundData {
                            sample_rate: sound.data.sample_rate,
                            frames: sound.data.frames,
                            settings,
                        })
                        .unwrap();
                    sound.handle.lock().take();
                    let _ = sound.handle.lock().set(handle);
                }
                AudioUpdate::NewLayer(layer) => {
                    let _ = layer.listener.lock().set(
                        spacial_scene
                            .add_listener(Vec3::ZERO, Quat::IDENTITY, ListenerSettings::default())
                            .unwrap(),
                    );
                }
            };
        }
    });
    send
}

pub(crate) enum AudioUpdate {
    Play(Sound),
    NewLayer(Arc<Layer>),
}

// future wrap
pub use kira::{
    dsp::*,
    sound::{PlaybackRate, Region},
    tween::{Easing, Tween},
    StartTime, Volume,
};
use parking_lot::Mutex;

use crate::objects::{scenes::Layer, Object};

use super::RESOURCES;

/// The shared loaded data of a sound, clone friendly and thread safe.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundData {
    pub sample_rate: u32,
    pub frames: Arc<[Frame]>,
}

impl SoundData {
    /// Loads a sound from a filesystem path.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, FromFileError> {
        let sound_data = StaticSoundData::from_file(path, StaticSoundSettings::default())?;

        Ok(Self {
            sample_rate: sound_data.sample_rate,
            frames: sound_data.frames,
        })
    }

    /// Loads the sound from a Cursor.
    pub fn from_cursor<T: AsRef<[u8]> + Send + Sync + 'static>(
        cursor: Cursor<T>,
    ) -> Result<Self, FromFileError> {
        let sound_data = StaticSoundData::from_cursor(cursor, StaticSoundSettings::default())?;

        Ok(Self {
            sample_rate: sound_data.sample_rate,
            frames: sound_data.frames,
        })
    }

    /// Returns the duration of this sound.
    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.frames.len() as f64 / self.sample_rate as f64)
    }

    /// Generates square wave sound data with length as seconds.
    pub fn gen_square_wave(frequency: f64, length: f64) -> Self {
        let sample_rate = 44100;
        let num_samples = (sample_rate as f64 * length) as usize;

        let period_samples = (sample_rate as f64 / frequency) as usize;

        let mut frames = Vec::with_capacity(num_samples);
        let mut sample_counter = 0;

        for _ in 0..num_samples {
            let value = if sample_counter < period_samples / 2 {
                1.0
            } else {
                -1.0
            };

            let frame = Frame {
                left: value,
                right: value,
            };
            frames.push(frame);

            sample_counter = (sample_counter + 1) % period_samples;
        }
        Self {
            sample_rate,
            frames: Arc::from(frames),
        }
    }
    /// Generates sine wave sound data with length as seconds.
    pub fn gen_sine_wave(frequency: f64, length: f64) -> Self {
        let sample_rate = 44100;
        let num_samples = (sample_rate as f64 * length) as usize;
        let angular_frequency = 2.0 * PI * frequency;

        let mut frames = Vec::with_capacity(num_samples);
        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let value = (angular_frequency as f32 * t).sin();
            let frame = Frame {
                left: value,
                right: value,
            };
            frames.push(frame);
        }
        Self {
            sample_rate,
            frames: Arc::from(frames),
        }
    }
}

pub struct Global;

#[derive(Clone)]
pub struct Sound {
    data: SoundData,
    settings: SoundSettings,
    emitter: Arc<Mutex<OnceLock<EmitterHandle>>>,
    handle: Arc<Mutex<OnceLock<StaticSoundHandle>>>,
    object: Option<Object>,
}

impl Sound {
    /// Makes a new sound with the given settings and data.
    pub fn new(data: SoundData, settings: SoundSettings) -> Self {
        Self {
            data,
            settings,
            emitter: Arc::new(Mutex::new(OnceLock::new())),
            handle: Arc::new(Mutex::new(OnceLock::new())),
            object: None,
        }
    }
    /// Returns the data behind the sound.
    pub fn data(&self) -> &SoundData {
        &self.data
    }
    /// Returns the current playback state of the sound.
    pub fn state(&self) -> PlaybackState {
        if let Some(handle) = self.handle.lock().get() {
            handle.state()
        } else {
            PlaybackState::Stopped
        }
    }
    /// Returns the playback position in seconds.
    pub fn position(&self) -> f64 {
        if let Some(handle) = self.handle.lock().get() {
            handle.position()
        } else {
            0.0
        }
    }
    /// Sets the volume of the sound.
    pub fn set_volume(&mut self, volume: impl Into<Volume>, tween: Tween) {
        let volume = volume.into();
        let value_volume = Value::Fixed(volume);
        self.settings.volume = volume;
        if let Some(handle) = self.handle.lock().get_mut() {
            handle.set_volume(value_volume, tween).unwrap();
        }
    }
    /// Sets the rate, at which the sound is getting played.
    pub fn set_playback_rate(&mut self, playback_rate: impl Into<PlaybackRate>, tween: Tween) {
        let playback_rate = playback_rate.into();
        let value_playback_rate = Value::Fixed(playback_rate);
        self.settings.playback_rate = playback_rate;
        if let Some(handle) = self.handle.lock().get_mut() {
            handle
                .set_playback_rate(value_playback_rate, tween)
                .unwrap();
        }
    }
    /// Sets the panning of the sound, where 0.0 is left and 1.0 is right.
    pub fn set_panning(&mut self, panning: f64, tween: Tween) {
        let value_panning = Value::Fixed(panning);
        self.settings.panning = panning;
        if let Some(handle) = self.handle.lock().get_mut() {
            handle.set_panning(value_panning, tween).unwrap();
        }
    }
    /// Sets the region, where the sound is getting played.
    pub fn set_playback_region(&mut self, playback_region: impl Into<Region>) {
        self.settings.playback_region = playback_region.into();
        if let Some(handle) = self.handle.lock().get_mut() {
            handle
                .set_playback_region(self.settings.playback_region)
                .unwrap();
        }
    }
    /// Sets the optional region, where the sound is getting looped.
    pub fn set_loop_region(&mut self, loop_region: impl IntoOptionalRegion) {
        self.settings.loop_region = loop_region.into_optional_loop_region();
        if let Some(handle) = self.handle.lock().get_mut() {
            handle.set_loop_region(self.settings.loop_region).unwrap();
        }
    }
    /// Binds an object to this sound and plays it where this object is at.
    pub fn bind_to_object(&mut self, object: Option<&Object>) {
        self.object = object.cloned();
    }
    /// Returns the object bound to this sound.
    pub fn get_object(&self) -> Option<&Object> {
        self.object.as_ref()
    }
    /// Updates the position of the sound.
    pub fn update(&mut self) {
        if let (Some(emitter), Some(object)) = (self.emitter.lock().get_mut(), &mut self.object) {
            object.update();
            emitter
                .set_position(object.transform.position.extend(0.0), Tween::default())
                .unwrap()
        }
    }

    /// Plays this sound.
    pub fn play(&mut self) {
        if self.state() != PlaybackState::Playing {
            RESOURCES
                .audio_server
                .send(AudioUpdate::Play(self.clone()))
                .unwrap();
        }
    }
    /// Pauses this sound.
    pub fn pause(&mut self, tween: Tween) {
        if self.state() != PlaybackState::Paused {
            if let Some(handle) = self.handle.lock().get_mut() {
                handle.pause(tween);
            }
        }
    }
    /// Resumes the playback of this sound.
    pub fn resume(&mut self, tween: Tween) {
        if self.state() != PlaybackState::Playing {
            if let Some(handle) = self.handle.lock().get_mut() {
                handle.resume(tween);
            }
        }
    }
    /// Stops this sound.
    pub fn stop(&mut self, tween: Tween) {
        if self.state() != PlaybackState::Stopped {
            if let Some(handle) = self.handle.lock().get_mut() {
                handle.stop(tween);
            }
        }
    }
    /// Sets the playhead to the given position in seconds.
    pub fn seek_to(&mut self, position: f64) {
        if let Some(handle) = self.handle.lock().get_mut() {
            handle.seek_to(position);
        }
    }
    /// Sets the playhead ahead by the given seconds.
    pub fn seek_by(&mut self, position: f64) {
        if let Some(handle) = self.handle.lock().get_mut() {
            handle.seek_by(position);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundSettings {
    /// The portion of the sound that should be played.
    pub playback_region: Region,
    /// The portion of the sound that should be looped.
    pub loop_region: Option<Region>,
    /// Whether the sound should be played in reverse.
    pub reverse: bool,
    /// The volume of the sound.
    pub volume: Volume,
    /// The rate at which the sound should be played.
    ///
    /// Changes both speed and pitch of the sound.
    pub playback_rate: PlaybackRate,
    /// The panning of the sound. 0 is left, 1 is right.
    pub panning: f64,
    /// An optional fade in.
    pub fade_in_tween: Option<Tween>,
}

macro_rules! builder_pattern {
    ($field:ident, $title:expr, $type:ty) => {
        #[doc=concat!("Sets ", $title, " and returns self.")]
        #[inline]
        pub fn $field(mut self, $field: impl Into<$type>) -> Self {
            self.$field = $field.into();
            self
        }
    };
}

impl SoundSettings {
    pub fn new() -> Self {
        let settings = StaticSoundSettings::new();
        let (volume, playback_rate, panning) =
            if let (Value::Fixed(volume), Value::Fixed(playback_rate), Value::Fixed(panning)) =
                (settings.volume, settings.playback_rate, settings.panning)
            {
                (volume, playback_rate, panning)
            } else {
                unreachable!()
            };
        Self {
            playback_region: settings.playback_region,
            loop_region: settings.loop_region,
            reverse: settings.reverse,
            volume,
            playback_rate,
            panning,
            fade_in_tween: settings.fade_in_tween,
        }
    }
    builder_pattern!(playback_region, "the playback region", Region);
    builder_pattern!(loop_region, "the optional loop region", Option<Region>);
    builder_pattern!(reverse, "whether this sound plays reverse", bool);
    builder_pattern!(volume, "the volume", Volume);
    builder_pattern!(playback_rate, "the playback rate", PlaybackRate);
    builder_pattern!(panning, "the panning", f64);
    builder_pattern!(fade_in_tween, "the fade in tween", Option<Tween>);
}

impl From<SoundSettings> for StaticSoundSettings {
    fn from(value: SoundSettings) -> StaticSoundSettings {
        StaticSoundSettings::new()
            .playback_region(value.playback_region)
            .loop_region(value.loop_region)
            .reverse(value.reverse)
            .volume(value.volume)
            .playback_rate(value.playback_rate)
            .panning(value.panning)
            .fade_in_tween(value.fade_in_tween)
    }
}

impl From<StaticSoundSettings> for SoundSettings {
    fn from(value: StaticSoundSettings) -> Self {
        let (volume, playback_rate, panning) =
            if let (Value::Fixed(volume), Value::Fixed(playback_rate), Value::Fixed(panning)) =
                (value.volume, value.playback_rate, value.panning)
            {
                (volume, playback_rate, panning)
            } else {
                unreachable!()
            };
        Self::new()
            .playback_region(value.playback_region)
            .loop_region(value.loop_region)
            .reverse(value.reverse)
            .volume(volume)
            .playback_rate(playback_rate)
            .panning(panning)
            .fade_in_tween(value.fade_in_tween)
    }
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self::new()
    }
}
