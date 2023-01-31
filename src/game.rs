pub mod resources;
use resources::Resources;
pub mod objects;
use objects::Object;
pub mod vulkan;
use vulkan::Vulkan;
use winit::{event_loop::EventLoop, window::{WindowBuilder, Window}};
mod draw;
use draw::Draw;

use crate::AppInfo;
use std::sync::Arc;

/// The struct that holds and executes all of the game data.
#[allow(dead_code)]
pub struct Game {
    pub objects: Vec<Arc<Object>>,
    pub resources: Resources,
    pub app_info: AppInfo,
    draw: Draw,
    vulkan: Vulkan,
}

impl Game {
    pub fn init(app_info: AppInfo, window_builder: WindowBuilder) -> (Self, EventLoop<()>) {
        let resources = Resources::new();
        let (vulkan, event_loop) = Vulkan::init(window_builder, app_info);
        let draw = Draw::setup(&vulkan, &resources);

        vulkan.surface.object().unwrap().downcast_ref::<Window>().unwrap().set_visible(true);
        
        (
            Self {
                objects: vec![],
                resources,
                app_info,
                vulkan,
                draw
            },
            event_loop,
        )
    }
    pub fn update(&mut self) {
        self.draw.redrawevent(&mut self.vulkan, &self.objects);
    }
}
