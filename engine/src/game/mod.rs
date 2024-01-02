// TODO: Please make this look better, oh no!
use anyhow::Result;
use derive_builder::Builder;
#[cfg(feature = "client")]
use glam::vec2;
#[cfg(feature = "client")]
use once_cell::unsync::OnceCell;
use parking_lot::{Condvar, Mutex};
use thiserror::Error;
#[cfg(feature = "client")]
pub mod window;
#[cfg(feature = "client")]
pub(crate) use super::draw::Draw;
#[cfg(feature = "client")]
pub use winit::event_loop::ControlFlow;
#[cfg(feature = "client")]
use winit::{
    event::{DeviceEvent, Event, MouseScrollDelta, StartCause, WindowEvent},
    event_loop::{EventLoop, EventLoopBuilder},
};
#[cfg(all(feature = "egui", feature = "client"))]
mod egui;
#[cfg(feature = "client")]
pub mod input;
mod tick_system;
pub use tick_system::*;
#[cfg(feature = "client")]
pub mod events;

use atomic_float::AtomicF64;
use crossbeam::atomic::AtomicCell;

#[cfg(feature = "client")]
use std::cell::RefCell;
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
    time::SystemTime,
};

#[cfg(feature = "client")]
use crate::{
    error::draw::VulkanError,
    resources::LABELIFIER,
    resources::{
        sounds::{AudioSettings, NoAudioServerError},
        RESOURCES,
    },
    INPUT,
};
use crate::{SETTINGS, TIME};

#[cfg(feature = "client")]
use self::{
    events::{InputEvent, ScrollDelta},
    window::{Window, WindowBuilder},
};

// The event loop that gets created and executed in the thread where Engine was made.
#[cfg(feature = "client")]
thread_local! {
    pub(crate) static EVENT_LOOP: RefCell<OnceCell<EventLoop<()>>> = RefCell::new(OnceCell::new());
}

/// Represents the game application with essential methods for a game's lifetime.
/// # Usage
///
/// ```
/// use let_engine::prelude::*;
///
/// struct Game {
///     exit: bool,
/// }
///
/// impl let_engine::Game for Game {
///     fn exit(&self) -> bool {
///        // exits the program in case self.exit is true
///        self.exit
///     }
///     fn update(&mut self) {
///         // runs every frame or every engine loop update.
///         //...
///     }
/// }
/// ```
pub trait Game {
    #[cfg_attr(
        feature = "client",
        doc = "Runs right before the first frame is drawn and the window gets displayed, initializing the instance."
    )]
    #[cfg_attr(
        not(feature = "client"),
        doc = "Runs right after the `start` function was called for the Engine."
    )]
    fn start(&mut self) {}
    #[cfg_attr(feature = "client", doc = "Runs before the frame is drawn.")]
    #[cfg_attr(
        not(feature = "client"),
        doc = "Runs in a loop after the `start` function."
    )]
    fn update(&mut self) {}
    /// Runs after the frame is drawn.
    #[cfg(feature = "client")]
    fn frame_update(&mut self) {}
    /// Runs based on the configured tick settings of the engine.
    fn tick(&mut self) {}
    /// Handles engine and window events.
    #[allow(unused_variables)]
    #[cfg(feature = "client")]
    fn event(&mut self, event: events::Event) {}
    #[cfg_attr(
        feature = "client",
        doc = "If true exits the program, stopping the loop and closing the window."
    )]
    #[cfg_attr(
        not(feature = "client"),
        doc = "If true exits the program, stopping the loop."
    )]
    fn exit(&self) -> bool;
}

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
    tick_settings: Mutex<TickSettings>,
    tick_pause_lock: (Mutex<bool>, Condvar),
    #[cfg(feature = "client")]
    window: Mutex<OnceCell<Arc<Window>>>,
    #[cfg(feature = "client")]
    audio_settings: Mutex<AudioSettings>,
}

impl Settings {
    pub(crate) fn new() -> Self {
        Self {
            tick_settings: Mutex::new(TickSettings::default()),
            tick_pause_lock: (Mutex::new(false), Condvar::new()),
            #[cfg(feature = "client")]
            window: Mutex::new(OnceCell::new()),
            #[cfg(feature = "client")]
            audio_settings: Mutex::new(AudioSettings::new()),
        }
    }
    #[cfg(feature = "client")]
    pub(crate) fn set_window(&self, window: Arc<Window>) {
        self.window.lock().set(window).unwrap();
    }
    #[cfg(feature = "client")]
    /// Returns the window of the game in case it is initialized.
    pub fn window(&self) -> Option<Arc<Window>> {
        self.window.lock().get().cloned()
    }
    /// Returns the engine wide tick settings.
    pub fn tick_settings(&self) -> TickSettings {
        self.tick_settings.lock().clone()
    }
    /// Sets the tick settings of the game engine.
    pub fn set_tick_settings(&self, settings: TickSettings) {
        *self.tick_pause_lock.0.lock() = settings.paused;
        *self.tick_settings.lock() = settings;
        self.tick_pause_lock.1.notify_all();
    }
    /// Returns the audio settings.
    #[cfg(feature = "client")]
    pub fn audio_settings(&self) -> AudioSettings {
        *self.audio_settings.lock()
    }
    /// Sets the audio settings and refreshes the engine side audio server to use them.
    #[cfg(feature = "client")]
    pub fn set_audio_settings(&self, settings: AudioSettings) -> Result<(), NoAudioServerError> {
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

/// The struct that holds and executes all of the game data.
pub struct Engine {
    #[cfg(all(feature = "egui", feature = "client"))]
    gui: egui_winit_vulkano::Gui,
    tick_system: TickSystem,

    #[cfg(feature = "client")]
    draw: Draw,
}

/// Makes sure the engine struct only gets constructed a single time.
static INIT: parking_lot::Once = parking_lot::Once::new();

/// Engine can only be made once.
#[derive(Error, Debug, Clone, Copy)]
#[error("You can only initialize this game engine one single time.")]
pub struct EngineInitialized;

impl Engine {
    /// Initializes the game engine with the given settings ready to be launched using the `start` method.
    ///
    /// This function can only be called one time. Attempting to make a second one of those will return an error.
    pub fn new(settings: impl Into<EngineSettings>) -> Result<Self> {
        if INIT.state() == parking_lot::OnceState::New {
            INIT.call_once(|| {});
            let settings = settings.into();
            SETTINGS.set_tick_settings(settings.tick_settings);
            let tick_system = TickSystem::new();

            #[cfg(feature = "client")]
            EVENT_LOOP.with_borrow_mut(|cell| {
                cell.get_or_init(|| EventLoopBuilder::new().build());
            });
            #[cfg(feature = "client")]
            let draw = Draw::setup(settings.window_settings);
            #[cfg(feature = "client")]
            SETTINGS.set_window(draw.get_window().clone());

            #[cfg(all(feature = "egui", feature = "client"))]
            let gui = egui::init(&draw);

            Ok(Self {
                #[cfg(all(feature = "egui", feature = "client"))]
                gui,
                tick_system,

                #[cfg(feature = "client")]
                draw,
            })
        } else {
            Err(EngineInitialized.into())
        }
    }

    /// Returns the window of the game.
    #[cfg(feature = "client")]
    pub fn get_window(&self) -> &Window {
        self.draw.get_window()
    }
    /// Server side start function running all the methods of the given game object as documented in the [trait](Game).
    #[cfg(not(feature = "client"))]
    pub fn start(mut self, game: impl Game + Send + 'static) {
        let game = Arc::new(Mutex::new(game));

        game.lock().start();
        self.tick_system.run(Arc::clone(&game));
        loop {
            if game.lock().exit() {
                break;
            }
            game.lock().update();
            TIME.update();
        }
    }
    /// Client start function running all the methods of the given game object as documented in the [trait](Game).
    #[cfg(feature = "client")]
    pub fn start(mut self, game: impl Game + Send + 'static) {
        let game = Arc::new(Mutex::new(game));

        EVENT_LOOP.with_borrow_mut(|event_loop| {
            event_loop
                .take()
                .unwrap()
                .run(move |event, _, control_flow| {
                    INPUT.update(&event, self.get_window().inner_size());
                    if game.lock().exit() {
                        control_flow.set_exit();
                    }
                    match event {
                        Event::WindowEvent { event, .. } => {
                            #[cfg(feature = "egui")]
                            self.gui.update(&event);
                            let event = match event {
                                WindowEvent::Resized(size) => {
                                    self.draw.recreate_swapchain = true;
                                    events::Event::Window(events::WindowEvent::Resized(size))
                                }
                                WindowEvent::ReceivedCharacter(char) => {
                                    events::Event::Input(InputEvent::ReceivedCharacter(char))
                                }
                                WindowEvent::CloseRequested => {
                                    events::Event::Window(events::WindowEvent::CloseRequested)
                                }
                                WindowEvent::CursorEntered { .. } => {
                                    events::Event::Window(events::WindowEvent::CursorEntered)
                                }
                                WindowEvent::CursorLeft { .. } => {
                                    events::Event::Window(events::WindowEvent::CursorLeft)
                                }
                                WindowEvent::CursorMoved { position, .. } => events::Event::Window(
                                    events::WindowEvent::CursorMoved(position),
                                ),
                                WindowEvent::Destroyed => {
                                    events::Event::Window(events::WindowEvent::Destroyed)
                                }
                                WindowEvent::HoveredFile(file) => {
                                    events::Event::Window(events::WindowEvent::HoveredFile(file))
                                }
                                WindowEvent::DroppedFile(file) => {
                                    events::Event::Window(events::WindowEvent::DroppedFile(file))
                                }
                                WindowEvent::HoveredFileCancelled => {
                                    events::Event::Window(events::WindowEvent::HoveredFileCancelled)
                                }
                                WindowEvent::Focused(focused) => {
                                    events::Event::Window(events::WindowEvent::Focused(focused))
                                }
                                WindowEvent::KeyboardInput { input, .. } => {
                                    events::Event::Input(InputEvent::KeyboardInput {
                                        input: events::KeyboardInput {
                                            scancode: input.scancode,
                                            keycode: input.virtual_keycode,
                                            state: input.state,
                                        },
                                    })
                                }
                                WindowEvent::ModifiersChanged(_) => {
                                    events::Event::Input(InputEvent::ModifiersChanged)
                                }
                                WindowEvent::MouseInput { state, button, .. } => {
                                    events::Event::Input(InputEvent::MouseInput(button, state))
                                }
                                WindowEvent::MouseWheel { delta, .. } => events::Event::Window(
                                    events::WindowEvent::MouseWheel(match delta {
                                        MouseScrollDelta::LineDelta(x, y) => {
                                            ScrollDelta::LineDelta(vec2(x, y))
                                        }
                                        MouseScrollDelta::PixelDelta(x) => {
                                            ScrollDelta::PixelDelta(x)
                                        }
                                    }),
                                ),
                                _ => events::Event::Destroyed,
                            };
                            // destroy event can't be called here so I did the most lazy approach possible.
                            if let events::Event::Destroyed = event {
                            } else {
                                game.lock().event(event);
                            }
                        }
                        Event::DeviceEvent { event, .. } => match event {
                            DeviceEvent::MouseMotion { delta } => {
                                game.lock()
                                    .event(events::Event::Input(InputEvent::MouseMotion(vec2(
                                        delta.0 as f32,
                                        delta.1 as f32,
                                    ))));
                            }
                            DeviceEvent::MouseWheel { delta } => {
                                game.lock()
                                    .event(events::Event::Input(InputEvent::MouseWheel(
                                        match delta {
                                            MouseScrollDelta::LineDelta(x, y) => {
                                                ScrollDelta::LineDelta(vec2(x, y))
                                            }
                                            MouseScrollDelta::PixelDelta(delta) => {
                                                ScrollDelta::PixelDelta(delta)
                                            }
                                        },
                                    )));
                            }
                            _ => (),
                        },
                        Event::MainEventsCleared => {
                            #[cfg(feature = "egui")]
                            self.gui.immediate_ui(|gui| {
                                game.lock().event(events::Event::Egui(gui.context()));
                            });

                            game.lock().update();
                            self.get_window().request_redraw();
                        }
                        Event::RedrawRequested(_) => {
                            let labelifier = &LABELIFIER;
                            labelifier.lock().update();
                            match self.draw.redraw_event(
                                #[cfg(feature = "egui")]
                                &mut self.gui,
                            ) {
                                Err(VulkanError::SwapchainOutOfDate) => {
                                    self.draw.recreate_swapchain = true
                                }
                                Err(e) => panic!("{e}"),
                                _ => (),
                            };
                        }
                        Event::RedrawEventsCleared => {
                            TIME.update();
                            game.lock().frame_update();
                        }
                        Event::LoopDestroyed => {
                            game.lock().event(events::Event::Destroyed);
                        }
                        Event::NewEvents(StartCause::Init) => {
                            #[cfg(feature = "egui")]
                            self.gui.immediate_ui(|gui| {
                                game.lock().event(events::Event::Egui(gui.context()));
                            });
                            match self.draw.redraw_event(
                                #[cfg(feature = "egui")]
                                &mut self.gui,
                            ) {
                                Err(VulkanError::SwapchainOutOfDate) => {
                                    self.draw.recreate_swapchain = true
                                }
                                Err(e) => panic!("{e}"),
                                _ => (),
                            };
                            game.lock().start();
                            self.get_window().initialize();
                            self.tick_system.run(Arc::clone(&game));
                        }
                        _ => (),
                    }
                });
        });
    }
}

/// Holds the timings of the engine like runtime and delta time.
pub struct Time {
    /// Time since engine start.
    time: SystemTime,
    time_scale: AtomicF64,
    delta_instant: AtomicCell<SystemTime>,
    delta_time: AtomicF64,
    pub(crate) zero_cvar: (Mutex<()>, Condvar),
}

impl Default for Time {
    fn default() -> Self {
        Self {
            time: SystemTime::now(),
            time_scale: AtomicF64::new(1.0f64),
            delta_instant: AtomicCell::new(SystemTime::now()),
            delta_time: AtomicF64::new(0.0f64),
            zero_cvar: (Mutex::new(()), Condvar::new()),
        }
    }
}

impl Time {
    /// Updates the time data on frame redraw.
    pub(crate) fn update(&self) {
        self.delta_time.store(
            self.delta_instant.load().elapsed().unwrap().as_secs_f64(),
            Ordering::Release,
        );
        self.delta_instant.store(SystemTime::now());
    }

    /// Returns the time it took to execute last iteration.
    pub fn delta_time(&self) -> f64 {
        self.delta_time.load(Ordering::Acquire) * self.scale()
    }

    /// Returns the delta time of the update iteration that does not scale with the time scale.
    pub fn unscaled_delta_time(&self) -> f64 {
        self.delta_time.load(Ordering::Acquire)
    }

    /// Returns the frames per second.
    pub fn fps(&self) -> f64 {
        1.0 / self.delta_time.load(Ordering::Acquire)
    }

    /// Returns the time since start of the engine game session.
    pub fn time(&self) -> f64 {
        self.time.elapsed().unwrap().as_secs_f64()
    }

    /// Returns the time scale of the game
    pub fn scale(&self) -> f64 {
        self.time_scale.load(Ordering::Acquire)
    }

    /// Sets the time scale of the game.
    ///
    /// Panics if the given time scale is negative.
    pub fn set_scale(&self, time_scale: f64) {
        if time_scale.is_sign_negative() {
            panic!("A negative time scale was given.");
        }
        self.time_scale.store(time_scale, Ordering::Release);
        if time_scale != 0.0 {
            self.zero_cvar.1.notify_all();
        }
    }

    /// Sleeps the given duration times the time scale of the game engine.
    pub fn sleep(&self, duration: Duration) {
        spin_sleep::sleep(duration.mul_f64(self.time_scale.load(Ordering::Acquire)));
    }
}

#[cfg(not(feature = "client"))]
#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn start_engine() {
        let engine = Engine::new(EngineSettings::default()).unwrap();

        struct Game {
            number: u32,
            exit: bool,
        }
        impl Game {
            pub fn new() -> Self {
                Self {
                    number: 0,
                    exit: false,
                }
            }
        }

        impl crate::Game for Game {
            fn exit(&self) -> bool {
                self.exit
            }
            fn tick(&mut self) {
                self.number += 1;
                if self.number > 62 {
                    self.exit = true;
                }
            }
        }

        engine.start(Game::new());
    }
}
