pub mod data;
use super::{materials, AObject, NObject};
use crate::error::objects::*;
use crate::error::textures::*;
use anyhow::Result;
pub use data::*;
use hashbrown::HashMap;
use indexmap::{indexset, IndexSet};
use parking_lot::Mutex;
use std::{
    default,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Weak,
    },
};

type ObjectsMap = HashMap<u64, NObject>;

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
    id: u64,
}

impl Object {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn new_square() -> Self {
        Self {
            graphics: Some(Appearance::new_square()),
            ..Default::default()
        }
    }
    pub fn graphics(mut self, graphics: Option<Appearance>) -> Self {
        self.graphics = graphics;
        self
    }
    pub fn get_id(&self) -> u64 {
        self.id
    }
    pub fn initialize(mut self, id: u64) -> Self {
        self.id = id;
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
            .zip(rhs.position)
            .map(|(a, b)| a + b)
            .collect();
        #[allow(clippy::suspicious_arithmetic_impl)]
        let size: Vec<f32> = self
            .size
            .clone()
            .iter()
            .zip(rhs.size)
            .map(|(a, b)| a * b)
            .collect();
        let rotation = self.rotation + rhs.rotation;

        Self {
            position: [position[0], position[1]],
            size: [size[0], size[1]],
            rotation,
            ..rhs
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
            id: 0,
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
                Self::order_position(order, &child);
            }
        }
    }
    pub fn remove_child(&mut self, object: &NObject) {
        let index = self
            .children
            .clone()
            .into_iter()
            .position(|x| Arc::ptr_eq(&x, object))
            .unwrap();
        self.children.remove(index);
    }
    pub fn remove_children(&mut self, objects: &mut ObjectsMap) {
        for child in self.children.iter() {
            child.clone().lock().remove_children(objects);
        }
        objects.remove(&self.object.lock().get_id());
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
    pub material: Option<materials::Material>,
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
    pub fn auto_scale(&mut self) -> Result<(), NoTextureError> {
        let dimensions;
        if let Some(material) = &self.material {
            dimensions = if let Some(texture) = &material.texture {
                texture.dimensions
            } else {
                return Err(NoTextureError);
            };
        } else {
            return Err(NoTextureError);
        };

        self.size = [dimensions.0 as f32 / 1000.0, dimensions.1 as f32 / 1000.0];

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
    pub fn material(mut self, material: materials::Material) -> Self {
        self.material = Some(material);
        self
    }
}

impl default::Default for Appearance {
    fn default() -> Self {
        Self {
            material: None,
            data: Data::empty(),
            position: [0.0; 2],
            size: [1.0; 2],
            rotation: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

#[derive(Clone)]
pub struct Layer {
    pub root: NObject,
    pub camera: Arc<Mutex<Option<NObject>>>,
    objects_map: Arc<Mutex<ObjectsMap>>,
    latest_object: Arc<AtomicU64>,
}

impl Layer {
    pub fn new(root: NObject) -> Self {
        Self {
            root,
            camera: Arc::new(Mutex::new(None)),
            objects_map: Arc::new(Mutex::new(HashMap::new())),
            latest_object: Arc::new(AtomicU64::new(0)),
        }
    }
    pub fn set_camera(&self, camera: &AObject) -> Result<(), NoObjectError> {
        {
            let mut camera = camera.lock();

            if camera.camera.is_none() {
                camera.camera = Some(CameraOption::new());
            }
        }
        let map = self.objects_map.lock();
        if let Some(camera) = map.get(&camera.lock().get_id()) {
            *self.camera.lock() = Some(camera.clone());
        } else {
            return Err(NoObjectError);
        }

        Ok(())
    }
    pub fn camera_position(&self) -> [f32; 2] {
        let camera = self.camera.lock();
        if let Some(camera) = camera.clone() {
            camera.lock().get_object().position
        } else {
            [0.0; 2]
        }
    }
    pub fn camera_scaling(&self) -> CameraScaling {
        let camera = self.camera.lock();
        if let Some(camera) = camera.clone() {
            let camera = camera.lock().get_object().camera;
            camera.unwrap().mode
        } else {
            CameraScaling::Stretch
        }
    }

    pub fn side_to_world(&self, direction: [f32; 2], dimensions: (f32, f32)) -> [f32; 2] {
        let camera = Self::camera_position(self);
        let direction = [direction[0] * 2.0 - 1.0, direction[1] * 2.0 - 1.0];
        let (width, height) = scale(Self::camera_scaling(self), dimensions);
        [
            direction[0] * width + camera[0] * 2.0,
            direction[1] * height + camera[1] * 2.0,
        ]
    }

    pub fn contains_object(&self, object: &AObject) -> bool {
        self.objects_map
            .lock()
            .contains_key(&object.lock().get_id())
    }

    pub fn add_object(
        &self,
        parent: Option<&AObject>,
        initial_object: Object,
    ) -> Result<AObject, NoParentError> {
        let id = self.latest_object.fetch_add(1, Ordering::AcqRel);

        let object = Arc::new(Mutex::new(initial_object.initialize(id)));

        let mut map = self.objects_map.lock();

        let parent: NObject = if let Some(parent) = parent {
            if let Some(parent) = map.get(&parent.lock().get_id()) {
                parent.clone()
            } else {
                return Err(NoParentError);
            }
        } else {
            self.root.clone()
        };

        let node = Arc::new(Mutex::new(Node {
            object: object.clone(),
            parent: Some(Arc::downgrade(&parent)),
            children: vec![],
        }));

        parent.lock().children.push(node.clone());

        map.insert(id, node);
        Ok(object)
    }

    pub fn remove_object(&self, object: &AObject) -> Result<(), NoObjectError> {
        let node;
        let mut map = self.objects_map.lock();
        if let Some(object) = map.remove(&object.lock().get_id()) {
            node = object;
        } else {
            return Err(NoObjectError);
        };

        let mut object = node.lock();
        object.remove_children(&mut map);

        let parent = object.parent.clone().unwrap().upgrade().unwrap();
        let mut parent = parent.lock();
        parent.remove_child(&node);

        Ok(())
    }

    pub fn move_to(
        &self,
        object: &AObject,
        index: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let node = Self::to_node(self, object)?;
        let count = Self::count_children(&node);

        if count > index {
            return Err(Box::new(MoveError));
        } else {
            Self::move_object_to(node, index);
        }
        Ok(())
    }

    pub fn move_down(&self, object: &AObject) -> Result<(), Box<dyn std::error::Error>> {
        //MoveError> {
        let node = Self::to_node(self, object)?;
        let parent = Self::get_parent(&node);
        let index = Self::find_child_index(&parent, &node);
        if index == 0 {
            return Err(Box::new(MoveError));
        } else {
            Self::move_object_to(node, index - 1);
        }
        Ok(())
    }

    pub fn move_up(&self, object: &AObject) -> Result<(), Box<dyn std::error::Error>> {
        let node = Self::to_node(self, object)?;
        let parent = Self::get_parent(&node);
        let count = Self::count_children(&node);
        let index = Self::find_child_index(&parent, &node);
        if count == index {
            return Err(Box::new(MoveError));
        } else {
            Self::move_object_to(node, count + 1);
        }
        Ok(())
    }

    pub fn move_to_bottom(&self, object: &AObject) -> Result<(), NoObjectError> {
        let node = Self::to_node(self, object)?;
        Self::move_object_to(node, 0);
        Ok(())
    }

    pub fn move_to_top(&self, object: &AObject) -> Result<(), NoObjectError> {
        let node = Self::to_node(self, object)?;
        let count = Self::count_children(&node);
        Self::move_object_to(node, count);
        Ok(())
    }

    fn get_parent(object: &NObject) -> NObject {
        object.lock().parent.clone().unwrap().upgrade().unwrap()
    }

    fn find_child_index(parent: &NObject, object: &NObject) -> usize {
        let parent = parent.lock();
        parent
            .children
            .clone()
            .into_iter()
            .position(|x| Arc::ptr_eq(&x, object))
            .unwrap()
    }

    fn count_children(object: &NObject) -> usize {
        let parent = Self::get_parent(object);
        let parent = parent.lock();
        parent.children.len()
    }

    fn to_node(&self, object: &AObject) -> Result<NObject, NoObjectError> {
        let map = self.objects_map.lock();
        if let Some(object) = map.get(&object.lock().get_id()) {
            Ok(object.clone())
        } else {
            Err(NoObjectError)
        }
    }

    fn move_object_to(src: NObject, dst: usize) {
        let parent = src.lock().parent.clone().unwrap().upgrade().unwrap();
        let mut parent = parent.lock();
        let index = parent
            .children
            .clone()
            .into_iter()
            .position(|x| Arc::ptr_eq(&x, &src))
            .unwrap();
        parent.children.swap(index, dst);
    }

    pub fn children_count(&self, parent: &AObject) -> Result<usize, NoObjectError> {
        let node = Self::to_node(self, parent)?;
        Ok(Self::count_children(&node))
    }
}

impl PartialEq for Layer {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.root, &other.root)
            && Arc::ptr_eq(&self.camera, &other.camera)
            && Arc::ptr_eq(&self.objects_map, &other.objects_map)
    }
}

impl Eq for Layer {}

impl std::hash::Hash for Layer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.root).hash(state);
        Arc::as_ptr(&self.camera).hash(state);
        Arc::as_ptr(&self.objects_map).hash(state);
    }
}

pub struct Scene {
    layers: Arc<Mutex<IndexSet<Layer>>>,
}

impl Scene {
    pub fn new_layer(&self) -> Layer {
        let object = Arc::new(Mutex::new(Object::new()));

        let node = Layer::new(Arc::new(Mutex::new(Node {
            object,
            parent: None,
            children: vec![],
        })));
        self.layers.lock().insert(node.clone());

        node
    }
    pub fn remove_layer(&self, layer: &mut Layer) -> Result<(), NoObjectError> {
        let node: NObject;
        let mut layers = self.layers.lock();
        if layers.remove(layer) {
            node = layer.root.clone();
        } else {
            return Err(NoObjectError);
        }
        let mut objectguard = node.lock();

        //delete all the children of the layer too.
        objectguard.remove_children(&mut layer.objects_map.lock());
        //finish him!
        layers.remove(layer);

        Ok(())
    }

    pub fn get_layers(&self) -> IndexSet<Layer> {
        self.layers.lock().clone()
    }

    //Add support to serialize and deserialize scenes. load and undload.
    //Add those functions to game.
}
impl Default for Scene {
    fn default() -> Self {
        Self {
            layers: Arc::new(Mutex::new(indexset![])),
        }
    }
}

use core::f32::consts::FRAC_1_SQRT_2;
pub fn scale(mode: CameraScaling, dimensions: (f32, f32)) -> (f32, f32) {
    match mode {
        CameraScaling::Stretch => (1.0, 1.0),
        CameraScaling::Linear => (
            0.5 / (dimensions.1 / (dimensions.0 + dimensions.1)),
            0.5 / (dimensions.0 / (dimensions.0 + dimensions.1)),
        ),
        CameraScaling::Circle => (
            1.0 / (dimensions.1.atan2(dimensions.0).sin() / FRAC_1_SQRT_2),
            1.0 / (dimensions.1.atan2(dimensions.0).cos() / FRAC_1_SQRT_2),
        ),
        CameraScaling::Limited => (
            1.0 / (dimensions.1 / dimensions.0.clamp(0.0, dimensions.1)),
            1.0 / (dimensions.0 / dimensions.1.clamp(0.0, dimensions.0)),
        ),
        CameraScaling::Expand => (dimensions.0 * 0.001, dimensions.1 * 0.001),
    }
}
