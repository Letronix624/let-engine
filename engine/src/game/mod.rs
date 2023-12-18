use super::objects;
use anyhow::Result;
use derive_builder::Builder;
use glam::vec2;
use objects::scenes::Scene;
use once_cell::unsync::OnceCell;
use parking_lot::{Condvar, Mutex};
pub mod window;
pub(crate) use super::draw::Draw;
pub use winit::event_loop::ControlFlow;
use winit::{
    event::{DeviceEvent, Event, MouseScrollDelta, StartCause, WindowEvent},
    event_loop::{EventLoop, EventLoopBuilder},
};
pub mod input;
use input::Input;
#[cfg(feature = "egui")]
mod egui;
mod tick_system;
pub use tick_system::*;
pub mod events;

use atomic_float::AtomicF64;
use crossbeam::atomic::AtomicCell;

use std::{cell::RefCell, time::Duration};
use std::{
    sync::{atomic::Ordering, Arc},
    time::SystemTime,
};

use crate::{error::draw::VulkanError, resources::LABELIFIER};

use self::{
    events::{InputEvent, ScrollDelta},
    window::{Window, WindowBuilder},
};

thread_local! {
    pub static EVENT_LOOP: RefCell<OnceCell<EventLoop<()>>> = RefCell::new(OnceCell::new());
}

#[derive(Clone)]
pub struct Components {
    scene: Scene,
    pub(crate) time: Arc<Time>,
    pub(crate) input: Input,
    window: Window,
    tick_settings: Arc<Mutex<TickSettings>>,
    pub(crate) tick_settings_changed: Arc<AtomicCell<bool>>,
}

impl Components {
    pub(crate) fn new(
        scene: Scene,
        time: Arc<Time>,
        input: Input,
        window: Window,

        tick_settings: TickSettings,
    ) -> Self {
        Self {
            scene,
            time,
            input,
            window,
            tick_settings: Arc::new(Mutex::new(tick_settings)),
            tick_settings_changed: Arc::new(AtomicCell::new(false)),
        }
    }
    pub fn scene(&self) -> &Scene {
        &self.scene
    }
    pub fn time(&self) -> &Arc<Time> {
        &self.time
    }
    pub fn input(&self) -> &Input {
        &self.input
    }
    pub fn window(&self) -> &Window {
        &self.window
    }
    pub fn tick_settings(&self) -> TickSettings {
        self.tick_settings.lock().clone()
    }
    pub fn set_tick_settings(&self, settings: TickSettings) {
        *self.tick_settings.lock() = settings;
        self.tick_settings_changed.store(true);
    }
}

/// Represents the game application with essential methods for a game's lifetime.
pub trait Game {
    /// Runs right before the first frame is drawn, initializing the instance.
    fn start(&mut self, _components: &Components) {}
    /// Runs before the frame is drawn.
    fn update(&mut self, _components: &Components) {}
    /// Runs after the frame is drawn.
    fn frame_update(&mut self, _components: &Components) {}
    /// Runs based on the configured tick settings of the engine.
    fn tick(&mut self, _components: &Components) {}
    /// Handles engine and window events.
    fn event(&mut self, _event: events::Event, _components: &Components) {}
    /// If true exits the program, stopping the loop and closing the window, when true.
    fn exit(&self) -> bool;
}

/// The initial settings of this engine.
#[derive(Clone, Builder, Default)]
pub struct EngineSettings {
    /// Settings that determines the look of the window.
    #[builder(setter(into, strip_option), default)]
    pub window_settings: WindowBuilder,
    /// The initial settings of the tick system.
    #[builder(setter(into), default)]
    pub tick_settings: TickSettings,
    // /// Starting scene of the game engine.
    // pub scene: Option<Scene>,
}

/// The struct that holds and executes all of the game data.
pub struct Engine {
    components: Components,
    #[cfg(feature = "egui")]
    gui: egui_winit_vulkano::Gui,
    tick_system: TickSystem,

    draw: Draw,
}

impl Engine {
    /// Initializes the game engine with the given settings ready to be launched using the `start` method.
    pub fn new(settings: impl Into<EngineSettings>) -> Result<Self> {
        let settings = settings.into();

        let scene = Scene::default();
        let input = Input::new();
        let time = Arc::new(Time::default());
        let tick_system = TickSystem::new(settings.tick_settings.clone());

        EVENT_LOOP.with_borrow_mut(|cell| {
            cell.get_or_init(|| EventLoopBuilder::new().build());
        });
        let draw = Draw::setup(settings.window_settings);

        #[cfg(feature = "egui")]
        let gui = egui::init(&draw);

        let window = draw.get_window();

        let components =
            Components::new(scene, time, input, window.clone(), settings.tick_settings);

        Ok(Self {
            components,

            #[cfg(feature = "egui")]
            gui,
            tick_system,

            draw,
        })
    }

    pub fn components(&self) -> &Components {
        &self.components
    }

    pub fn start(mut self, game: impl Game + Send + 'static) {
        let game = Arc::new(Mutex::new(game));
        EVENT_LOOP.with_borrow_mut(|event_loop| {
            event_loop
                .take()
                .unwrap()
                .run(move |event, _, control_flow| {
                    self.components
                        .input
                        .update(&event, self.components.window.inner_size());
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
                                game.lock().event(event, &self.components);
                            }
                        }
                        Event::DeviceEvent { event, .. } => match event {
                            DeviceEvent::MouseMotion { delta } => {
                                game.lock().event(
                                    events::Event::Input(InputEvent::MouseMotion(vec2(
                                        delta.0 as f32,
                                        delta.1 as f32,
                                    ))),
                                    &self.components,
                                );
                            }
                            DeviceEvent::MouseWheel { delta } => {
                                game.lock().event(
                                    events::Event::Input(InputEvent::MouseWheel(match delta {
                                        MouseScrollDelta::LineDelta(x, y) => {
                                            ScrollDelta::LineDelta(vec2(x, y))
                                        }
                                        MouseScrollDelta::PixelDelta(delta) => {
                                            ScrollDelta::PixelDelta(delta)
                                        }
                                    })),
                                    &self.components,
                                );
                            }
                            _ => (),
                        },
                        Event::MainEventsCleared => {
                            #[cfg(feature = "egui")]
                            self.gui.immediate_ui(|gui| {
                                game.lock()
                                    .event(events::Event::Egui(gui.context()), &self.components);
                            });

                            game.lock().update(&self.components);
                            self.components.window.request_redraw();
                        }
                        Event::RedrawRequested(_) => {
                            let labelifier = &LABELIFIER;
                            labelifier.lock().update();
                            match self.draw.redrawevent(
                                &self.components.scene,
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
                            self.components.time.update();
                            game.lock().frame_update(&self.components);
                        }
                        Event::LoopDestroyed => {
                            game.lock()
                                .event(events::Event::Destroyed, &self.components);
                        }
                        Event::NewEvents(StartCause::Init) => {
                            #[cfg(feature = "egui")]
                            self.gui.immediate_ui(|gui| {
                                game.lock()
                                    .event(events::Event::Egui(gui.context()), &self.components);
                            });
                            match self.draw.redrawevent(
                                &self.components.scene,
                                #[cfg(feature = "egui")]
                                &mut self.gui,
                            ) {
                                Err(VulkanError::SwapchainOutOfDate) => {
                                    self.draw.recreate_swapchain = true
                                }
                                Err(e) => panic!("{e}"),
                                _ => (),
                            };
                            game.lock().start(&self.components);
                            if !self.components.tick_settings.lock().paused {
                                self.tick_system
                                    .run(Arc::clone(&game), self.components.clone());
                            }
                        }
                        _ => (),
                    }
                    if self.components.tick_settings_changed.load() {
                        // Also stops the tick system.
                        self.tick_system
                            .update(self.components.tick_settings().clone());
                        // Start it back up in case it is not paused.
                        if !self.components.tick_settings.lock().paused {
                            self.tick_system
                                .run(Arc::clone(&game), self.components.clone());
                        }
                        self.components.tick_settings_changed.store(false);
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
