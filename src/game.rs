pub mod resources;
use resources::Resources;
pub mod objects;
use objects::Object;
mod vulkan;
use vulkan::Vulkan;
use winit::{event_loop::EventLoop, window::WindowBuilder};
mod draw;

use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct AppInfo {
    pub AppName: &'static str,
}
/// The struct that holds and executes all of the game data.
#[allow(dead_code)]
pub struct Game {
    pub objects: Vec<Arc<Object>>,
    pub resources: Resources,
    pub app_info: AppInfo,
    vulkan: Vulkan,
}

impl Game {
    pub fn init(app_info: AppInfo, window_builder: WindowBuilder) -> (Self, EventLoop<()>) {
        let (vulkan, event_loop) = Vulkan::init(window_builder, app_info);

        (
            Self {
                objects: vec![],
                resources: Resources::new(),
                app_info,
                vulkan,
            },
            event_loop,
        )
    }
}
