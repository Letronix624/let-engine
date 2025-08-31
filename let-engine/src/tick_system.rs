use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crossbeam::atomic::AtomicCell;
use derive_builder::Builder;
use let_engine_core::backend::Backends;
use parking_lot::Mutex;

use crate::{Game, GameWrapper};

pub(crate) struct TickSystem {
    settings: Mutex<TickSettings>,
    report: AtomicCell<Tick>,
    tick_pause_lock: (parking_lot::Mutex<bool>, parking_lot::Condvar),
}

impl TickSystem {
    pub(crate) fn new(settings: TickSettings) -> Self {
        Self {
            settings: Mutex::new(settings),
            report: AtomicCell::new(Tick::default()),
            tick_pause_lock: (parking_lot::Mutex::new(false), parking_lot::Condvar::new()),
        }
    }
}

/// Runs the games `tick` function after every iteration.
pub(super) fn run<G: Game<B>, B: Backends>(game: Arc<GameWrapper<G, B>>) {
    let mut index: usize = 0;

    loop {
        #[allow(unused_mut)]
        let mut settings: TickSettings = {
            let interface = &game.backends.tick_system;

            // wait if paused
            interface
                .tick_pause_lock
                .1
                .wait_while(&mut interface.tick_pause_lock.0.lock(), |x| *x);
            interface.settings.lock().clone()
        };

        // capture tick start time.
        let start_time = Instant::now();

        // Run the logic
        game.tick();

        // update the physics in case they are active in the tick settings.
        #[cfg(feature = "physics")]
        if game
            .scene
            .lock()
            .physics_iteration(settings.update_physics)
            .is_err()
        {
            // Disable physics updating if it fails. Return running this tick system.
            settings.update_physics = false;
        };

        // record the elapsed time.
        let elapsed_time = start_time.elapsed();

        // Lock the thread in case the time scale is 0.
        if game.time.scale() == 0.0 {
            let mut guard = game.time.zero_cvar.0.lock();
            game.time.zero_cvar.1.wait(&mut guard);
        }

        let tick_wait = if settings.time_scale_influence {
            // Multiply the waiting duration with the inverse time scale.
            settings.tick_wait.mul_f64(1.0 / game.time.scale())
        } else {
            settings.tick_wait
        };

        // calculate waiting time
        // ((1.0 / time_scale) * tick_wait) - elapsed_time
        let waiting_time = if let TimeStep::Variable = settings.timestep_mode {
            // Subtract the tick logic execution time from the waiting time to make the waiting time between ticks more consistent.
            tick_wait.saturating_sub(elapsed_time)
        } else {
            tick_wait
        };

        // Spin sleep so windows users with their lower quality sleep functions get the same sleep duration
        spin_sleep::sleep(waiting_time);

        // report tick in case a reporter is active
        game.backends.tick_system.report.store(Tick {
            duration: elapsed_time,
            waiting_time,
            index,
        });

        index += 1;

        if game.exit.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
    }
}

/// The settings for the tick system of the game engine.
#[derive(Clone, Debug, Builder)]
pub struct TickSettings {
    /// The target duration to wait after every tick.
    ///
    /// ## Default
    ///
    /// - 1 / 62 seconds
    ///
    /// 62 ticks per second.
    #[builder(setter(into), default = "Duration::from_secs_f64(1.0/62.0)")]
    pub tick_wait: Duration,
    /// The waiting behaviour of this tick system.
    ///
    /// ## Default
    ///
    /// `TimeStep::Variable`
    ///
    /// Prevents the game from slowing down in case ticks become more time expensive.
    #[builder(default)]
    pub timestep_mode: TimeStep,
    /// If true this tick system will also iterate all the physics systems in the scene and update them.
    ///
    /// ## Default
    ///
    /// `true`
    #[builder(default = "true")]
    #[cfg(feature = "physics")]
    pub update_physics: bool,
    /// If this is true the tick system will be paused.
    ///
    /// ## Default
    ///
    /// `false`
    #[builder(default)]
    pub paused: bool,
    /// If this is true the tick systems tick rate will be influenced by the time scale.
    ///
    /// ## Default
    ///
    /// `true`
    #[builder(default = "true")]
    pub time_scale_influence: bool,
}

impl Default for TickSettings {
    fn default() -> Self {
        Self {
            tick_wait: Duration::from_secs_f64(1.0 / 62.0),
            #[cfg(feature = "physics")]
            update_physics: true,
            timestep_mode: TimeStep::default(),
            paused: false,
            time_scale_influence: true,
        }
    }
}
impl TickSettings {
    pub fn into_builder(self) -> TickSettingsBuilder {
        self.into()
    }
}
impl From<TickSettings> for TickSettingsBuilder {
    fn from(value: TickSettings) -> Self {
        Self {
            tick_wait: Some(value.tick_wait),
            timestep_mode: Some(value.timestep_mode),
            #[cfg(feature = "physics")]
            update_physics: Some(value.update_physics),
            paused: Some(value.paused),
            time_scale_influence: Some(value.time_scale_influence),
        }
    }
}

/// A tick report.
#[derive(Clone, Default, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick {
    /// Time it took to execute this tick.
    pub duration: Duration,
    /// Time the tick waited before running the next one.
    pub waiting_time: Duration,
    /// The index of this tick.
    pub index: usize,
}

impl Tick {
    pub fn duration(&self) -> &Duration {
        &self.duration
    }
    pub fn waiting_time(&self) -> &Duration {
        &self.waiting_time
    }
    pub fn index(&self) -> usize {
        self.index
    }
    /// Returns true if the tick execution time takes longer than the expected waiting time.
    ///
    /// Because if the tick execution takes longer than the target waiting time the rate decreases making the logic behind the tick system slower.
    pub fn has_slowdown(&self) -> bool {
        self.waiting_time.is_zero()
    }
}

impl std::fmt::Debug for Tick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tick")
            .field("duration", &self.duration)
            .field("waiting time", &self.waiting_time)
            .field("index", &self.index)
            .field("has slowdown", &self.has_slowdown())
            .finish()
    }
}

/// The waiting behaviour of the tick system.
///
/// Set to variable by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TimeStep {
    /// Wait a fixed time after every tick, not caring about the duration the tick actually lasted for.
    Fixed,
    /// Wait a variable time using the tick_wait field as a target duration.
    ///
    /// That means the tick system waits less the longer the tick took to execute.
    /// This for example prevents the physics system from slowing down in case the iterations get more expensive.
    Variable,
}

impl Default for TimeStep {
    fn default() -> Self {
        Self::Variable
    }
}
