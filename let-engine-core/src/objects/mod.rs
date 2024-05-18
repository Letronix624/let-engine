//! Objects to be drawn to the screen.

#[cfg(feature = "client")]
mod appearance;
#[cfg(feature = "client")]
mod color;
#[cfg(feature = "client")]
pub use appearance::*;
#[cfg(feature = "client")]
pub use color::Color;

#[cfg(feature = "physics")]
pub mod physics;
#[cfg(feature = "physics")]
use physics::*;

pub mod scenes;
use scenes::Layer;

use anyhow::{anyhow, Error, Result};

use derive_builder::Builder;
use parking_lot::Mutex;

use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};

use glam::{vec2, Vec2};

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

impl From<(Vec2, f32)> for Transform {
    fn from(value: (Vec2, f32)) -> Self {
        Self {
            position: value.0,
            rotation: value.1,
            ..Default::default()
        }
    }
}

impl From<Transform> for (Vec2, f32) {
    fn from(value: Transform) -> Self {
        (value.position, value.rotation)
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
pub(crate) struct Node<T> {
    pub object: T,
    // pub parent: Option<Weak<Mutex<Node<T>>>>,
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
            if !child.object.appearance.get_visible() {
                continue;
            }
            let object = VisualObject::combined(&objects.object, &child.object);
            order.push(object.clone());
            for child in child.children.iter() {
                let child = child.lock();
                if !child.object.appearance.get_visible() {
                    continue;
                }
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
    ///
    /// In case there is no child it will return an error.
    pub fn remove_child(&mut self, object: &NObject) -> Result<()> {
        let index = self
            .children
            .clone()
            .into_iter()
            .position(|x| Arc::ptr_eq(&x, object))
            .ok_or(Error::msg("No child found"))?;
        self.children.remove(index);
        Ok(())
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
        objects.remove(self.object.id());
        #[cfg(feature = "physics")]
        rigid_bodies.remove(self.object.id());
        self.children = vec![];
    }
}

/// Object to be initialized to the layer.
#[derive(Default, Clone, Builder, PartialEq, Debug)]
pub struct NewObject {
    #[builder(setter(into), default)]
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
    node: WeakObject,
    parent_node: Option<WeakObject>,
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
    pub fn init(self, layer: &Arc<Layer>) -> Result<Object> {
        self.init_with_optional_parent(layer, None)
    }

    /// Initializes the object into a layer with a parent object.
    pub fn init_with_parent(self, parent: &Object) -> Result<Object> {
        let layer = parent.layer();
        self.init_with_optional_parent(layer, Some(parent))
    }

    /// Initializes the object into a layer with an optional parent object.
    pub fn init_with_optional_parent(
        mut self,
        layer: &Arc<Layer>,
        parent: Option<&Object>,
    ) -> Result<Object> {
        // Init ID of this object.
        let id = layer.increment_id();

        #[cfg(feature = "physics")]
        let mut rigid_body_parent;
        let parent: NObject = if let Some(parent) = parent {
            let Ok(parent) = parent.as_node() else {
                return Err(anyhow!("Parent uninitialized"));
            };
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
        let parent_transform = self
            .physics
            .update(
                &self.transform,
                &mut parent.lock(),
                &mut rigid_body_parent,
                id as u128,
                &mut layer.physics().lock(),
            )
            .ok_or(Error::msg(
                "Could not update the physics side of this object.",
            ))?;
        #[cfg(not(feature = "physics"))]
        let parent_transform = parent.lock().object.public_transform();

        // Make yourself to a node.
        let node: NObject = std::sync::Arc::new_cyclic(|weak| {
            let parent = Some(std::sync::Arc::downgrade(&parent));
            let object = Object {
                transform: self.transform,
                parent_transform,
                #[cfg(feature = "client")]
                appearance: self.appearance,
                id,
                node: weak.clone(),
                parent_node: parent.clone(),
                #[cfg(feature = "physics")]
                physics: self.physics,
                layer: Some(layer.clone()),
            };
            Mutex::new(Node {
                object,
                // parent: parent.clone(),
                #[cfg(feature = "physics")]
                rigid_body_parent: rigid_body_parent.clone(),
                children: vec![],
            })
        });

        let object = node.lock().object.clone();

        // In case there is no rigid body roots make yourself one.
        #[cfg(feature = "physics")]
        if let Some(value) = &rigid_body_parent {
            if value.is_none() && object.physics.rigid_body.is_some() {
                layer.rigid_body_roots().lock().insert(id, node.clone());
            }
        }

        // Add yourself to the objects map.
        layer.add_object(id, &node);

        // Add yourself to the list of children of the parent.
        parent.lock().children.push(node.clone());
        Ok(object)
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
    pub(crate) fn root(node: WeakObject) -> Self {
        Self {
            transform: Transform::default(),
            parent_transform: Transform::default(),
            #[cfg(feature = "client")]
            appearance: Appearance::default(),
            id: 0,
            node,
            parent_node: None,
            #[cfg(feature = "physics")]
            physics: ObjectPhysics::default(),
            layer: None,
        }
    }

    pub fn layer(&self) -> &Arc<Layer> {
        self.layer.as_ref().unwrap()
    }

    pub(crate) fn parent_node(&self) -> NObject {
        self.parent_node.as_ref().unwrap().upgrade().unwrap()
    }

    /// Returns false if the `remove` function was called on another instance of this object before.
    pub fn is_initialized(&self) -> bool {
        self.as_node().is_ok()
    }

    /// Removes the object from it's layer in case it is still initialized.
    #[allow(unused_mut)]
    pub fn remove(mut self) -> Result<NewObject> {
        let layer = self.layer.as_ref().unwrap();
        let mut map = layer.objects_map.lock();
        #[cfg(feature = "physics")]
        let mut rigid_bodies = layer.rigid_body_roots().lock();
        let node = map.remove(&self.id).ok_or(ObjectError::Uninit)?;

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

        let mut parent_node = self.parent_node();
        parent_node.lock().remove_child(&node)?;

        Ok(NewObject {
            transform: self.transform,
            #[cfg(feature = "client")]
            appearance: self.appearance,
            #[cfg(feature = "physics")]
            physics: self.physics,
        })
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
    pub fn id(&self) -> &usize {
        &self.id
    }

    pub(crate) fn as_node(&self) -> Result<NObject, ObjectError> {
        self.node.upgrade().ok_or(ObjectError::Uninit)
    }

    /// Updates the object to match the object information located inside the system of the layer. Useful when having physics.
    pub fn update(&mut self) -> Result<(), ObjectError> {
        // receive
        let node = self.as_node()?;
        let object = &node.lock().object;
        self.transform = object.transform;
        #[cfg(feature = "client")]
        {
            self.appearance = object.appearance().clone();
        }
        Ok(())
    }

    /// Updates the object inside the layer system to match with this one. Useful when doing anything to the object and submitting it with this function.
    pub fn sync(&mut self) -> Result<(), ObjectError> {
        // send
        // update public position of all children recursively
        let node = self.as_node()?.clone();
        #[cfg(feature = "physics")]
        {
            let layer = self.layer().clone();
            let parent_node = self.parent_node();
            let mut parent = parent_node.lock();
            let mut node = node.lock();
            let mut physics = layer.physics().lock();
            self.parent_transform = self
                .physics
                .update(
                    &self.transform,
                    &mut parent,
                    &mut node.rigid_body_parent,
                    self.id as u128,
                    &mut physics,
                )
                .unwrap();
        }
        let mut node = node.lock();
        node.update_children_position(self.public_transform());
        node.object = self.clone();
        Ok(())
    }

    /// Moves an object to the given index in the children order of the object it is inside right now.
    ///
    /// It returns an error in case the given index is not covered.
    pub fn move_to(&self, index: usize) -> Result<(), ObjectError> {
        self.layer().move_to(self, index)
    }

    /// Moves an object up one item in it's parents children order.
    pub fn move_up(&self) -> Result<(), ObjectError> {
        self.layer().move_up(self)
    }

    /// Moves an object down one item in it's parents children order.
    pub fn move_down(&self) -> Result<(), ObjectError> {
        self.layer().move_down(self)
    }

    /// Moves an object completely up in it's parents children order.
    pub fn move_to_top(&self) -> Result<(), ObjectError> {
        self.layer().move_to_top(self)
    }

    /// Moves an object completely down in it's parents children order.
    pub fn move_to_bottom(&self) -> Result<(), ObjectError> {
        self.layer().move_to_bottom(self)
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

// Object based errors.

use thiserror::Error;

/// This error gets returned when the layer that gets specified when an object needs to get added
/// does not exit in the objects list anymore.
#[derive(Error, Debug)]
#[error("No Layer found")]
pub struct NoLayerError;

/// Errors that happen in object and layer functions.
#[derive(Error, Debug)]
pub enum ObjectError {
    /// The move operation has failed.
    #[error("This object can not be moved to this position:\n{0}")]
    Move(String),
    /// This object does not have a parent.
    #[error("This object does not have a parent. This operation can not be applied.")]
    NoParent,
    /// The object you are trying to access is not initialized anymore.
    #[error("This object was removed from the objects list.")]
    Uninit,
}
