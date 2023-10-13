pub mod resources;
use glam::vec2;
use resources::{vulkan::Vulkan, Loader, Resources};
pub mod objects;
use objects::Node;
pub use objects::{
    physics,
    scenes::{Layer, Scene},
    GameObject, Transform,
};
pub mod camera;
mod draw;
pub mod window;
pub use draw::Draw;
use objects::labels::Labelifier;
pub use winit::event_loop::ControlFlow;
use winit::{
    event::{DeviceEvent, Event, MouseScrollDelta, WindowEvent},
    event_loop::{EventLoop, EventLoopBuilder},
};
pub mod input;
pub use input::Input;
pub use resources::materials;
#[cfg(feature = "egui")]
pub mod egui;
pub mod events;

use atomic_float::AtomicF64;
pub use engine_macros;
use parking_lot::Mutex;

use std::{
    sync::{atomic::Ordering, Arc, Weak},
    time::SystemTime,
};

pub use resources::data;
pub use resources::data::{tvert, vert, Vertex};

use self::{
    events::{InputEvent, ScrollDelta},
    window::{Window, WindowBuilder},
};

pub(crate) type AObject = Box<dyn GameObject>;
pub type NObject = Arc<Mutex<Node<AObject>>>;
pub type WeakObject = Weak<Mutex<Node<AObject>>>;

/// Initializes the let engine.
///
/// Creates 4 static variables that can be accessed everywhere to use.
///
/// `INPUT`: a live updated [Input] struct.
///
/// `TIME`: a live updated [Time] struct.
///
/// `RESOURCES`: The resource manager where you load your assets.
///
/// `WINDOW`: A [Window] that you can change the attributes of once you've run the [start_engine] macro.
#[macro_export]
macro_rules! let_engine {
    () => {
        static INPUT: let_engine::Lazy<let_engine::Input> =
            let_engine::Lazy::new(let_engine::Input::default);
        static TIME: let_engine::Lazy<let_engine::Time> =
            let_engine::Lazy::new(let_engine::Time::default);
        static _RESOURCES: let_engine::Lazy<let_engine::_Resources> = let_engine::Lazy::new(|| {
            std::sync::Arc::new(let_engine::Mutex::new(
                let_engine::resources::Resources::new(),
            ))
        });
        static SCENE: let_engine::Lazy<let_engine::Scene> =
            let_engine::Lazy::new(let_engine::Scene::default);
        static RESOURCES: let_engine::Lazy<let_engine::resources::Resources> =
            let_engine::Lazy::new(|| {
                let resources = _RESOURCES.lock();
                resources.clone()
            });
        static WINDOW: let_engine::Lazy<let_engine::window::Window> =
            let_engine::Lazy::new(|| _RESOURCES.lock().get_window());
    };
}

/// Starts the engine, enables the window and starts drawing the scene.
/// Takes a [window::WindowBuilder] for the initial window.
#[macro_export]
macro_rules! start_engine {
    ($window_builder:expr) => {{
        let_engine::Game::new(
            $window_builder,
            _RESOURCES.clone(),
            SCENE.clone(),
            INPUT.clone(),
            TIME.clone(),
        )
    }};
}

/// The struct that holds and executes all of the game data.
pub struct Game {
    resources: Resources,
    scene: Scene,
    input: Input,
    time: Time,
    window: Window,
    event_loop: EventLoop<()>,
    #[cfg(feature = "egui")]
    gui: egui_winit_vulkano::Gui,

    draw: Draw,
}

impl Game {
    pub fn new(
        window_builder: WindowBuilder,
        resources: Arc<Mutex<Resources>>,
        scene: Scene,
        input: Input,
        time: Time,
    ) -> Self {
        let event_loop = EventLoopBuilder::new().build();
        let vulkan = Vulkan::init(&event_loop, window_builder);

        #[cfg(feature = "egui")]
        let gui = egui::init(&event_loop, &vulkan);

        let mut resources = resources.lock();
        resources.init(vulkan);
        let resources = resources.clone();

        let draw = Draw::setup(&resources);

        let window = resources.get_window();

        Self {
            resources,
            scene,
            input,
            time,
            window,
            event_loop,

            #[cfg(feature = "egui")]
            gui,

            draw,
        }
    }

    /// Runs the main loop updating the window after every iteration.
    ///
    /// There is also a provided control flow.
    ///
    /// On `Wait` this will update each window event.
    /// It also allows updating the window using the `request_redraw()` function for the [window::Window]
    pub fn run_loop<F>(mut self, mut func: F)
    where
        F: FnMut(events::Event, &mut ControlFlow) + 'static,
    {
        let event_loop = self.event_loop;

        event_loop.run(move |event, _, control_flow| {
            self.input.update(&event, self.window.inner_size());
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
                        WindowEvent::CursorMoved { position, .. } => {
                            events::Event::Window(events::WindowEvent::CursorMoved(position))
                        }
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
                        WindowEvent::MouseWheel { delta, .. } => {
                            events::Event::Window(events::WindowEvent::MouseWheel(match delta {
                                MouseScrollDelta::LineDelta(x, y) => {
                                    ScrollDelta::LineDelta(vec2(x, y))
                                }
                                MouseScrollDelta::PixelDelta(x) => ScrollDelta::PixelDelta(x),
                            }))
                        }
                        _ => events::Event::Destroyed,
                    };
                    // destroy event can't be called here so I did the most lazy approach possible.
                    if let events::Event::Destroyed = event {
                    } else {
                        func(event, control_flow);
                    }
                }
                Event::DeviceEvent { event, .. } => match event {
                    DeviceEvent::MouseMotion { delta } => {
                        func(
                            events::Event::Input(InputEvent::MouseMotion(vec2(
                                delta.0 as f32,
                                delta.1 as f32,
                            ))),
                            control_flow,
                        );
                    }
                    DeviceEvent::MouseWheel { delta } => {
                        func(
                            events::Event::Input(InputEvent::MouseWheel(match delta {
                                MouseScrollDelta::LineDelta(x, y) => {
                                    ScrollDelta::LineDelta(vec2(x, y))
                                }
                                MouseScrollDelta::PixelDelta(delta) => {
                                    ScrollDelta::PixelDelta(delta)
                                }
                            })),
                            control_flow,
                        );
                    }
                    _ => (),
                },
                Event::MainEventsCleared => {
                    #[cfg(feature = "egui")]
                    {
                        self.gui.immediate_ui(|gui| {
                            func(events::Event::Egui(gui.context()), control_flow);
                        });
                        self.gui.immediate_ui(|_gui| {});
                    }

                    func(events::Event::Update, control_flow);
                }
                Event::RedrawEventsCleared => {
                    self.resources.update();
                    self.draw.redrawevent(
                        &self.resources,
                        &self.scene,
                        #[cfg(feature = "egui")]
                        &mut self.gui,
                    );
                    self.time.update();
                    func(events::Event::FrameUpdate, control_flow);
                }
                Event::LoopDestroyed => {
                    func(events::Event::Destroyed, control_flow);
                }
                _ => (),
            }
        });
    }
}

/// Holds the timings of the engine like runtime and delta time.
#[derive(Clone)]
pub struct Time {
    /// Time since engine start.
    pub time: SystemTime,
    delta_instant: SystemTime,
    delta_time: Arc<AtomicF64>,
}

impl Default for Time {
    fn default() -> Self {
        Self {
            time: SystemTime::now(),
            delta_instant: SystemTime::now(),
            delta_time: Arc::new(AtomicF64::new(0.0f64)),
        }
    }
}

impl Time {
    /// Updates the time data on frame redraw.
    pub(crate) fn update(&mut self) {
        self.delta_time.store(
            self.delta_instant.elapsed().unwrap().as_secs_f64(),
            Ordering::Release,
        );
        self.delta_instant = SystemTime::now();
    }

    /// Returns the time it took to execute last iteration.
    pub fn delta_time(&self) -> f64 {
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
}
