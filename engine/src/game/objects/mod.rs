//! Objects to be drawn to the screen.

pub mod data;
pub mod labels;
pub mod physics;
use self::data::Data;

use super::camera::{Camera, CameraScaling};
use super::{materials, AObject, NObject};
use crate::error::objects::*;
use crate::error::textures::*;
use crate::utils::scale;
use physics::Physics;

use anyhow::Result;
use glam::f32::{vec2, Vec2};
use hashbrown::HashMap;
use indexmap::{indexset, IndexSet};
use parking_lot::Mutex;
use rapier2d::prelude::*;

use std::{
    any::Any,
    default,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Weak,
    },
};
type ObjectsMap = HashMap<usize, NObject>;

/// Holds position size and rotation of an object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub position: Vec2,
    pub size: Vec2,
    pub rotation: f32,
}
impl Transform {
    pub fn combine(self, rhs: Self) -> Self {
        Self {
            position: self.position + rhs.position.rotate(Vec2::from_angle(self.rotation)),
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

/// The trait that automatically gets implemented to each object you give the object attribute.
pub trait GameObject: Send + Any {
    fn transform(&self) -> Transform;
    fn set_isometry(&mut self, position: Vec2, rotation: f32);
    fn public_transform(&self) -> Transform;
    fn set_parent_transform(&mut self, transform: Transform);
    fn appearance(&self) -> &Appearance;
    fn id(&self) -> usize;
    fn init_to_layer(
        &mut self,
        id: usize,
        parent: &NObject,
        layer: &Layer,
    ) -> NObject;
    fn remove_event(&mut self);
    fn as_any(&self) -> &dyn Any;
    fn collider_handle(&self) -> Option<rapier2d::geometry::ColliderHandle>;
    fn rigidbody_handle(&self) -> Option<rapier2d::dynamics::RigidBodyHandle>;
}

#[derive(Clone)]
pub(crate) struct Object {
    pub transform: Transform,
    pub appearance: Appearance,
}
impl Object {
    /// Combines the object position data.
    pub fn combined(object: &AObject, other: &AObject) -> Self {
        let transform = object.transform().combine(other.transform());
        let appearance = other.appearance().clone();
        Self {
            transform,
            appearance,
        }
    }
}

/// Node structure for the layer.
pub struct Node<T> {
    pub object: T,
    pub parent: Option<Weak<Mutex<Node<AObject>>>>,
    pub children: Vec<Arc<Mutex<Node<T>>>>,
}

impl Node<AObject> {
    /// Takes a vector of every object transform and appearance and fills it with the right drawing order based on the root node inserted.
    pub(crate) fn order_position(order: &mut Vec<Object>, objects: &Self) {
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
    /// Iterates to the last child to update all public position held by the Node.
    pub fn update_children_position(&mut self, parent_pos: Transform) {
        self.object.set_parent_transform(parent_pos);
        for child in self.children.iter() {
            child
                .lock()
                .update_children_position(self.object.public_transform());
        }
    }
    /// Searches for the given object to be removed from the list of children.
    pub fn remove_child(&mut self, object: &NObject) {
        let index = self
            .children
            .clone()
            .into_iter()
            .position(|x| Arc::ptr_eq(&x, object))
            .unwrap();
        self.children.remove(index);
    }
    /// Removes all children and their children from the layer.
    pub fn remove_children(&mut self, objects: &mut ObjectsMap, rigid_bodies: &mut ObjectsMap) {
        self.object.remove_event();
        for child in self.children.iter() {
            child.clone().lock().remove_children(objects, rigid_bodies);
        }
        objects.remove(&self.object.id());
        rigid_bodies.remove(&self.object.id());
        self.children = vec![];
    }
    /// Returns the public transform of this objects.
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
    /// Makes a default appearance.
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
    /// Makes a while 1x1 square.
    pub fn new_square() -> Self {
        Self {
            data: Data::square(),
            ..Default::default()
        }
    }
    /// Makes a new appearance with given color.
    pub fn new_color(color: [f32; 4]) -> Self {
        Self {
            color,
            ..Default::default()
        }
    }
    /// Scales the object appearance according to the texture applied. Works best in Expand camera mode for best quality.
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
    /// Sets visibility of this appearance.
    #[inline]
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
    /// Sets the model data of this appearance.
    #[inline]
    pub fn data(mut self, data: Data) -> Self {
        self.data = data;
        self
    }
    /// Sets the transform of this appearance.
    #[inline]
    pub fn transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }
    /// Sets the color of this appearance.
    pub fn color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }
    /// Sets the material of this appearance.
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
/// A layer struct holding it's own object hierarchy, camera and physics iteration.
#[derive(Clone)]
pub struct Layer {
    pub root: NObject,
    pub camera: Arc<Mutex<Option<Box<dyn Camera>>>>,
    objects_map: Arc<Mutex<ObjectsMap>>,
    pub(crate) rigid_body_roots: Arc<Mutex<ObjectsMap>>,
    latest_object: Arc<AtomicU64>,
    pub(crate) physics: Arc<Mutex<Physics>>,
    physics_enabled: Arc<AtomicBool>,
}

impl Layer {
    /// Creates a new layer with the given root.
    pub fn new(root: NObject) -> Self {
        let mut objects_map = HashMap::new();
        objects_map.insert(0, root.clone());
        Self {
            root,
            camera: Arc::new(Mutex::new(None)),
            objects_map: Arc::new(Mutex::new(objects_map)),
            rigid_body_roots: Arc::new(Mutex::new(HashMap::new())),
            latest_object: Arc::new(AtomicU64::new(1)),
            physics: Arc::new(Mutex::new(Physics::new())),
            physics_enabled: Arc::new(AtomicBool::new(true)),
        }
    }
    /// Sets the camera of this layer.
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

    /// Returns the position of a given side with given window dimensions to world space.
    ///
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

    /// Checks if the layer contains this object.
    pub fn contains_object(&self, object_id: &usize) -> bool {
        self.objects_map.lock().contains_key(object_id)
    }

    pub(crate) fn step_physics(&self, physics_pipeline: &mut PhysicsPipeline) {
        if self.physics_enabled.load(Ordering::Acquire) {
            let mut map = self.rigid_body_roots.lock();

            let mut physics = self.physics.lock();
            physics.step(physics_pipeline); // Rapier-side physics iteration run.
            for (_, object) in map.iter_mut() {
                let mut node = object.lock();
                let rigid_body = physics
                    .rigid_body_set
                    .get(node.object.rigidbody_handle().unwrap())
                    .unwrap();
                node.object.set_isometry(
                    (*rigid_body.translation()).into(),
                    rigid_body.rotation().angle(),
                );
            }
        }
    }

    /// Gets the gravity parameter.
    pub fn gravity(&self) -> Vec2 {
        self.physics.lock().gravity.into()
    }
    /// Sets the gravity parameter.
    pub fn set_gravity(&self, gravity: Vec2) {
        self.physics.lock().gravity = gravity.into();
    }
    /// Returns if physics is enabled.
    pub fn physics_enabled(&self) -> bool {
        self.physics_enabled.load(Ordering::Acquire)
    }
    /// Set physics to be enabled or disabled.
    pub fn set_physics_enabled(&self, enabled: bool) {
        self.physics_enabled.store(enabled, Ordering::Release)
    }
    /// Takes the physics simulation parameters.
    pub fn physics_parameters(&self) -> IntegrationParameters {
        self.physics.lock().integration_parameters
    }
    /// Sets the physics simulation parameters.
    pub fn set_physics_parameters(&self, parameters: IntegrationParameters) {
        self.physics.lock().integration_parameters = parameters;
    }
    /// Adds a joint between object 1 and 2.
    pub fn add_joint(
        &self,
        object1: &impl GameObject,
        object2: &impl GameObject,
        data: impl Into<physics::joints::GenericJoint>,
        wake_up: bool,
    ) -> Result<ImpulseJointHandle, NoRigidBodyError> {
        if let (Some(handle1), Some(handle2)) =
            (object1.rigidbody_handle(), object2.rigidbody_handle())
        {
            Ok(self.physics.lock().impulse_joint_set.insert(
                handle1,
                handle2,
                data.into().data,
                wake_up,
            ))
        } else {
            Err(NoRigidBodyError)
        }
    }
    /// Returns if the joint exists.
    pub fn get_joint(&self, handle: ImpulseJointHandle) -> Option<physics::joints::GenericJoint> {
        self.physics
            .lock()
            .impulse_joint_set
            .get(handle)
            .map(|joint| physics::joints::GenericJoint { data: joint.data })
    }
    /// Updates a joint.
    pub fn set_joint(
        &self,
        data: impl Into<physics::joints::GenericJoint>,
        handle: ImpulseJointHandle,
    ) -> Result<(), NoJointError> {
        if let Some(joint) = self.physics.lock().impulse_joint_set.get_mut(handle) {
            joint.data = data.into().data;
            Ok(())
        } else {
            Err(NoJointError)
        }
    }
    /// Removes a joint.
    pub fn remove_joint(&self, handle: ImpulseJointHandle, wake_up: bool) {
        self.physics
            .lock()
            .impulse_joint_set
            .remove(handle, wake_up);
    }

    /// Adds object with an optional parent.
    pub fn add_object_with_optional_parent<T: GameObject + Clone + 'static>(
        &self,
        parent: Option<&T>,
        object: &mut T,
    ) -> Result<(), NoParentError> {
        // Create ID for object
        let id = self.latest_object.fetch_add(1, Ordering::AcqRel) as usize;

        let parent: NObject = if let Some(parent) = parent {
            let map = self.objects_map.lock();
            if let Some(parent) = map.get(&parent.id()) {
                parent.clone()
            } else {
                return Err(NoParentError);
            }
        } else {
            self.root.clone()
        };

        let node = object.init_to_layer(id, &parent, self);

        let boxed_object: Box<dyn GameObject> = Box::new(object.clone());

        node.lock().object = boxed_object;

        parent.lock().children.push(node.clone());

        self.objects_map.lock().insert(id, node);

        Ok(())
    }

    /// Just adds an object without parent.
    pub fn add_object<T: GameObject + Clone + 'static>(&self, object: &mut T) {
        Self::add_object_with_optional_parent(self, None, object).unwrap();
    }
    /// Adds an object with given parent.
    pub fn add_object_with_parent<T: GameObject + Clone + 'static>(
        &self,
        parent: &T,
        object: &mut T,
    ) -> Result<(), NoParentError> {
        Self::add_object_with_optional_parent(self, Some(parent), object)
    }

    /// Removes an object using it's ID.
    pub fn remove_object(&self, object: &mut impl GameObject) -> Result<(), NoObjectError> {
        let mut map = self.objects_map.lock();
        let mut rigid_bodies = self.rigid_body_roots.lock();
        let node = if let Some(object) = map.remove(&object.id()) {
            object
        } else {
            return Err(NoObjectError);
        };
        rigid_bodies.remove(&object.id());
        object.remove_event();

        let mut object = node.lock();
        object.remove_children(&mut map, &mut rigid_bodies);

        let parent = object.parent.clone().unwrap().upgrade().unwrap();
        let mut parent = parent.lock();
        parent.remove_child(&node);

        Ok(())
    }

    /// Returns the nearest collider id from a specific location.
    pub fn query_nearest_collider_at(&self, position: Vec2) -> Option<usize> {
        let mut physics = self.physics.lock();
        physics.update_query_pipeline();

        let result = physics.query_pipeline.project_point(
            &physics.rigid_body_set,
            &physics.collider_set,
            &position.into(),
            true,
            QueryFilter::default(),
        );

        if let Some((handle, _)) = result {
            Some(physics.collider_set.get(handle).unwrap().user_data as usize)
        } else {
            None
        }
    }

    /// Returns id of the first collider intersecting with given ray.
    pub fn cast_ray(
        &self,
        position: Vec2,
        direction: Vec2,
        time_of_impact: Real,
        solid: bool,
    ) -> Option<usize> {
        let mut physics = self.physics.lock();
        physics.update_query_pipeline();

        let result = physics.query_pipeline.cast_ray(
            &physics.rigid_body_set,
            &physics.collider_set,
            &Ray::new(position.into(), direction.into()),
            time_of_impact,
            solid,
            QueryFilter::default(),
        );

        if let Some((handle, _)) = result {
            Some(physics.collider_set.get(handle).unwrap().user_data as usize)
        } else {
            None
        }
    }

    /// Cast a shape and return the first collider intersecting with it.
    pub fn intersection_with_shape(
        &self,
        shape: physics::Shape,
        position: (Vec2, f32),
    ) -> Option<usize> {
        let mut physics = self.physics.lock();
        physics.update_query_pipeline();

        let result = physics.query_pipeline.intersection_with_shape(
            &physics.rigid_body_set,
            &physics.collider_set,
            &position.into(),
            shape.0.as_ref(),
            QueryFilter::default(),
        );
        result.map(|handle| physics.collider_set.get(handle).unwrap().user_data as usize)
    }

    /// Moves an object on the given index in it's parents children order.
    pub fn move_to(
        &self,
        object: &impl GameObject,
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

    /// Moves an object one down in it's parents children order.
    pub fn move_down(&self, object: &impl GameObject) -> Result<(), Box<dyn std::error::Error>> {
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

    /// Moves an object one up in it's parents children order.
    pub fn move_up(&self, object: &impl GameObject) -> Result<(), Box<dyn std::error::Error>> {
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

    /// Moves an object all the way to the top of it's parents children list.
    pub fn move_to_top(&self, object: &impl GameObject) -> Result<(), NoObjectError> {
        let node = Self::to_node(self, object)?;
        Self::move_object_to(node, 0);
        Ok(())
    }

    /// Moves an object all the way to the bottom of it's parents children list.
    pub fn move_to_bottom(&self, object: &impl GameObject) -> Result<(), NoObjectError> {
        let node = Self::to_node(self, object)?;
        let count = Self::count_children(&node) - 1;
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

    fn to_node(&self, object: &impl GameObject) -> Result<NObject, NoObjectError> {
        let map = self.objects_map.lock();
        if let Some(object) = map.get(&object.id()) {
            Ok(object.clone())
        } else {
            Err(NoObjectError)
        }
    }

    /// Moves an object on the given index in it's parents children order.
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

    pub fn children_count(&self, parent: &impl GameObject) -> Result<usize, NoObjectError> {
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

/// The whole scene seen with all it's layers.
#[derive(Clone)]
pub struct Scene {
    layers: Arc<Mutex<IndexSet<Layer>>>,
    physics_pipeline: Arc<Mutex<PhysicsPipeline>>,
}

/// Default layer root object.
struct Root {
    pub transform: Transform,
    pub appearance: Appearance,
    id: usize,
}
impl GameObject for Root {
    fn transform(&self) -> Transform {
        self.transform
    }
    fn set_isometry(&mut self, _position: Vec2, _rotation: f32) {}
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
    fn init_to_layer(
        &mut self,
        _id: usize,
        _parent: &NObject,
        _layer: &Layer,
    ) -> NObject {
        todo!()
    }
    fn remove_event(&mut self) {}
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn collider_handle(&self) -> Option<ColliderHandle> {
        None
    }
    fn rigidbody_handle(&self) -> Option<RigidBodyHandle> {
        None
    }
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
    /// Iterates through all physics.
    pub fn iterate_all_physics(&self) {
        let mut pipeline = self.physics_pipeline.lock();
        let layers = self.layers.lock();

        for layer in layers.iter() {
            layer.step_physics(&mut pipeline);
        }
    }

    /// Initializes a new layer into the scene.
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

    /// Removes a layer from the scene.
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
        objectguard.remove_children(
            &mut layer.objects_map.lock(),
            &mut layer.rigid_body_roots.lock(),
        );
        //finish him!
        layers.remove(layer);

        Ok(())
    }

    /// Returns an IndexSet of all layers.
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
