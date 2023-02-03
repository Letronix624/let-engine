pub mod data;
use data::*;
use std::sync::{Arc, Mutex};

/// Main game object that holds position, size, rotation, color, texture and data.
/// To make your objects appear take an empty object, add your traits and send an receiver
/// of it to the main game object.
#[derive(Clone, Debug)]
pub struct Object {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub rotation: f32,
    pub color: [f32; 4],
    pub graphics: Option<VisualObject>,
}
//game objects have position, size, rotation, color texture and data.
//text objects have position, size, rotation, color, text and font.
impl Object {
    pub fn new() -> Self {
        Self {
            position: [0.0, 0.0],
            size: [1.0, 1.0],
            rotation: 0.0,
            color: [0.0, 0.0, 0.0, 1.0],
            graphics: None,
        }
    }
    pub fn new_square() -> Self {
        Self {
            position: [0.0, 0.0],
            size: [0.5, 0.5],
            rotation: 0.0,
            color: [0.0, 0.0, 0.0, 1.0],
            graphics: Some(VisualObject::new(Display::Data)),
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

pub struct ObjectNode {
    pub object: Object,
    pub children: Vec<Arc<Mutex<ObjectNode>>>,
}

impl ObjectNode {
    pub fn new(object: Object, children: Vec<Arc<Mutex<ObjectNode>>>) -> Self {
        Self { object, children }
    }
    pub fn order_position(order: &mut Vec<Object>, objects: &Arc<Mutex<Self>>) {
        let objects = objects.lock().unwrap();
        for child in objects.children.clone() {
            let child = child.lock().unwrap();
            let object = objects.object.clone() + child.object.clone();
            order.push(object.clone());
            for child in child.children.clone() {
                order.push(object.clone() + child.lock().unwrap().object.clone());
                Self::order_position(order, &child.clone());
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct VisualObject {
    pub texture: Option<String>,
    pub data: Data,
    pub text: Option<String>,
    pub font: Option<String>,
    pub display: Display,
}
impl VisualObject {
    pub fn empty() -> Self {
        Self {
            texture: None,
            data: Data::empty(),
            text: None,
            font: None,
            display: Display::Data,
        }
    }
    pub fn new(display: Display) -> Self {
        Self {
            display: display,
            ..Self::empty()
        }
    }
    pub fn new_square() -> Self {
        Self {
            data: Data::square(),
            ..Self::empty()
        }
    }
    pub fn new_text(text: &str, font: &str) -> Self {
        Self {
            text: Some(text.to_string()),
            font: Some(font.to_string()),
            display: Display::Labeled,
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
    pub fn text(mut self, text: &str) -> Self {
        self.text = Some(text.to_string());
        self
    }
    pub fn font(mut self, font: &str) -> Self {
        self.font = Some(font.to_string());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Display {
    Data,
    Labeled,
}
