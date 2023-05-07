pub mod resources;
pub use resources::Resources;
use resources::{GameFont, Loader, Texture};
pub mod objects;
pub use objects::{
    data::Data, Appearance, CameraOption, CameraScaling, Layer, Node, Object, Scene,
};
pub mod vulkan;
use vulkan::Vulkan;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};
mod draw;
use draw::Draw;
mod font_layout;
use font_layout::Labelifier;
pub mod materials;
pub mod input;
pub use input::Input;

use parking_lot::Mutex;
use atomic_float::AtomicF64;

use std::{sync::{Arc, atomic::Ordering}, time::Instant};

pub use self::objects::data::Vertex;

pub type AObject = Arc<Mutex<Object>>;
pub type NObject = Arc<Mutex<Node<AObject>>>;
pub type Font = GameFont;

/// This is what you create your whole game session with.
pub struct GameBuilder {
    window_builder: Option<WindowBuilder>,
    clear_background_color: [f32; 4],
}

impl GameBuilder {
    pub fn new() -> Self {
        Self {
            window_builder: None,
            clear_background_color: [0.0; 4],
        }
    }
    pub fn with_window_builder(mut self, window_builder: WindowBuilder) -> Self {
        self.window_builder = Some(window_builder);
        self
    }
    pub fn with_clear_background_clear_color(mut self, color: [f32; 4]) -> Self {
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
        let draw = Draw::setup(&vulkan, &mut loader);
        let labelifier = Labelifier::new(&vulkan, &mut loader);

        let resources = Resources::new(
            vulkan,
            Arc::new(Mutex::new(loader)),
            Arc::new(Mutex::new(labelifier)),
        );

        (
            Game {
                scene: Scene::new(),
                resources,
                draw,
                input: Input::new(),

                time: Time::default(),
                clear_background_color,
            },
            event_loop,
        )
    }
}

/// The struct that holds and executes all of the game data.
pub struct Game {
    pub scene: Scene,
    pub resources: Resources,
    pub time: Time,
    pub input: Input,
                        

    draw: Draw,
    clear_background_color: [f32; 4],
}

impl Game {
    pub fn update<T: 'static>(&mut self, event: &Event<T>) {
        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                self.draw.recreate_swapchain = true;
            }
            Event::RedrawEventsCleared => {
                self.resources.update();
                self.draw.redrawevent(
                    &self.resources.vulkan,
                    &mut self.resources.loader.lock(),
                    &self.scene,
                    self.clear_background_color,
                );

                self.time.update();
            }
            _ => (),
        }
    }

    pub fn set_clear_background_color(&mut self, color: [f32; 4]) {
        self.clear_background_color = color;
    }
}

#[derive(Clone)]
pub struct Time {
    pub time: Instant,
    delta_instant: Instant,
    pub delta_time: Arc<AtomicF64>,
}

impl Default for Time {
    fn default() -> Self {
        Self {
            time: Instant::now(),
            delta_instant: Instant::now(),
            delta_time: Arc::new(AtomicF64::new(0.0f64)),
        }
    }
}

impl Time {
    /// Don't call this function. This is for the game struct to handle.
    pub fn update(&mut self) {
        self.delta_time.store(self.delta_instant.elapsed().as_secs_f64(), Ordering::Release);
        self.delta_instant = Instant::now();
    }
    pub fn delta_time(&self) -> f64 {
        self.delta_time.load(Ordering::Acquire)
    }
    pub fn fps(&self) -> f64 {
        1.0 / self.delta_time.load(Ordering::Acquire)
    }
    pub fn time(&self) -> f64 {
        self.time.elapsed().as_secs_f64()
    }
}
