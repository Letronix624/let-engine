pub mod resources;
use resources::{Loader, Resources};
pub mod objects;
use objects::Node;
pub use objects::{physics, GameObject, Layer, Scene, Transform};
pub mod camera;
pub mod vulkan;
use vulkan::Vulkan;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};
mod draw;
use draw::Draw;
use objects::labels::Labelifier;
pub mod input;
pub use input::Input;
pub use resources::materials;
#[cfg(feature = "egui")]
pub mod egui;

use atomic_float::AtomicF64;
pub use engine_macros;
use parking_lot::Mutex;

use std::{
    sync::{atomic::Ordering, Arc, Weak},
    time::SystemTime,
};

pub use objects::data;
pub use objects::data::{tvert, vert, Vertex};

pub type AObject = Box<dyn GameObject>;
pub type NObject = Arc<Mutex<Node<AObject>>>;
pub type WeakObject = Weak<Mutex<Node<AObject>>>;

/// This is what you create your whole game session with.
pub struct GameBuilder {
    window_builder: Option<WindowBuilder>,
    clear_background_color: [f32; 4],
}

impl GameBuilder {
    /// Makes a new game builder.
    pub fn new() -> Self {
        Self {
            window_builder: None,
            clear_background_color: [0.0; 4],
        }
    }
    /// Adds a window builder.
    pub fn with_window_builder(mut self, window_builder: WindowBuilder) -> Self {
        self.window_builder = Some(window_builder);
        self
    }
    /// Adds a clear color.
    pub fn with_clear_color(mut self, color: [f32; 4]) -> Self {
        self.clear_background_color = color;
        self
    }
    pub fn build(&mut self) -> (Game, EventLoop<()>) {
        let window_builder = if let Some(window_builder) = self.window_builder.clone() {
            window_builder
        } else {
            panic!("no window builder");
        };

        let clear_background_color = self.clear_background_color;

        let (vulkan, event_loop) = Vulkan::init(window_builder);
        let mut loader = Loader::init(&vulkan);
        #[cfg(feature = "egui")]
        let gui = egui::init(&event_loop, &vulkan);
        let draw = Draw::setup(&vulkan, &loader);
        let labelifier = Labelifier::new(&vulkan, &mut loader);

        let resources = Resources::new(
            vulkan,
            Arc::new(Mutex::new(loader)),
            Arc::new(Mutex::new(labelifier)),
        );

        (
            Game {
                scene: Scene::default(),
                resources,
                draw,
                input: Input::default(),
                #[cfg(feature = "egui")]
                gui,
                #[cfg(feature = "egui")]
                gui_updated: false,

                time: Time::default(),
                clear_background_color,
            },
            event_loop,
        )
    }
}
impl Default for GameBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// The struct that holds and executes all of the game data.
pub struct Game {
    pub scene: Scene,
    pub resources: Resources,
    pub time: Time,
    pub input: Input,
    #[cfg(feature = "egui")]
    gui: egui_winit_vulkano::Gui,
    #[cfg(feature = "egui")]
    gui_updated: bool,

    draw: Draw,
    clear_background_color: [f32; 4],
}

impl Game {
    /// Renders the font and draws the image of the window.
    pub fn update<T: 'static>(&mut self, event: &Event<T>) {
        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                self.draw.recreate_swapchain = true;
            }
            #[cfg(feature = "egui")]
            Event::WindowEvent { event, .. } => {
                self.gui.update(event);
            }
            Event::RedrawEventsCleared => {
                #[cfg(feature = "egui")]
                {
                    if !self.gui_updated {
                        self.gui.immediate_ui(|_gui| {});
                    }
                    self.gui_updated = false;
                }

                self.resources.update();
                self.draw.redrawevent(
                    &self.resources.vulkan,
                    &mut self.resources.loader.lock(),
                    &self.scene,
                    self.clear_background_color,
                    #[cfg(feature = "egui")]
                    &mut self.gui,
                );

                self.time.update();
            }
            _ => (),
        }
        self.input
            .update(event, self.resources.get_window().inner_size());
    }

    /// Updates the Egui gui.
    #[cfg(feature = "egui")]
    pub fn update_gui(&mut self, func: impl FnOnce(egui_winit_vulkano::egui::Context)) {
        self.gui.immediate_ui(|gui| {
            func(gui.context());
        });
        self.gui_updated = true;
    }

    /// Sets the background color to clear the screen with.
    pub fn set_clear_background_color(&mut self, color: [f32; 4]) {
        self.clear_background_color = color;
    }
}

/// Holds the timings of the engine like runtime and delta time.
#[derive(Clone)]
pub struct Time {
    pub time: SystemTime,
    delta_instant: SystemTime,
    pub delta_time: Arc<AtomicF64>,
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
