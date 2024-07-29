use std::{
    marker::PhantomData,
    sync::atomic::AtomicBool,
    time::{Duration, SystemTime},
};

use async_std::sync::{Arc, Mutex};
use crossbeam::atomic::AtomicCell;
use derive_builder::Builder;

use crate::{Game, SETTINGS, TIME};

#[cfg(not(feature = "networking"))]
pub(crate) struct TickSystem<G: Game + Send + 'static, #[cfg(feature = "networking")] Msg> {
    stop: Arc<AtomicBool>,
    #[cfg(feature = "networking")]
    _msg: PhantomData<Msg>,
    _game: PhantomData<G>,
}

#[cfg(feature = "networking")]
pub(crate) struct TickSystem<G: Game<Msg> + Send + 'static, #[cfg(feature = "networking")] Msg> {
    stop: Arc<AtomicBool>,
    #[cfg(feature = "networking")]
    _msg: PhantomData<Msg>,
    _game: PhantomData<G>,
}

macro_rules! impl_ticksys {
    { impl TickSystem $implementations:tt } => {
        #[cfg(not(feature = "networking"))]
        impl<G: Game + Send + 'static> TickSystem<G> $implementations

        #[cfg(feature = "networking")]
        impl<G: Game<Msg> + Send + 'static, Msg> TickSystem<G, Msg> $implementations
    };
}

impl_ticksys! {
    impl TickSystem {
        pub fn new() -> Self {
            Self {
                stop: Arc::new(AtomicBool::new(false)),
                #[cfg(feature = "networking")]
                _msg: PhantomData,
                _game: PhantomData
            }
        }
        /// Runs the games `tick` function after every iteration.
        pub async fn run(&mut self, game: Arc<Mutex<G>>) {
            let mut index: usize = 0;
            let stop = self.stop.clone();
            let game = game.clone();
            loop {
                // wait if paused
                SETTINGS
                    .tick_system
                    .tick_pause_lock
                    .1
                    .wait_while(&mut SETTINGS.tick_system.tick_pause_lock.0.lock(), |x| *x);
                let settings = SETTINGS.tick_system.get();
                // capture tick start time.
                let start_time = SystemTime::now();
                // Run the logic
                game.lock().await.tick().await;

                // update the physics in case they are active in the tick settings.
                #[cfg(feature = "physics")]
                if let_engine_core::objects::scenes::SCENE
                    .update(settings.update_physics)
                    .is_err()
                {
                    // Disable physics updating if it fails. Return running this tick system.
                    SETTINGS.tick_system.tick_settings.lock().update_physics = false;
                };
                // record the elapsed time.
                let elapsed_time = start_time.elapsed().unwrap_or_default();

                // Lock the thread in case the time scale is 0.
                if TIME.scale() == 0.0 {
                    let mut guard = TIME.zero_cvar.0.lock();
                    TIME.zero_cvar.1.wait(&mut guard);
                }

                let tick_wait = if settings.time_scale_influence {
                    // Multiply the waiting duration with the inverse time scale.
                    settings.tick_wait.mul_f64(1.0 / TIME.scale())
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
                if let Some(ref reporter) = settings.reporter {
                    reporter.update(Tick {
                        duration: elapsed_time,
                        waiting_time,
                        index,
                    });
                }
                index += 1;
                if stop.load(std::sync::atomic::Ordering::Acquire) {
                    break;
                }
            }
        }
    }
}

/// The settings for the tick system of the game engine.
#[derive(Clone, Debug, Builder)]
pub struct TickSettings {
    /// The target duration to wait after every tick.
    ///
    /// ## Default configuration:
    ///
    /// - 1 / 62 seconds
    ///
    /// 62 ticks per second.
    #[builder(setter(into), default = "Duration::from_secs_f64(1.0/62.0)")]
    pub tick_wait: Duration,
    /// The waiting behaviour of this tick system.
    ///
    /// ## Default configuration:
    ///
    /// `TimeStep::Variable`
    ///
    /// Prevents the game from slowing down in case ticks become more time expensive.
    #[builder(default)]
    pub timestep_mode: TimeStep,
    /// If true this tick system will also iterate all the physics systems in the scene and update them.
    ///
    /// ## Default configuration:
    ///
    /// `true`
    #[builder(default = "true")]
    #[cfg(feature = "physics")]
    pub update_physics: bool,
    /// If there is some reporter it will report about the most recent tick to the given reporter.
    ///
    /// ## Default configuration:
    ///
    /// `None`
    #[builder(setter(strip_option), default)]
    pub reporter: Option<TickReporter>,
    /// If this is true the tick system will be paused.
    ///
    /// ## Default configuration:
    ///
    /// `false`
    #[builder(default)]
    pub paused: bool,
    /// If this is true the tick systems tick rate will be influenced by the time scale.
    ///
    /// ## Default configuration:
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
            reporter: None,
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
            reporter: Some(value.reporter),
            paused: Some(value.paused),
            time_scale_influence: Some(value.time_scale_influence),
        }
    }
}

/// A reporter containing information about the most recent tick.
#[derive(Clone)]
pub struct TickReporter {
    tick: Arc<AtomicCell<Tick>>,
}

impl TickReporter {
    pub fn new() -> Self {
        Self {
            tick: Arc::new(AtomicCell::new(Tick::default())),
        }
    }
    pub fn get(&self) -> Tick {
        self.tick.load()
    }
    pub(crate) fn update(&self, tick: Tick) {
        self.tick.store(tick)
    }
}

impl Default for TickReporter {
    fn default() -> Self {
        Self::new()
    }
}
impl std::fmt::Debug for TickReporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tick Report")
            .field("tick", &self.tick.load())
            .finish()
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
