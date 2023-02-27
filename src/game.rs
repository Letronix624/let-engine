pub mod resources;
use resources::Resources;
pub mod objects;
pub use objects::{data::Data, Object, ObjectNode, VisualObject};
pub mod vulkan;
use vulkan::Vulkan;
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};
mod draw;
use draw::Draw;

use std::sync::{Arc, Mutex};

use crate::AppInfo;

/// This is what you create your whole game session with.
pub struct GameBuilder {
    window_builder: Option<WindowBuilder>,
    app_info: Option<AppInfo>,
    //resources: Resources,
}

impl GameBuilder {
    pub fn new() -> Self {
        Self {
            window_builder: None,
            app_info: None,
            //resources: Resources::new(),
        }
    }
    // pub fn with_resources(mut self, resources: Resources) -> Self {
    //     self.resources = resources;
    //     self
    // }
    pub fn with_window_builder(mut self, window_builder: WindowBuilder) -> Self {
        self.window_builder = Some(window_builder);
        self
    }
    pub fn with_app_info(mut self, app_info: AppInfo) -> Self {
        self.app_info = Some(app_info);
        self
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
    pub objects: Vec<Arc<Mutex<ObjectNode>>>, // look here!
    resources: Resources,
    app_info: AppInfo,
    draw: Draw,
    vulkan: Vulkan,
}

impl Game {
    pub fn update(&mut self) {
        self.draw
            .redrawevent(&mut self.vulkan, &self.objects, &self.resources);
    }
    pub fn recreate_swapchain(&mut self) {
        self.draw.recreate_swapchain = true;
    }
    pub fn load_font_bytes(&mut self, name: &str, data: &[u8], size: f32, characters: Vec<char>) {
        self.resources.add_font_bytes(name, size, data, characters);
        self.draw
            .update_font_objects(&mut self.vulkan, &mut self.resources);
    }
    pub fn unload_font(&mut self, name: &str) {
        self.resources.remove_font(name);
        self.draw
            .update_font_objects(&mut self.vulkan, &mut self.resources);
    }
    pub fn load_sound(&mut self, name: &str, sound: &[u8]) {
        self.resources.add_sound(name, sound);
    }
    pub fn load_texture(&mut self, name: &str, texture: Vec<u8>, width: u32, height: u32) {
        self.resources.add_texture(name, texture, width, height);
        self.draw.update_textures(&self.vulkan, &self.resources);
    }
    pub fn get_window(&self) -> &Window {
        self.vulkan.surface
            .object()
            .unwrap()
            .downcast_ref::<Window>()
            .unwrap()
    }
}
