pub mod resources;
use resources::Resources;
pub mod objects;
use objects::Object;
pub mod vulkan;
use vulkan::Vulkan;
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};
mod draw;
use draw::Draw;

use crate::AppInfo;
use std::sync::mpsc::Receiver;

pub struct GameBuilder {
    window_builder: Option<WindowBuilder>,
    app_info: Option<AppInfo>,
}

/// This is what you create your whole game session with.
impl GameBuilder {
    pub fn new() -> Self {
        Self {
            window_builder: None,
            app_info: None,
        }
    }
    pub fn with_window_builder(&mut self, window_builder: WindowBuilder) {
        self.window_builder = Some(window_builder);
    }
    pub fn build(&mut self) -> (Game, EventLoop<()>) {
        let app_info = if let Some(app_info) = self.app_info {
            app_info
        } else {
            panic!("No app info");
        };

        let window_builder = if let Some(window_builder) = self.window_builder.clone() {
            window_builder
        } else {
            panic!("no window builder");
        };

        let resources = Resources::new();
        let (vulkan, event_loop) = Vulkan::init(window_builder, app_info);
        let draw = Draw::setup(&vulkan, &resources);

        vulkan
            .surface
            .object()
            .unwrap()
            .downcast_ref::<Window>()
            .unwrap()
            .set_visible(true);

        (
            Game {
                objects: vec![],
                resources,
                app_info,
                vulkan,
                draw,
            },
            event_loop,
        )
    }
}

/// The struct that holds and executes all of the game data.
#[allow(dead_code)]
pub struct Game {
    pub objects: Vec<Receiver<Object>>,
    pub resources: Resources,
    pub app_info: AppInfo,
    draw: Draw,
    vulkan: Vulkan,
}

impl Game {
    pub fn update(&mut self) {
        self.draw.redrawevent(&mut self.vulkan, &self.objects);
    }
    pub fn recreate_swapchain(&mut self) {
        self.draw.recreate_swapchain = true;
    }
}
