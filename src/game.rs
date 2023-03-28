pub mod resources;
use anyhow::Result;
use hashbrown::HashMap;
use resources::Resources;
pub mod objects;
pub use objects::{data::Data, Appearance, CameraOption, CameraScaling, Node, Object};
pub mod vulkan;
use vulkan::Vulkan;
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};
mod draw;
use draw::Draw;
mod font_layout;

use parking_lot::Mutex;
use std::{sync::Arc, time::Instant};

use crate::{errors::*, AppInfo};

pub use self::objects::data::Vertex;

type AObject = Arc<Mutex<Object>>;

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
        let draw = Draw::setup(&vulkan);

        (
            Game {
                objects: vec![],
                objects_map: HashMap::new(),
                resources,

                time: Instant::now(),
                delta_instant: Instant::now(),
                delta_time: 0.0,

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
    //camera layers here with each object as a child of a specific layer.
    objects: Vec<(Arc<Mutex<Node<AObject>>>, Option<Arc<Mutex<Node<AObject>>>>)>,
    objects_map: HashMap<*const Mutex<Object>, Arc<Mutex<Node<AObject>>>>,
    resources: Resources,

    time: Instant,
    delta_instant: Instant,
    delta_time: f64,

    app_info: AppInfo,
    draw: Draw,
    vulkan: Vulkan,
}

/* notes
One main camera for everything.
Hud as children of the camera object.

Multiple camera layers with different positions rotations or scaling modes.
Layer struct with camera settings.

*/

impl Game {
    pub fn update(&mut self) {
        self.draw
            .redrawevent(&mut self.vulkan, self.objects.clone());
        self.delta_time = self.delta_instant.elapsed().as_secs_f64();
        self.delta_instant = Instant::now();
    }
    pub fn recreate_swapchain(&mut self) {
        self.draw.recreate_swapchain = true;
    }
    pub fn new_layer(&mut self) -> AObject {
        let object = Arc::new(Mutex::new(Object::new()));

        let node = Arc::new(Mutex::new(Node {
            object: object.clone(),
            parent: None,
            children: vec![],
        }));
        self.objects.push((node.clone(), None));

        self.objects_map.insert(Arc::as_ptr(&object), node.clone());
        object
    }
    pub fn set_camera(
        &mut self,
        layer: &AObject,
        camera: &AObject,
    ) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut camera = camera.lock();

            if let None = camera.camera {
                camera.camera = Some(CameraOption::new())
            }
        }

        if let Some(layer) = self.objects_map.get(&Arc::as_ptr(layer)) {
            if let Some(index) = self.objects.iter().position(|x| Arc::ptr_eq(&x.0, layer)) {
                if let Some(camera) = self.objects_map.get(&Arc::as_ptr(camera)) {
                    self.objects[index].1 = Some(camera.clone())
                } else {
                    return Err(Box::new(NoObjectError));
                }
            }
        } else {
            return Err(Box::new(NoLayerError));
        }

        Ok(())
    }
    pub fn add_object(
        &mut self,
        parent: &AObject,
        initial_object: Object,
    ) -> Result<AObject, NoParentError> {
        let object = Arc::new(Mutex::new(initial_object));

        let parent = if let Some(parent) = self.objects_map.get(&Arc::as_ptr(parent)) {
            parent.clone()
        } else {
            return Err(NoParentError);
        };

        let node = Arc::new(Mutex::new(Node {
            object: object.clone(),
            parent: Some(Arc::downgrade(&parent)),
            children: vec![],
        }));

        parent.lock().children.push(node.clone());

        self.objects_map.insert(Arc::as_ptr(&object), node);
        Ok(object)
    }
    pub fn contains_object(&self, object: &AObject) -> bool {
        self.objects_map.contains_key(&Arc::as_ptr(object))
    }
    pub fn remove_object(&mut self, object: &AObject) -> Result<(), NoObjectError> {
        let node: Arc<Mutex<Node<AObject>>>;
        if let Some(obj) = self.objects_map.remove(&Arc::as_ptr(object)) {
            node = obj.clone();
        } else {
            return Err(NoObjectError);
        }
        let objectguard = node.lock();
        if let Some(parent) = &objectguard.parent {
            let parent = parent.clone().upgrade().unwrap();

            parent.lock().remove_child(&node, &mut self.objects_map);
        } else {
            if let Some(index) = self
                .objects
                .clone()
                .into_iter()
                .position(|x| Arc::ptr_eq(&x.0, &node))
            {
                self.objects.remove(index);
            }
        }
        Ok(())
    }

    pub fn time(&self) -> f64 {
        self.time.elapsed().as_secs_f64()
    }

    pub fn delta_time(&self) -> f64 {
        self.delta_time
    }

    pub fn fps(&self) -> f64 {
        1.0 / self.delta_time
    }

    pub fn load_font_bytes(&mut self, name: &str, data: &[u8]) {
        self.resources.add_font_bytes(name, data);
    }
    pub fn unload_font(&mut self, name: &str) {
        self.resources.remove_font(name);
    }
    pub fn unload_texture(&mut self, name: &str) {
        self.resources.remove_texture(name);
    }
    pub fn unload_sound(&mut self, name: &str) {
        self.resources.remove_sound(name);
    }
    pub fn load_sound(&mut self, name: &str, sound: &[u8]) {
        self.resources.add_sound(name, sound);
    }
    pub fn load_texture(&mut self, name: &str, texture: Vec<u8>, width: u32, height: u32) {
        self.resources.add_texture(name, texture, width, height);
        self.draw.update_textures(&self.vulkan, &self.resources);
    }
    pub fn get_window(&self) -> &Window {
        self.vulkan
            .surface
            .object()
            .unwrap()
            .downcast_ref::<Window>()
            .unwrap()
    }
    pub fn label(
        &mut self,
        object: &AObject,
        font: &str,
        text: &str,
        scale: f32,
        binding: [f32; 2],
    ) {
        let mut object = object.lock();
        if let Some(mut appearance) = object.graphics.as_mut() {
            let data = font_layout::get_data(self, font, text, scale, appearance.size, binding);
            appearance.texture = Some("fontatlas".to_string());
            appearance.data = data;
            appearance.material = 2;
        } else {
            let data = font_layout::get_data(self, font, text, scale, [1.0; 2], binding);
            object.graphics = Some(Appearance {
                texture: Some("fontatlas".to_string()),
                data,
                material: 2,
                color: [1.0; 4],
                ..Default::default()
            });
        }
    }
}
