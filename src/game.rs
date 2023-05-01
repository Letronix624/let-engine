pub mod resources;
use anyhow::Result;
use hashbrown::HashMap;
use resources::{GameFont, Resources, Texture};
pub mod objects;
pub use objects::{data::Data, Appearance, CameraOption, CameraScaling, Node, Object};
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
use parking_lot::Mutex;
use std::{sync::Arc, time::Instant};

use crate::error::objects::*;

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
        let mut draw = Draw::setup(&vulkan);
        let labelifier = Labelifier::new(&vulkan, &mut draw);

        let resources = Resources::new(
            vulkan,
            Arc::new(Mutex::new(draw)),
            Arc::new(Mutex::new(labelifier)),
        );

        (
            Game {
                objects: vec![],
                objects_map: HashMap::new(),
                resources,

                time: Instant::now(),
                delta_instant: Instant::now(),
                delta_time: 0.0,
                clear_background_color,
            },
            event_loop,
        )
    }
}

/// The struct that holds and executes all of the game data.
#[allow(dead_code)]
pub struct Game {
    objects: Vec<(NObject, Option<Arc<Mutex<Node<AObject>>>>)>,
    objects_map: HashMap<*const Mutex<Object>, NObject>,
    pub resources: Resources,

    time: Instant,
    delta_instant: Instant,
    delta_time: f64,
    clear_background_color: [f32; 4],
}

impl Game {
    pub fn update<T: 'static>(&mut self, event: &Event<T>) {
        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                self.resources.recreate_swapchain();
            }
            Event::RedrawEventsCleared => {
                self.resources.redraw(&self.objects, self.clear_background_color);
                self.delta_time = self.delta_instant.elapsed().as_secs_f64();
                self.delta_instant = Instant::now();
            }
            _ => (),
        }
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





    
    
    pub fn load_font(&mut self, data: &[u8]) -> Arc<Font> {
        self.resources.load_font(data)
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
        let node: NObject;
        if let Some(obj) = self.objects_map.remove(&Arc::as_ptr(object)) {
            node = obj.clone();
        } else {
            return Err(NoObjectError);
        }
        let mut objectguard = node.lock();

        objectguard.remove_children(&mut self.objects_map);

        if let Some(parent) = &objectguard.parent {
            let parent = parent.clone().upgrade().unwrap();
            let mut parent = parent.lock();
            parent.remove_child(&node);
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
    pub fn set_clear_background_color(&mut self, color: [f32; 4]) {
        self.clear_background_color = color;
    }
}

