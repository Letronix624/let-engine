//! Objects to be drawn to the screen.

#[cfg(feature = "client")]
pub mod appearance;
#[cfg(feature = "client")]
pub mod color;
#[cfg(feature = "labels")]
pub mod labels;

#[cfg(feature = "physics")]
pub mod physics;
pub mod scenes;

use crate::prelude::*;

use derive_builder::Builder;
use scenes::Layer;

use glam::f32::{vec2, Vec2};
use parking_lot::Mutex;

use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};
#[cfg(feature = "physics")]
type RigidBodyParent = Option<Option<Weak<Mutex<Node<Object>>>>>;
type ObjectsMap = HashMap<usize, NObject>;
pub(crate) type NObject = Arc<Mutex<Node<Object>>>;
type WeakObject = Weak<Mutex<Node<Object>>>;

/// Holds position size and rotation of an object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub position: Vec2,
    pub size: Vec2,
    pub rotation: f32,
}
impl Eq for Transform {}
impl Transform {
    /// Combines two Transforms with each other. It adds position, multiplies size and adds rotation.
    pub fn combine(self, rhs: Self) -> Self {
        Self {
            position: self.position + rhs.position.rotate(Vec2::from_angle(self.rotation)),
            size: self.size * rhs.size,
            rotation: self.rotation + rhs.rotation,
        }
    }

    /// Sets the position of this transform and returns itself.
    pub fn position(mut self, position: Vec2) -> Self {
        self.position = position;
        self
    }

    /// Sets the size of this transform and returns itself.
    pub fn size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    /// Sets the rotation of this transform and returns itself.
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

#[derive(Clone)]
#[cfg(feature = "client")]
pub(crate) struct VisualObject {
    pub transform: Transform,
    pub appearance: Appearance,
}
#[cfg(feature = "client")]
impl VisualObject {
    /// Combines the object position data.
    pub fn combined(object: &Object, other: &Object) -> Self {
        let transform = object.transform.combine(other.transform);
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
    pub parent: Option<Weak<Mutex<Node<T>>>>,
    #[cfg(feature = "physics")]
    pub rigid_body_parent: RigidBodyParent,
    pub children: Vec<Arc<Mutex<Node<T>>>>,
}
impl PartialEq for Node<Object> {
    fn eq(&self, other: &Self) -> bool {
        self.object == other.object
    }
}

impl Node<Object> {
    /// Takes a vector of every object transform and appearance and fills it with the right client order based on the root node inserted.
    #[cfg(feature = "client")]
    pub(crate) fn order_position(order: &mut Vec<VisualObject>, objects: &Self) {
        for child in objects.children.iter() {
            let child = child.lock();
            let object = VisualObject::combined(&objects.object, &child.object);
            order.push(object.clone());
            for child in child.children.iter() {
                let child = child.lock();
                order.push(VisualObject {
                    transform: object.transform.combine(child.object.transform),
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
    pub fn remove_children(
        &mut self,
        objects: &mut ObjectsMap,
        #[cfg(feature = "physics")] rigid_bodies: &mut ObjectsMap,
    ) {
        #[cfg(feature = "physics")]
        {
            let layer = self.object.layer().clone();
            self.object.physics.remove(layer.physics());
        }
        for child in self.children.iter() {
            child.clone().lock().remove_children(
                objects,
                #[cfg(feature = "physics")]
                rigid_bodies,
            );
        }
        objects.remove(&self.object.id());
        #[cfg(feature = "physics")]
        rigid_bodies.remove(&self.object.id());
        self.children = vec![];
    }

    /// Returns the public transform of this objects.
    pub fn end_transform(&self) -> Transform {
        if let Some(parent) = &self.parent {
            let parent = parent.upgrade().unwrap();
            let parent = parent.lock();
            parent.end_transform().combine(self.object.transform)
        } else {
            self.object.transform
        }
    }
}

/// Object to be initialized to the layer.
#[derive(Default, Clone, Builder, PartialEq, Debug)]
pub struct NewObject {
    #[builder(setter(into))]
    pub transform: Transform,
    #[builder(setter(into))]
    #[cfg(feature = "client")]
    pub appearance: Appearance,
    #[builder(setter(skip))]
    #[cfg(feature = "physics")]
    pub(crate) physics: ObjectPhysics,
}

/// An initialized object that gets rendered on the screen.
#[derive(Clone)]
pub struct Object {
    pub transform: Transform,
    parent_transform: Transform,
    #[cfg(feature = "client")]
    pub appearance: Appearance,
    id: usize,
    node: Option<WeakObject>,
    #[cfg(feature = "physics")]
    pub(crate) physics: ObjectPhysics,
    layer: Option<Arc<Layer>>,
}
impl std::fmt::Debug for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Object")
            .field("id", &self.id)
            .field("transform", &self.transform)
            .field("parent_transform", &self.parent_transform)
            .finish()
    }
}

impl Eq for NewObject {}
impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        #[cfg(not(feature = "client"))]
        {
            self.transform == other.transform
                && self.parent_transform == other.parent_transform
                && self.id == other.id
                && self.layer == other.layer
        }
        #[cfg(feature = "client")]
        {
            self.transform == other.transform
                && self.parent_transform == other.parent_transform
                && self.appearance == other.appearance
                && self.id == other.id
                && self.layer == other.layer
        }
    }
}
impl Eq for Object {}

/// New
impl NewObject {
    /// Returns a default object
    pub fn new() -> Self {
        Self::default()
    }

    /// Initializes the object into a layer.
    pub fn init(self, layer: &Arc<Layer>) -> Object {
        self.init_with_optional_parent(layer, None)
    }

    /// Initializes the object into a layer with a parent object.
    pub fn init_with_parent(self, layer: &Arc<Layer>, parent: &Object) -> Object {
        self.init_with_optional_parent(layer, Some(parent))
    }

    /// Initializes the object into a layer with an optional parent object.
    #[allow(unused_mut)]
    pub fn init_with_optional_parent(
        mut self,
        layer: &Arc<Layer>,
        parent: Option<&Object>,
    ) -> Object {
        // Init ID of this object.
        let id = layer.increment_id();

        #[cfg(feature = "physics")]
        let mut rigid_body_parent;
        let parent: NObject = if let Some(parent) = parent {
            let parent = parent.as_node();
            #[cfg(feature = "physics")]
            {
                rigid_body_parent = parent.lock().rigid_body_parent.clone();
            }
            parent.clone()
        } else {
            #[cfg(feature = "physics")]
            {
                rigid_body_parent = None;
            }
            layer.root.clone()
        };
        // Updates the physics side and returns the parent position.
        #[cfg(feature = "physics")]
        let parent_transform = self.physics.update(
            &self.transform,
            &parent,
            &mut rigid_body_parent,
            id as u128,
            layer.physics(),
        );
        #[cfg(not(feature = "physics"))]
        let parent_transform = parent.lock().object.public_transform();

        let mut initialized = Object {
            transform: self.transform,
            parent_transform,
            #[cfg(feature = "client")]
            appearance: self.appearance,
            id,
            node: None,
            #[cfg(feature = "physics")]
            physics: self.physics,
            layer: Some(layer.clone()),
        };

        // Make yourself to a node.
        let node: NObject = std::sync::Arc::new(Mutex::new(Node {
            object: initialized.clone(),
            parent: Some(std::sync::Arc::downgrade(&parent)),
            #[cfg(feature = "physics")]
            rigid_body_parent: rigid_body_parent.clone(),
            children: vec![],
        }));

        // set reference to own node to manipulate.
        let reference = Some(std::sync::Arc::downgrade(&node));
        node.lock().object.node = reference.clone();
        initialized.node = reference;

        // In case there is no rigid body roots make yourself one.
        #[cfg(feature = "physics")]
        if let Some(value) = &rigid_body_parent {
            if value.is_none() && initialized.physics.rigid_body.is_some() {
                layer.rigid_body_roots().lock().insert(id, node.clone());
            }
        }

        // Add yourself to the objects map.
        layer.add_object(id, &node);

        // Add yourself to the list of children of the parent.
        parent.lock().children.push(node.clone());
        initialized
    }
}

/// Setters
impl NewObject {
    /// Sets the position and rotation of an object.
    pub fn set_isometry(&mut self, position: Vec2, rotation: f32) {
        self.transform.position = position;
        self.transform.rotation = rotation;
    }
    /// Returns a reference to the appearance of the object.
    #[cfg(feature = "client")]
    pub fn appearance(&self) -> &Appearance {
        &self.appearance
    }
}

/// Physics
#[cfg(feature = "physics")]
impl NewObject {
    /// Returns the collider of the object in case it has one.
    #[cfg(feature = "physics")]
    pub fn collider(&self) -> Option<&Collider> {
        self.physics.collider.as_ref()
    }
    /// Sets the collider of the object.
    #[cfg(feature = "physics")]
    pub fn set_collider(&mut self, collider: Option<Collider>) {
        self.physics.collider = collider;
    }
    /// Returns a mutable reference to the collider.
    #[cfg(feature = "physics")]
    pub fn collider_mut(&mut self) -> Option<&mut Collider> {
        self.physics.collider.as_mut()
    }
    /// Returns the rigid bodyh of the object in case it has one.
    #[cfg(feature = "physics")]
    pub fn rigid_body(&self) -> Option<&RigidBody> {
        self.physics.rigid_body.as_ref()
    }
    /// Sets the rigid body of the object.
    #[cfg(feature = "physics")]
    pub fn set_rigid_body(&mut self, rigid_body: Option<RigidBody>) {
        self.physics.rigid_body = rigid_body;
    }
    /// Returns a mutable reference to the rigid body.
    #[cfg(feature = "physics")]
    pub fn rigid_body_mut(&mut self) -> Option<&mut RigidBody> {
        self.physics.rigid_body.as_mut()
    }
    /// Returns the local position of the collider.
    #[cfg(feature = "physics")]
    pub fn local_collider_position(&self) -> Vec2 {
        self.physics.local_collider_position
    }
    /// Sets the local position of the collider of this object in case it has one.
    #[cfg(feature = "physics")]
    pub fn set_local_collider_position(&mut self, pos: Vec2) {
        self.physics.local_collider_position = pos;
    }
}

impl Object {
    pub(crate) fn root() -> Self {
        Self {
            transform: Transform::default(),
            parent_transform: Transform::default(),
            #[cfg(feature = "client")]
            appearance: Appearance::default(),
            id: 0,
            node: None,
            #[cfg(feature = "physics")]
            physics: ObjectPhysics::default(),
            layer: None,
        }
    }

    pub fn layer(&self) -> &Arc<Layer> {
        self.layer.as_ref().unwrap()
    }

    /// Removes the object from it's layer.
    #[allow(unused_mut)]
    pub fn remove(mut self) -> NewObject {
        let layer = self.layer.unwrap();
        let mut map = layer.objects_map.lock();
        #[cfg(feature = "physics")]
        let mut rigid_bodies = layer.rigid_body_roots().lock();
        let node = map.remove(&self.id).unwrap();

        #[cfg(feature = "physics")]
        {
            rigid_bodies.remove(&self.id);
            // Remove self from the physics side.
            self.physics.remove(layer.physics());
        }

        let mut object = node.lock();
        object.remove_children(
            &mut map,
            #[cfg(feature = "physics")]
            &mut rigid_bodies,
        );

        let parent = object.parent.clone().unwrap().upgrade().unwrap();
        let mut parent = parent.lock();
        parent.remove_child(&node);

        NewObject {
            transform: self.transform,
            #[cfg(feature = "client")]
            appearance: self.appearance,
            #[cfg(feature = "physics")]
            physics: self.physics,
        }
    }

    /// Makes a new object from this object.
    pub fn to_new(&self) -> NewObject {
        NewObject {
            transform: self.transform,
            #[cfg(feature = "client")]
            appearance: self.appearance.clone(),
            #[cfg(feature = "physics")]
            physics: self.physics.clone(),
        }
    }

    /// Copies the data from a `NewObject` into itself.
    pub fn copy_new(&mut self, object: NewObject) {
        self.transform = object.transform;
        #[cfg(feature = "physics")]
        {
            self.physics = object.physics;
        }
        #[cfg(feature = "client")]
        {
            self.appearance = object.appearance;
        }
    }

    /// Sets the position and rotation of an object.
    pub fn set_isometry(&mut self, position: Vec2, rotation: f32) {
        self.transform.position = position;
        self.transform.rotation = rotation;
    }

    /// Returns the public position where the object is going to be rendered.
    pub fn public_transform(&self) -> Transform {
        self.transform.combine(self.parent_transform)
    }

    pub(crate) fn set_parent_transform(&mut self, transform: Transform) {
        self.parent_transform = transform;
    }

    /// Returns a reference to the appearance of the object.
    #[cfg(feature = "client")]
    pub fn appearance(&self) -> &Appearance {
        &self.appearance
    }

    /// Returns the identification number of the object specific the layer it is inside right now.
    ///
    /// Returns 0 in case it is not initialized to a layer yet.
    pub fn id(&self) -> usize {
        self.id
    }

    pub(crate) fn as_node(&self) -> NObject {
        self.node.as_ref().unwrap().upgrade().unwrap()
    }

    /// Updates the object to match the object information located inside the system of the layer. Useful when having physics.
    pub fn update(&mut self) {
        // receive
        let node = self.as_node();
        let object = &node.lock().object;
        self.transform = object.transform;
        #[cfg(feature = "client")]
        {
            self.appearance = object.appearance().clone();
        }
    }

    /// Updates the object inside the layer system to match with this one. Useful when doing anything to the object and submitting it with this function.
    pub fn sync(&mut self) {
        // send
        // update public position of all children recursively
        let node = self.as_node().clone();
        #[cfg(feature = "physics")]
        {
            let mut node = node.lock();
            let layer = self.layer().clone();
            self.parent_transform = self.physics.update(
                &self.transform,
                &node.parent.clone().unwrap().upgrade().unwrap(),
                &mut node.rigid_body_parent,
                self.id as u128,
                layer.physics(),
            );
        }
        node.lock()
            .update_children_position(self.public_transform());
        let arc = self.as_node();
        let mut object = arc.lock();
        object.object = self.clone();
    }
}

/// Physics
#[cfg(feature = "physics")]
impl Object {
    pub(crate) fn rigidbody_handle(&self) -> Option<rapier2d::dynamics::RigidBodyHandle> {
        self.physics.rigid_body_handle
    }

    /// Returns the collider of the object in case it has one.
    pub fn collider(&self) -> Option<&Collider> {
        self.physics.collider.as_ref()
    }

    /// Sets the collider of the object.
    pub fn set_collider(&mut self, collider: Option<Collider>) {
        self.physics.collider = collider;
    }

    /// Returns a mutable reference to the collider.
    pub fn collider_mut(&mut self) -> Option<&mut Collider> {
        self.physics.collider.as_mut()
    }

    /// Returns the rigid bodyh of the object in case it has one.
    pub fn rigid_body(&self) -> Option<&RigidBody> {
        self.physics.rigid_body.as_ref()
    }

    /// Sets the rigid body of the object.
    pub fn set_rigid_body(&mut self, rigid_body: Option<RigidBody>) {
        self.physics.rigid_body = rigid_body;
    }

    /// Returns a mutable reference to the rigid body.
    pub fn rigid_body_mut(&mut self) -> Option<&mut RigidBody> {
        self.physics.rigid_body.as_mut()
    }

    /// Returns the local position of the collider.
    pub fn local_collider_position(&self) -> Vec2 {
        self.physics.local_collider_position
    }

    /// Sets the local position of the collider of this object in case it has one.
    pub fn set_local_collider_position(&mut self, pos: Vec2) {
        self.physics.local_collider_position = pos;
    }
}
