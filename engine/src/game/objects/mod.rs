pub mod data;
pub mod physics;
use super::{materials, AObject, NObject, WeakObject};
use crate::error::objects::*;
use crate::error::textures::*;
pub use data::*;
use physics::Physics;

use anyhow::Result;
use glam::f32::{vec2, Vec2};
use hashbrown::HashMap;
use indexmap::{indexset, IndexSet};
use parking_lot::Mutex;
use rapier2d::prelude::*;

use std::{
    default,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Weak,
    },
};

type ObjectsMap = HashMap<usize, NObject>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub position: Vec2,
    pub size: Vec2,
    pub rotation: f32,
}
impl Transform {
    pub fn combine(self, rhs: Self) -> Self {
        Self {
            position: self.position + rhs.position,
            size: self.size * rhs.size,
            rotation: self.rotation + rhs.rotation,
        }
    }
    pub fn position(mut self, position: Vec2) -> Self {
        self.position = position;
        self
    }
    pub fn size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }
    pub fn rotation(mut self, rotation: f32) -> Self {
        self.rotation = rotation;
        self
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: vec2(0.0, 0.0),
            size: vec2(1.0, 1.0),
            rotation: 0.0,
        }
    }
}

pub trait GameObject: Send {
    fn transform(&self) -> Transform;
    fn public_transform(&self) -> Transform;
    fn set_parent_transform(&mut self, transform: Transform);
    fn appearance(&self) -> &Appearance;
    fn id(&self) -> usize;
    fn init_to_layer(&mut self, id: usize, weak: WeakObject, layer: &Layer);
    fn remove_event(&mut self);
}

pub trait Camera: GameObject {
    fn settings(&self) -> CameraSettings;
}

pub trait Collider: GameObject {
    fn collider_handle(&self) -> Option<rapier2d::geometry::ColliderHandle>;
}

pub trait Rigidbody: Collider + GameObject {
    fn rigidbody(&self) -> rapier2d::dynamics::RigidBodyHandle;
}

#[derive(Clone, Copy)]
pub struct CameraSettings {
    pub zoom: f32,
    pub mode: CameraScaling,
}
impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            mode: CameraScaling::Stretch,
        }
    }
}
impl CameraSettings {
    pub fn zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom;
        self
    }
    pub fn mode(mut self, mode: CameraScaling) -> Self {
        self.mode = mode;
        self
    }
}

#[derive(Clone)]
pub struct Object {
    pub transform: Transform,
    pub appearance: Appearance,
}
impl Object {
    pub fn combined(object: &AObject, other: &AObject) -> Self {
        let transform = object.transform().combine(other.transform());
        let appearance = other.appearance().clone();
        Self {
            transform,
            appearance,
        }
    }
}

pub struct Node<T> {
    pub object: T,
    pub parent: Option<Weak<Mutex<Node<T>>>>,
    pub children: Vec<Arc<Mutex<Node<T>>>>,
}

impl Node<AObject> {
    pub fn order_position(order: &mut Vec<Object>, objects: &Self) {
        for child in objects.children.iter() {
            let child = child.lock();
            let object = Object::combined(&objects.object, &child.object);
            order.push(object.clone());
            for child in child.children.iter() {
                let child = child.lock();
                order.push(Object {
                    transform: object.transform.combine(child.object.transform()),
                    appearance: child.object.appearance().clone(),
                });
                Self::order_position(order, &child);
            }
        }
    }
    pub fn update_children_position(&mut self, parent_pos: Transform) {
        self.object.set_parent_transform(parent_pos);
        for child in self.children.iter() {
            child
                .lock()
                .update_children_position(self.object.public_transform());
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
        self.object.remove_event();
        for child in self.children.iter() {
            child.clone().lock().remove_children(objects);
        }
        objects.remove(&self.object.id());
        self.children = vec![];
    }
    pub fn end_transform(&self) -> Transform {
        if let Some(parent) = &self.parent {
            let parent = parent.upgrade().unwrap();
            let parent = parent.lock();
            parent.end_transform().combine(self.object.transform())
        } else {
            self.object.transform()
        }
    }
}

/// Holds everything about the appearance of objects like
/// textures, vetex/index data, color and material.
#[derive(Debug, Clone, PartialEq)]
pub struct Appearance {
    pub visible: bool,
    pub material: Option<materials::Material>,
    pub data: Data,
    pub transform: Transform,
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

        self.transform.size = vec2(dimensions.0 as f32 / 1000.0, dimensions.1 as f32 / 1000.0);

        Ok(())
    }
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
    pub fn data(mut self, data: Data) -> Self {
        self.data = data;
        self
    }
    pub fn transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
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
            visible: true,
            material: None,
            data: Data::empty(),
            transform: Transform::default(),
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}
#[derive(Clone)]
pub struct Layer {
    pub root: NObject,
    pub camera: Arc<Mutex<Option<Box<dyn Camera>>>>,
    objects_map: Arc<Mutex<ObjectsMap>>,
    latest_object: Arc<AtomicU64>,
    pub(crate) physics: Arc<Mutex<Physics>>,
    pub physics_enabled: bool,
    query_pipeline: Arc<Mutex<QueryPipeline>>,
}

impl Layer {
    pub fn new(root: NObject) -> Self {
        let mut objects_map = HashMap::new();
        objects_map.insert(0, root.clone());
        Self {
            root,
            camera: Arc::new(Mutex::new(None)),
            objects_map: Arc::new(Mutex::new(objects_map)),
            latest_object: Arc::new(AtomicU64::new(1)),
            physics: Arc::new(Mutex::new(Physics::new())),
            physics_enabled: false,
            query_pipeline: Arc::new(Mutex::new(QueryPipeline::new())),
        }
    }
    pub fn set_camera<T: Camera + 'static>(&self, camera: T) {
        *self.camera.lock() = Some(Box::new(camera));
    }
    pub(crate) fn camera_position(&self) -> Vec2 {
        if let Some(camera) = self.camera.lock().as_ref() {
            camera.transform().position
        } else {
            vec2(0.0, 0.0)
        }
    }
    pub(crate) fn camera_scaling(&self) -> CameraScaling {
        if let Some(camera) = self.camera.lock().as_ref() {
            camera.settings().mode
        } else {
            CameraScaling::Stretch
        }
    }
    pub(crate) fn zoom(&self) -> f32 {
        if let Some(camera) = self.camera.lock().as_ref() {
            camera.settings().zoom
        } else {
            1.0
        }
    }

    /// Be careful! Don't use this when the camera is locked.
    pub fn side_to_world(&self, direction: [f32; 2], dimensions: (f32, f32)) -> Vec2 {
        let camera = Self::camera_position(self);
        let direction = [direction[0] * 2.0 - 1.0, direction[1] * 2.0 - 1.0];
        let (width, height) = scale(Self::camera_scaling(self), dimensions);
        let zoom = 1.0 / Self::zoom(self);
        vec2(
            direction[0] * (width * zoom) + camera.x * 2.0,
            direction[1] * (height * zoom) + camera.y * 2.0,
        )
    }

    pub fn contains_object(&self, object_id: &usize) -> bool {
        self.objects_map.lock().contains_key(object_id)
    }

    pub fn step_physics(&self, physics: &mut PhysicsPipeline) {
        self.physics.lock().step(physics);
    }

    pub fn gravity(&self) -> Vec2 {
        self.physics.lock().gravity.into()
    }
    pub fn set_gravity(&self, gravity: Vec2) {
        self.physics.lock().gravity = gravity.into();
    }

    pub fn add_object<T: GameObject + Clone + 'static>(
        &self,
        parent: Option<&AObject>,
        object: &mut T,
    ) -> Result<(), NoParentError> {
        let mut map = self.objects_map.lock();
        let parent: NObject = if let Some(parent) = parent {
            if let Some(parent) = map.get(&parent.id()) {
                parent.clone()
            } else {
                return Err(NoParentError);
            }
        } else {
            self.root.clone()
        };

        let id = self.latest_object.fetch_add(1, Ordering::AcqRel) as usize;

        let placeholder = crate::prelude::Object::default();

        let node: NObject = Arc::new(Mutex::new(Node {
            object: Box::new(placeholder),
            parent: Some(Arc::downgrade(&parent)),
            children: vec![],
        }));

        object.init_to_layer(id, Arc::downgrade(&node), self);

        let boxed_object: Box<dyn GameObject> = Box::new(object.clone());

        node.lock().object = boxed_object;

        parent.lock().children.push(node.clone());

        map.insert(id, node);

        Ok(())
    }

    /// Checks if a collider is at a specific location and returns the ID of that collider.
    pub fn query_collider_at(&self, position: Vec2) -> Option<usize> {
        let physics = self.physics.lock();
        let mut pipeline = self.query_pipeline.lock();

        pipeline.update(&physics.rigid_body_set, &physics.collider_set);

        let result = pipeline.cast_ray(
            &physics.rigid_body_set,
            &physics.collider_set,
            &Ray::new(position.into(), vector![0.0, 0.0]),
            0.0,
            true,
            QueryFilter::default(),
        );

        if let Some((handle, _)) = result {
            Some(physics.collider_set.get(handle).unwrap().user_data as usize)
        } else {
            None
        }
    }

    pub fn remove_object(&self, object_id: usize) -> Result<(), NoObjectError> {
        let mut map = self.objects_map.lock();
        let node = if let Some(object) = map.remove(&object_id) {
            object
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
        if let Some(object) = map.get(&object.id()) {
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
    physics_pipeline: Arc<Mutex<PhysicsPipeline>>,
}

struct Root {
    pub transform: Transform,
    pub appearance: Appearance,
    id: usize,
}
impl GameObject for Root {
    fn transform(&self) -> Transform {
        self.transform
    }
    fn public_transform(&self) -> Transform {
        self.transform
    }
    fn set_parent_transform(&mut self, _transform: Transform) {}
    fn appearance(&self) -> &Appearance {
        &self.appearance
    }
    fn id(&self) -> usize {
        self.id
    }
    fn init_to_layer(&mut self, _id: usize, _weak: WeakObject, _layer: &Layer) {}
    fn remove_event(&mut self) {}
}
impl Default for Root {
    fn default() -> Self {
        Self {
            transform: Transform::default(),
            appearance: Appearance::default().visible(false),
            id: 0,
        }
    }
}

impl Scene {
    pub fn new_layer(&self) -> Layer {
        let object = Box::<Root>::default();

        let node = Layer::new(Arc::new(Mutex::new(Node {
            object,
            parent: None,
            children: vec![],
        })));
        self.layers.lock().insert(node.clone());

        node
    }

    pub fn iterate_all_physics(&self) {
        let mut physics = self.physics_pipeline.lock();
        let layers = self.layers.lock();

        for layer in layers.iter() {
            layer.step_physics(&mut physics);
        }
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
            physics_pipeline: Arc::new(Mutex::new(PhysicsPipeline::new())),
        }
    }
}

use core::f32::consts::FRAC_1_SQRT_2; // Update. move to crate/utils.rs
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
