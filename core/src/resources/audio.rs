/// Exports of Kira objects for audio capabilities.
use std::{sync::Arc, time::Duration};

use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
pub use kira::{
    Decibels, Easing, Frame, Mapping, Mix, Panning, PlaybackRate, Semitones, StartTime, Tween,
    Tweenable, Value, clock, command, command_writers_and_readers, effect, info, interpolate_frame,
    listener, modulator, sound, track,
};

/// Generates square wave sound data with length as seconds.
pub fn gen_square_wave(
    frequency: f64,
    duration: Duration,
    settings: StaticSoundSettings,
) -> StaticSoundData {
    let sample_rate: u32 = 44100;
    let num_samples = (sample_rate as f64 * duration.as_secs_f64()) as usize;

    let period_samples = (sample_rate as f64 / frequency) as usize;

    let mut frames = Vec::with_capacity(num_samples);
    let mut sample_counter = 0;

    for _ in 0..num_samples {
        let value = if sample_counter < period_samples / 2 {
            1.0
        } else {
            -1.0
        };

        let frame = Frame::from_mono(value);
        frames.push(frame);

        sample_counter = (sample_counter + 1) % period_samples;
    }

    StaticSoundData {
        sample_rate,
        frames: Arc::from(frames),
        settings,
        slice: None,
    }
}

/// Generates sine wave sound data with length as seconds.
pub fn gen_sine_wave(
    frequency: f64,
    duration: Duration,
    settings: StaticSoundSettings,
) -> StaticSoundData {
    let sample_rate: u32 = 44100;
    let num_samples = (sample_rate as f64 * duration.as_secs_f64()) as usize;
    let angular_frequency = 2.0 * std::f64::consts::PI * frequency;

    let mut frames = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let value = (angular_frequency as f32 * t).sin();
        let frame = Frame::from_mono(value);
        frames.push(frame);
    }

    StaticSoundData {
        sample_rate,
        frames: Arc::from(frames),
        settings,
        slice: None,
    }
}
