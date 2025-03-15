//! Everything about playing audio in the game engine.

use std::{
    f64::consts::PI,
    io::Cursor,
    path::Path,
    sync::{Arc, LazyLock, OnceLock},
    thread,
    time::Duration,
};

use anyhow::Result;
use crossbeam::channel::{unbounded, Sender};
use glam::{Quat, Vec3};
use kira::{
    manager::{backend::DefaultBackend, AudioManager, AudioManagerSettings, Capacities},
    sound::{
        static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings},
        FromFileError,
    },
    spatial::{
        emitter::{EmitterHandle, EmitterSettings},
        listener::{ListenerHandle, ListenerSettings},
        scene::SpatialSceneSettings,
    },
    tween::Value,
};

static AUDIO_SERVER: LazyLock<Sender<AudioUpdate>> = LazyLock::new(audio_server);

/// The audio server has not started.
#[derive(Clone, Copy, Debug, Error)]
#[error("The audio server is not started for this session.")]
pub struct NoAudioServerError;

fn audio_server() -> Sender<AudioUpdate> {
    let (send, recv) = unbounded();
    thread::spawn(|| {
        let recv = recv;

        let (manager_settings, scene_settings) = (
            AudioManagerSettings::default(),
            SpatialSceneSettings::default(),
        ); //SETTINGS.audio.get().make();

        let mut audio_manager = AudioManager::<DefaultBackend>::new(manager_settings);
        if let Ok(audio_manager) = audio_manager.as_mut() {
            let mut spacial_scene = audio_manager
                .add_spatial_scene(scene_settings)
                .expect("impossible");
            loop {
                match recv.recv() {
                    Ok(AudioUpdate::Play(sound)) => {
                        let mut emitter = sound.emitter.lock();
                        let mut settings: StaticSoundSettings = sound.settings.into();
                        if let Some(spatial_emitter) = emitter.get() {
                            // remove the emitter in case the object was removed.
                            if sound.object.is_none() {
                                emitter.take();
                            } else {
                                settings = settings.output_destination(spatial_emitter);
                            };
                        }
                        // if the sound contains an object then add a spatial emitter
                        if let (None, Some(object)) = (emitter.get(), &sound.object) {
                            if let Ok(spatial_emitter) = spacial_scene.add_emitter(
                                object.transform.position.extend(0.0),
                                sound.spatial_settings().into(),
                            ) {
                                settings = settings.output_destination(&spatial_emitter);
                                let _ = emitter.set(spatial_emitter);
                            }
                        }
                        let handle = audio_manager.play(StaticSoundData {
                            sample_rate: sound.data.sample_rate,
                            frames: sound.data.frames,
                            settings,
                            slice: sound.data.slice,
                        });
                        sound.handle.lock().take();
                        let _ = sound.handle.lock().set(handle.map_err(|x| x.into()));
                    }
                    Ok(AudioUpdate::NewListener(sender)) => {
                        if let Ok(listener) = spacial_scene.add_listener(
                            Vec3::ZERO,
                            Quat::IDENTITY,
                            ListenerSettings::default(),
                        ) {
                            let _ = sender.send(listener);
                        };
                    }
                    Ok(AudioUpdate::SettingsChange(settings)) => {
                        let (manager_settings, scene_settings) = settings.make();
                        if let Ok(mut manager) =
                            AudioManager::<DefaultBackend>::new(manager_settings)
                        {
                            spacial_scene = manager
                                .add_spatial_scene(scene_settings)
                                .expect("unreachable");
                            *audio_manager = manager;
                        } else {
                            break;
                        };
                    }
                    _ => (),
                };
            }
        }
    });
    send
}

pub enum AudioUpdate {
    Play(Sound),
    NewListener(Sender<ListenerHandle>),
    SettingsChange(AudioSettings),
}

pub use kira::{
    sound::{
        EndPosition, IntoOptionalRegion, PlaybackPosition, PlaybackRate, PlaybackState, Region,
    },
    spatial::emitter::EmitterDistances as Distances,
    tween::Easing,
    Frame, Volume,
};

/// "Inbetween"
///
/// Describes an interpolation between values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween {
    /// The duration of the interpolation.
    pub duration: Duration,
    /// The easing method used.
    pub easing: Easing,
}

impl Default for Tween {
    fn default() -> Self {
        kira::tween::Tween::default().into()
    }
}

impl From<kira::tween::Tween> for Tween {
    fn from(value: kira::tween::Tween) -> Self {
        Self {
            duration: value.duration,
            easing: value.easing,
        }
    }
}
impl From<Tween> for kira::tween::Tween {
    fn from(value: Tween) -> Self {
        Self {
            duration: value.duration,
            easing: value.easing,
            start_time: kira::StartTime::Immediate,
        }
    }
}

/// The global audio settings that should be used throughout the game.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct AudioSettings {
    /// The limit of how many sounds can exist at the same time.
    pub sound_capacity: u16,
    /// The limit of how many sounds can be bound to objects to make them spatial.
    pub object_bound_sound_capacity: u16,
    /// The limit of how many scenes can play spatial sounds.
    pub spatial_scene_capacity: u16,
}

impl AudioSettings {
    /// Makes default audio settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum amount of sounds.
    pub fn set_sound_capacities(&mut self, sound_capacity: u16) {
        self.sound_capacity = sound_capacity;
    }

    /// Sets the maximum amount of sounds and returns self.
    pub fn sound_capacity(mut self, sound_capacity: u16) -> Self {
        self.sound_capacity = sound_capacity;
        self
    }

    /// Sets the maximum amount of sounds you can bind on objects to make them spatial.
    pub fn set_object_bound_sound_capacity(&mut self, sound_capacity: u16) {
        self.object_bound_sound_capacity = sound_capacity;
    }

    /// Sets the maximum amount of sounds you can bind on objects to make them spatial and returns self.
    pub fn object_bound_sound_capacity(mut self, sound_capacity: u16) -> Self {
        self.object_bound_sound_capacity = sound_capacity;
        self
    }

    /// Sets the maximum amount of scenes that can play spatial sounds.
    pub fn set_spatial_scene_capacity(&mut self, scene_capacity: u16) {
        self.spatial_scene_capacity = scene_capacity;
    }

    /// Sets the maximum amount of scenes that can play spatial sounds and returns self.
    pub fn spatial_scene_capacity(mut self, scene_capacity: u16) -> Self {
        self.spatial_scene_capacity = scene_capacity;
        self
    }

    /// Converts these audio settings to the kira settings to be used when making or editing the settings.
    pub(crate) fn make(&self) -> (AudioManagerSettings<DefaultBackend>, SpatialSceneSettings) {
        let manager_settings = AudioManagerSettings {
            capacities: Capacities {
                command_capacity: 256,
                sound_capacity: self.sound_capacity,
                clock_capacity: 1,
                spatial_scene_capacity: self.spatial_scene_capacity,
                ..Default::default()
            },
            ..Default::default()
        };
        let scene_settings = SpatialSceneSettings::new()
            .emitter_capacity(self.object_bound_sound_capacity)
            .listener_capacity(self.spatial_scene_capacity);

        (manager_settings, scene_settings)
    }
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            sound_capacity: 256,
            object_bound_sound_capacity: 256,
            spatial_scene_capacity: 8,
        }
    }
}

use parking_lot::Mutex;
use thiserror::Error;

use let_engine_core::objects::Object;

/// The shared loaded data of a sound, clone friendly and thread safe.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundData {
    pub sample_rate: u32,
    pub frames: Arc<[Frame]>,
    pub slice: Option<(usize, usize)>,
}

impl SoundData {
    /// Loads a sound from a filesystem path.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, FromFileError> {
        let sound_data = StaticSoundData::from_file(path)?;

        Ok(Self {
            sample_rate: sound_data.sample_rate,
            frames: sound_data.frames,
            slice: None,
        })
    }

    /// Loads the sound from a Cursor.
    pub fn from_cursor<T: AsRef<[u8]> + Send + Sync + 'static>(
        cursor: Cursor<T>,
    ) -> Result<Self, FromFileError> {
        let sound_data = StaticSoundData::from_cursor(cursor)?;

        Ok(Self {
            sample_rate: sound_data.sample_rate,
            frames: sound_data.frames,
            slice: None,
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
            slice: None,
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
            slice: None,
        }
    }
}

/// The settings of a sound with an object bound to it.
#[derive(Clone, Copy, Debug)]
pub struct SpatialSettings {
    /// The distances where the sound appears the loudest and where it appears the quietest.
    ///
    /// By default it goes from 1 to 100.
    pub distances: Distances,
    /// How the volume will change with distance.
    ///
    /// On `None` the output volume will be constant.
    pub attenuation_function: Option<Easing>,
    /// On `true` sounds from the left will pan to the left and sounds from the right will pan right.
    pub spatialization: bool,
}

impl SpatialSettings {
    pub fn new() -> Self {
        Self::from(EmitterSettings::new())
    }
}

impl Default for SpatialSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl From<EmitterSettings> for SpatialSettings {
    fn from(value: EmitterSettings) -> Self {
        Self {
            distances: value.distances,
            attenuation_function: value.attenuation_function,
            spatialization: value.enable_spatialization,
        }
    }
}
impl From<SpatialSettings> for EmitterSettings {
    fn from(value: SpatialSettings) -> Self {
        Self::default()
            .distances(value.distances)
            .attenuation_function(value.attenuation_function)
            .enable_spatialization(value.spatialization)
    }
}

/// A playable sound.
///
/// You can bind an object to this sound making it directional.
#[derive(Clone)]
pub struct Sound {
    data: SoundData,
    settings: SoundSettings,
    spatial_settings: SpatialSettings,
    emitter: Arc<Mutex<OnceLock<EmitterHandle>>>,
    handle: Arc<Mutex<OnceLock<Result<StaticSoundHandle>>>>,
    object: Option<Object>,
}

impl Sound {
    /// Makes a new sound with the given settings and data.
    pub fn new(data: SoundData, settings: SoundSettings) -> Self {
        Self {
            data,
            settings,
            spatial_settings: SpatialSettings::new(),
            emitter: Arc::new(Mutex::new(OnceLock::new())),
            handle: Arc::new(Mutex::new(OnceLock::new())),
            object: None,
        }
    }

    /// Sets the settings of this sound.
    pub fn set_settings(&mut self, settings: SoundSettings) {
        self.settings = settings;
    }

    /// Returns the settings of this sounds.
    pub fn settings(&self) -> SoundSettings {
        self.settings
    }

    /// Sets the spatial settings of this sound.
    ///
    /// Spatial settings are applied with the `play` function.
    pub fn set_spatial_settings(&mut self, settings: SpatialSettings) {
        self.spatial_settings = settings;
    }

    /// Returns the spatial settings of this sounds.
    pub fn spatial_settings(&self) -> SpatialSettings {
        self.spatial_settings
    }

    /// Returns the data behind the sound.
    pub fn data(&self) -> &SoundData {
        &self.data
    }

    /// Returns the current playback state of the sound.
    pub fn state(&self) -> PlaybackState {
        if let Some(Ok(handle)) = self.handle.lock().get() {
            handle.state()
        } else {
            PlaybackState::Stopped
        }
    }

    /// Returns the playback position in seconds.
    pub fn position(&self) -> f64 {
        if let Some(Ok(handle)) = self.handle.lock().get() {
            handle.position()
        } else {
            0.0
        }
    }

    /// Sets the volume of the sound.
    ///
    /// Returns an error in case the command queue is full.
    pub fn set_volume(&mut self, volume: impl Into<Volume>, tween: Tween) {
        let volume = volume.into();
        let value_volume = Value::Fixed(volume);
        self.settings.volume = volume;
        if let Some(Ok(handle)) = self.handle.lock().get_mut() {
            handle.set_volume(value_volume, tween.into());
        }
    }
    /// Sets the rate, at which the sound is getting played.
    ///
    /// Returns an error in case the command queue is full.
    pub fn set_playback_rate(&mut self, playback_rate: impl Into<PlaybackRate>, tween: Tween) {
        let playback_rate = playback_rate.into();
        let value_playback_rate = Value::Fixed(playback_rate);
        self.settings.playback_rate = playback_rate;
        if let Some(Ok(handle)) = self.handle.lock().get_mut() {
            handle.set_playback_rate(value_playback_rate, tween.into());
        }
    }

    /// Sets the panning of the sound, where 0.0 is left and 1.0 is right.
    ///
    /// Returns an error in case the command queue is full.
    pub fn set_panning(&mut self, panning: f64, tween: Tween) {
        let value_panning = Value::Fixed(panning);
        self.settings.panning = panning;
        if let Some(Ok(handle)) = self.handle.lock().get_mut() {
            handle.set_panning(value_panning, tween.into());
        }
    }

    /// Sets the optional region, where the sound is getting looped.
    ///
    /// Returns an error in case the command queue is full.
    pub fn set_loop_region(&mut self, loop_region: impl IntoOptionalRegion) {
        self.settings.loop_region = loop_region.into_optional_region();
        if let Some(Ok(handle)) = self.handle.lock().get_mut() {
            handle.set_loop_region(self.settings.loop_region);
        }
    }

    /// Binds an object to this sound and plays it where this object is located at.
    pub fn bind_to_object(&mut self, object: Option<&Object>) {
        self.object = object.cloned();
    }

    /// Returns the object bound to this sound.
    pub fn object(&self) -> Option<&Object> {
        self.object.as_ref()
    }

    /// Updates the position of the sound.
    ///
    /// Returns an error in case the command queue is full.
    pub fn update(&mut self, tween: Tween) -> Result<()> {
        if let (Some(emitter), Some(object)) = (self.emitter.lock().get_mut(), &mut self.object) {
            object.update()?;
            emitter.set_position(object.transform.position.extend(0.0), tween.into())
        }
        Ok(())
    }

    /// Plays this sound.
    pub fn play(&mut self) -> Result<()> {
        if self.state() != PlaybackState::Playing {
            AUDIO_SERVER
                .send(AudioUpdate::Play(self.clone()))
                .ok()
                .ok_or(NoAudioServerError)?;
        }
        Ok(())
    }

    /// Pauses this sound.
    ///
    /// Returns an error in case the command queue is full.
    pub fn pause(&mut self, tween: Tween) {
        if self.state() != PlaybackState::Paused {
            if let Some(Ok(handle)) = self.handle.lock().get_mut() {
                handle.pause(tween.into());
            }
        }
    }

    /// Resumes the playback of this sound.
    ///
    /// Returns an error in case the command queue is full.
    pub fn resume(&mut self, tween: Tween) {
        if self.state() != PlaybackState::Playing {
            if let Some(Ok(handle)) = self.handle.lock().get_mut() {
                handle.resume(tween.into());
            }
        }
    }

    /// Stops this sound.
    ///
    /// Returns an error in case the command queue is full.
    pub fn stop(&mut self, tween: Tween) {
        if self.state() != PlaybackState::Stopped {
            if let Some(Ok(handle)) = self.handle.lock().get_mut() {
                handle.stop(tween.into());
            }
        }
    }

    /// Sets the playhead to the given position in seconds.
    ///
    /// Returns an error in case the command queue is full.
    pub fn seek_to(&mut self, position: f64) {
        if let Some(Ok(handle)) = self.handle.lock().get_mut() {
            handle.seek_to(position);
        }
    }

    /// Sets the playhead ahead by the given seconds.
    ///
    /// Returns an error in case the command queue is full.
    pub fn seek_by(&mut self, position: f64) {
        if let Some(Ok(handle)) = self.handle.lock().get_mut() {
            handle.seek_by(position);
        }
    }
}

/// Settings that determine the appearance of the sound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundSettings {
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
            loop_region: settings.loop_region,
            reverse: settings.reverse,
            volume,
            playback_rate,
            panning,
            fade_in_tween: settings.fade_in_tween.map(Tween::from),
        }
    }
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
            .loop_region(value.loop_region)
            .reverse(value.reverse)
            .volume(value.volume)
            .playback_rate(value.playback_rate)
            .panning(value.panning)
            .fade_in_tween(value.fade_in_tween.map(kira::tween::Tween::from))
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
            .loop_region(value.loop_region)
            .reverse(value.reverse)
            .volume(volume)
            .playback_rate(playback_rate)
            .panning(panning)
            .fade_in_tween(value.fade_in_tween.map(Tween::from))
    }
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self::new()
    }
}

/// Engine wide audio settings.
#[derive(Default)]
pub struct Audio {
    audio_settings: Mutex<AudioSettings>,
}

impl Audio {
    /// Returns the audio settings.
    pub fn get(&self) -> AudioSettings {
        *self.audio_settings.lock()
    }
    /// Sets and applies the audio settings and therefore refreshes the engine side audio server to use them.
    pub fn set(&self, settings: AudioSettings) -> Result<(), NoAudioServerError> {
        *self.audio_settings.lock() = settings;
        AUDIO_SERVER
            .send(AudioUpdate::SettingsChange(settings))
            .ok()
            .ok_or(NoAudioServerError)
    }
}

/// Your "ears". The object this is bound to represents the position and orientation of where the sound is to be heard.
///
/// Just the existence of this object is enough for you to be able to hear sounds directionally from the position of this listener.
pub struct Listener {
    listener: ListenerHandle,
    object: Object,
}

impl Listener {
    /// Creates a new Listener using the given object as ears.
    pub fn new(object: &Object) -> Result<Self> {
        let (sender, recv) = unbounded();
        AUDIO_SERVER.send(AudioUpdate::NewListener(sender))?;
        Ok(Self {
            object: object.clone(),
            listener: recv.recv()?,
        })
    }

    /// Returns the object bound to this listener.
    pub fn object(&self) -> &Object {
        &self.object
    }

    /// Updates the listener to the object it is bound to.
    pub fn update(&mut self, tween: Tween) -> Result<()> {
        self.object.update()?;
        self.listener
            .set_position(self.object.transform.position.extend(0.0), tween.into());
        self.listener.set_orientation(
            Quat::from_rotation_z(self.object.transform.rotation),
            tween.into(),
        );
        Ok(())
    }
}
