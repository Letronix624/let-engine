pub mod data;
pub use data::*;
use parking_lot::Mutex;
use hashbrown::HashMap;
use std::sync::{Arc, Weak};

/// Main game object that holds position, size, rotation, color, texture and data.
/// To make your objects appear take an empty object, add your traits and send an receiver
/// of it to the main game object.
#[derive(Clone, Debug, PartialEq)]
pub struct Object {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub rotation: f32,
    pub graphics: Option<Appearance>,
    pub camera: Option<CameraOption>,
}
//game objects have position, size, rotation, color texture and data.
//text objects have position, size, rotation, color, text, font and font size.
impl Object {
    pub fn new() -> Self {
        Self {
            position: [0.0, 0.0],
            size: [1.0, 1.0],
            rotation: 0.0,
            graphics: None,
            camera: None
        }
    }
    pub fn new_square() -> Self {
        Self {
            position: [0.0, 0.0],
            size: [0.5, 0.5],
            rotation: 0.0,
            graphics: Some(Appearance::new_square()),
            camera: None
        }
    }
}

impl std::ops::Add for Object {
    type Output = Object;

    fn add(self, rhs: Self) -> Self::Output {
        let position: Vec<f32> = self
            .position
            .clone()
            .iter()
            .zip(rhs.position.clone())
            .map(|(a, b)| a + b)
            .collect();
        let size: Vec<f32> = self
            .size
            .clone()
            .iter()
            .zip(rhs.size.clone())
            .map(|(a, b)| a * b)
            .collect();
        let rotation = self.rotation + rhs.rotation;

        Self {
            position: [position[0], position[1]],
            size: [size[0], size[1]],
            rotation,
            ..rhs.clone()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraOption {
    pub zoom: f32,
    pub mode: CameraScaling
}

impl CameraOption {
    pub fn new() -> Self { // Best for in-game scenes
        Self {
            zoom: 1.0,
            mode: CameraScaling::Circle
        }
    }
    pub fn new_hud() -> Self { // Best for huds menus screen savers and consistant things.
        Self {
            zoom: 1.0,
            mode: CameraScaling::Expand
        }
    }
}

impl std::default::Default for CameraOption {
    fn default() -> Self { Self::new() }
}

pub struct Node<T> {
    pub object: T,
    pub parent: Option<Weak<Mutex<Node<T>>>>,
    pub children: Vec<Arc<Mutex<Node<T>>>>,
}

impl Node<Arc<Mutex<Object>>> {
    pub fn order_position(order: &mut Vec<Object>, objects: &Self) {
        for child in objects.children.iter() {
            let child = child.lock();
            let object = objects.object.lock().clone() + child.object.lock().clone();
            //objects.object.clone() + child.object.clone();
            order.push(object.clone());
            for child in child.children.iter() {
                let child = child.lock();
                order.push(object.clone() + child.object.lock().clone());
                Self::order_position(order, &*child);
            }
        }
    }
    pub fn remove_child(&mut self, object: &Arc<Mutex<Node<Arc<Mutex<Object>>>>>, objects: &mut HashMap<*const Mutex<Object>, Arc<Mutex<Node<Arc<Mutex<Object>>>>>>) {
        let index = self
            .children
            .clone()
            .into_iter()
            .position(|x| Arc::as_ptr(&x) == Arc::as_ptr(&object))
            .unwrap();
        self.children[index.clone()].lock().remove_children(objects);
        self.children.remove(index.clone());
    }
    pub fn remove_children(&mut self, objects: &mut HashMap<*const Mutex<Object>, Arc<Mutex<Node<Arc<Mutex<Object>>>>>>) {
        for child in self.children.iter() {
            child.clone().lock().remove_children(objects);
        }
        objects.remove(&Arc::as_ptr(&self.object));
        self.children = vec![];
    }
    pub fn get_object(&self) -> Object {
        if let Some(parent) = &self.parent {
            let parent = parent.upgrade().unwrap();
            let parent = parent.lock();
            parent.get_object() + self.object.lock().clone()
        }
        else {
            self.object.lock().clone()
        }
    }
}

/// Holds everything about the appearance of objects like
/// textures, vetex/index data, color and material.
#[derive(Debug, Clone, PartialEq)]
pub struct Appearance {
    pub texture: Option<String>,
    pub data: Data,
    pub color: [f32; 4],
    pub material: u32,
}
impl Appearance {
    pub fn empty() -> Self {
        Self {
            texture: None,
            data: Data::empty(),
            color: [0.0, 0.0, 0.0, 1.0],
            material: 0,
        }
    }
    pub fn new() -> Self {
        Self { ..Self::empty() }
    }
    pub fn new_square() -> Self {
        Self {
            data: Data::square(),
            ..Self::empty()
        }
    }
    pub fn texture(mut self, texture: &str) -> Self {
        self.texture = Some(texture.to_string());
        self
    }
    pub fn data(mut self, data: Data) -> Self {
        self.data = data;
        self
    }
    pub fn color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }
    pub fn material(mut self, material: u32) -> Self {
        self.material = material;
        self
    }
}

//fn textdata() -> ()
