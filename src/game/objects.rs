pub mod data;
use super::resources::*;
use crate::error::textures::*;
use anyhow::Result;
pub use data::*;
use hashbrown::HashMap;
use parking_lot::Mutex;
use std::{
    default,
    sync::{Arc, Weak},
};

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

impl Object {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn new_square() -> Self {
        Self {
            size: [0.5, 0.5],
            graphics: Some(Appearance::new_square()),
            ..Default::default()
        }
    }
    pub fn graphics(mut self, graphics: Option<Appearance>) -> Self {
        self.graphics = graphics;
        self
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

impl Default for Object {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            size: [1.0, 1.0],
            rotation: 0.0,
            graphics: None,
            camera: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraOption {
    pub zoom: f32,
    pub mode: CameraScaling,
}

impl CameraOption {
    pub fn new() -> Self {
        // Best for in-game scenes
        Self {
            zoom: 1.0,
            mode: CameraScaling::Circle,
        }
    }
    pub fn new_hud() -> Self {
        // Best for huds menus screen savers and consistant things.
        Self {
            zoom: 1.0,
            mode: CameraScaling::Expand,
        }
    }
}

impl std::default::Default for CameraOption {
    fn default() -> Self {
        Self::new()
    }
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
            order.push(object.clone());
            for child in child.children.iter() {
                let child = child.lock();
                order.push(object.clone() + child.object.lock().clone());
                Self::order_position(order, &*child);
            }
        }
    }
    pub fn remove_child(&mut self, object: &Arc<Mutex<Node<Arc<Mutex<Object>>>>>) {
        let index = self
            .children
            .clone()
            .into_iter()
            .position(|x| Arc::as_ptr(&x) == Arc::as_ptr(&object))
            .unwrap();
        self.children.remove(index.clone());
    }
    pub fn remove_children(
        &mut self,
        objects: &mut HashMap<*const Mutex<Object>, Arc<Mutex<Node<Arc<Mutex<Object>>>>>>,
    ) {
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
        } else {
            self.object.lock().clone()
        }
    }
    #[allow(dead_code)]
    pub fn print_tree(&self, indent_level: usize) {
        let indent = "  ".repeat(indent_level);
        println!("{}{:?}", indent, Arc::as_ptr(&self.object));
        for child in &self.children {
            child.lock().print_tree(indent_level + 1);
        }
    }
}

/// Holds everything about the appearance of objects like
/// textures, vetex/index data, color and material.
#[derive(Debug, Clone, PartialEq)]
pub struct Appearance {
    pub texture: Option<Arc<Texture>>,
    pub texture_id: u32,
    pub data: Data,
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub rotation: f32,
    pub color: [f32; 4],
}
impl Appearance {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
    pub fn new_square() -> Self {
        Self {
            data: Data::square(),
            ..Default::default()
        }
    }
    pub fn new_color(color: [f32; 4]) -> Self {
        Self {
            color,
            ..Default::default()
        }
    }
    pub fn texture(mut self, texture: &Arc<Texture>) -> Self {
        self.texture = Some(texture.clone());
        self
    }
    pub fn texture_id(&mut self, id: u32) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(texture) = &self.texture {
            if id > texture.frames - 1 {
                return Err(Box::new(TextureIDError));
            }
        } else {
            return Err(Box::new(NoTextureError));
        }
        self.texture_id = id;
        Ok(())
    }
    pub fn data(mut self, data: Data) -> Self {
        self.data = data;
        self
    }
    pub fn position(mut self, position: [f32; 2]) -> Self {
        self.position = position;
        self
    }
    pub fn size(mut self, size: [f32; 2]) -> Self {
        self.size = size;
        self
    }
    pub fn rotation(mut self, angle: f32) -> Self {
        self.rotation = angle;
        self
    }
    pub fn color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }
    pub fn get_texture_id(&self) -> u32 {
        self.texture_id
    }
    pub fn next_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(texture) = &self.texture {
            if texture.frames <= self.texture_id + 1 {
                return Err(Box::new(TextureIDError));
            }
        } else {
            return Err(Box::new(NoTextureError));
        }
        self.texture_id += 1;
        Ok(())
    }
}

impl default::Default for Appearance {
    fn default() -> Self {
        Self {
            texture: None,
            texture_id: 0,
            data: Data::empty(),
            position: [0.0; 2],
            size: [1.0; 2],
            rotation: 0.0,
            color: [0.0, 0.0, 0.0, 1.0],
        }
    }
}
