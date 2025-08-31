//! Objects to be drawn to the screen.

mod appearance;
mod color;
pub use appearance::*;
use bytemuck::AnyBitPattern;
pub use color::Color;

#[cfg(feature = "physics")]
pub mod physics;
use engine_macros::Vertex;
#[cfg(feature = "physics")]
use physics::*;

pub mod scenes;
use scenes::Layer;

use anyhow::{anyhow, Error, Result};

use derive_builder::Builder;

use crate::{HashMap, Mutex};
use std::sync::{Arc, Weak};

use glam::{vec2, Mat4, Quat, Vec2};

#[cfg(feature = "physics")]
type RigidBodyParent<T> = Option<Option<Weak<Mutex<Node<T>>>>>;
type ObjectsMap<T> = HashMap<usize, NObject<T>>;
pub(crate) type NObject<T> = Arc<Mutex<Node<T>>>;
type WeakObject<T> = Weak<Mutex<Node<T>>>;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Holds position size and rotation of an object.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Vertex, AnyBitPattern)]
pub struct Transform {
    #[format(Rg32Float)]
    pub position: Vec2,
    #[format(Rg32Float)]
    pub size: Vec2,
    #[format(R32Float)]
    pub rotation: f32,
}
impl Eq for Transform {}
impl Transform {
    const ORIGIN: Self = Self {
        position: Vec2::ZERO,
        size: Vec2::ONE,
        rotation: 0.0,
    };

    /// Creates a new [`Transform`].
    #[inline]
    pub fn new(position: Vec2, size: Vec2, rotation: f32) -> Self {
        Self {
            position,
            size,
            rotation,
        }
    }

    /// Creates a new [`Transform`] with the given position.
    #[inline]
    pub fn with_position(position: Vec2) -> Self {
        Self {
            position,
            ..Default::default()
        }
    }

    /// Creates a new [`Transform`] with the given size.
    #[inline]
    pub fn with_size(size: Vec2) -> Self {
        Self {
            size,
            ..Default::default()
        }
    }

    /// Creates a new [`Transform`] with the given rotation.
    #[inline]
    pub fn with_rotation(rotation: f32) -> Self {
        Self {
            rotation,
            ..Default::default()
        }
    }

    /// Creates a new [`Transform`] with the given position and size.
    #[inline]
    pub fn with_position_size(position: Vec2, size: Vec2) -> Self {
        Self {
            position,
            size,
            ..Default::default()
        }
    }

    /// Creates a new [`Transform`] with the given position and rotation.
    #[inline]
    pub fn with_position_rotation(position: Vec2, rotation: f32) -> Self {
        Self {
            position,
            rotation,
            ..Default::default()
        }
    }

    /// Creates a new [`Transform`] with the given size and rotation.
    #[inline]
    pub fn with_size_rotation(size: Vec2, rotation: f32) -> Self {
        Self {
            size,
            rotation,
            ..Default::default()
        }
    }

    /// Combines two Transforms with each other. It adds position, multiplies size and adds rotation.
    pub fn combine(self, parent: Self) -> Self {
        // Calculate the rotation matrix for the parent's rotation
        let rotation_matrix = glam::Mat2::from_angle(parent.rotation);

        // Apply the parent's rotation to the child's position
        let new_position = rotation_matrix * self.position + parent.position;

        // Combine the sizes (assuming sizes scale multiplicatively)
        let new_size = self.size * parent.size;

        // Combine the rotations
        let new_rotation = self.rotation + parent.rotation;

        Transform {
            position: new_position,
            size: new_size,
            rotation: new_rotation,
        }
    }

    /// Creates a view matrix using the transform as a camera orientation.
    pub fn make_view_matrix(&self) -> Mat4 {
        let translation = Mat4::from_translation(self.position.extend(0.0));
        let rotation = Mat4::from_rotation_z(self.rotation);
        let scale = Mat4::from_scale(self.size.extend(1.0));

        (translation * rotation * scale).inverse()
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

/// Returns the origin transform at 0, 0 with rotation 0 and size 1, 1.
impl Default for Transform {
    fn default() -> Self {
        Self::ORIGIN
    }
}

/// Node structure for the layer.
#[derive(Debug)]
pub struct Node<T: Loaded> {
    pub object: Object<T>,
    #[cfg(feature = "physics")]
    pub rigid_body_parent: RigidBodyParent<T>,
    pub children: Vec<Arc<Mutex<Node<T>>>>,
}

impl<T: Loaded> PartialEq for Node<T> {
    fn eq(&self, other: &Self) -> bool {
        self.object == other.object
    }
}

pub struct VisualObject<T: Loaded> {
    pub transform: Transform,
    pub appearance: Appearance<T>,
}

impl<T: Loaded> Clone for VisualObject<T> {
    fn clone(&self) -> Self {
        Self {
            transform: self.transform,
            appearance: self.appearance.clone(),
        }
    }
}

impl<T: Loaded> VisualObject<T> {
    /// Combines the object position data.
    pub fn combined(object: &Object<T>, parent: &Object<T>) -> Self {
        let transform = object.transform.combine(parent.public_transform());
        Self {
            transform,
            appearance: object.appearance.clone(),
        }
    }

    /// Creates a model matrix for the given object.
    pub fn make_model_matrix(&self) -> Mat4 {
        let transform = self.appearance.transform().combine(self.transform);

        Mat4::from_scale_rotation_translation(
            transform.size.extend(0.0),
            Quat::from_rotation_z(transform.rotation),
            transform.position.extend(0.0),
        )
    }
}

impl<T: Loaded> Node<T> {
    /// Takes a vector of every object transform and appearance and fills it with the right client order based on the root node inserted.
    pub fn order_position(order: &mut Vec<VisualObject<T>>, objects: &Self) {
        for child in objects.children.iter() {
            let child = child.lock();
            if !child.object.appearance.visible() {
                continue;
            }
            let object = VisualObject::combined(&child.object, &objects.object);
            order.push(object.clone());
            for child in child.children.iter() {
                let child = child.lock();
                if !child.object.appearance.visible() {
                    continue;
                }
                order.push(VisualObject {
                    transform: child.object.transform.combine(object.transform),
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
    pub fn remove_child(&mut self, object: &NObject<T>) -> Result<()> {
        let index = self
            .children
            .clone()
            .into_iter()
            .position(|x| Arc::ptr_eq(&x, object))
            .ok_or(Error::msg("No child found"))?;
        self.children.remove(index);
        Ok(())
    }

    /// Removes all children and their children recursively from the layer.
    pub fn remove_children(
        &mut self,
        objects: &mut ObjectsMap<T>,
        #[cfg(feature = "physics")] rigid_bodies: &mut ObjectsMap<T>,
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
// #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Builder)]
pub struct NewObject<T: Loaded> {
    #[builder(setter(into), default)]
    pub transform: Transform,
    #[builder(setter(into))]
    pub appearance: Appearance<T>,
    #[builder(setter(skip))]
    #[cfg(feature = "physics")]
    pub(crate) physics: ObjectPhysics,
}

/// An initialized object that gets rendered on the screen.
pub struct Object<T: Loaded = ()> {
    pub transform: Transform,
    parent_transform: Transform,
    pub appearance: Appearance<T>,
    id: usize,
    node: WeakObject<T>,
    parent_node: Option<WeakObject<T>>,
    #[cfg(feature = "physics")]
    pub(crate) physics: ObjectPhysics,
    layer: Option<Arc<Layer<T>>>,
}

impl<T: Loaded> Clone for Object<T> {
    fn clone(&self) -> Self {
        Self {
            transform: self.transform,
            parent_transform: self.parent_transform,
            appearance: self.appearance.clone(),
            id: self.id,
            node: self.node.clone(),
            parent_node: self.parent_node.clone(),
            #[cfg(feature = "physics")]
            physics: self.physics.clone(),
            layer: self.layer.clone(),
        }
    }
}

impl<T: Loaded> std::fmt::Debug for Object<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Object")
            .field("id", &self.id)
            .field("transform", &self.transform)
            .field("parent_transform", &self.parent_transform)
            .finish()
    }
}

impl<T: Loaded> PartialEq for Object<T> {
    fn eq(&self, other: &Self) -> bool {
        self.transform == other.transform
            && self.parent_transform == other.parent_transform
            && self.id == other.id
            && self.layer == other.layer
    }
}
impl<T: Loaded> Eq for Object<T> {}

/// New
impl<T: Loaded> NewObject<T> {
    /// Returns a default object
    pub fn new(appearance: Appearance<T>) -> Self {
        Self {
            transform: Transform::default(),
            appearance,
            #[cfg(feature = "physics")]
            physics: ObjectPhysics::default(),
        }
    }

    /// Initializes the object into a layer.
    pub fn init(self, layer: &Arc<Layer<T>>) -> Result<Object<T>> {
        self.init_with_optional_parent(layer, None)
    }

    /// Initializes the object into a layer with a parent object.
    pub fn init_with_parent(self, parent: &Object<T>) -> Result<Object<T>> {
        let layer = parent.layer();
        self.init_with_optional_parent(layer, Some(parent))
    }

    /// Initializes the object into a layer with an optional parent object.
    #[allow(unused_mut)]
    pub fn init_with_optional_parent(
        mut self,
        layer: &Arc<Layer<T>>,
        parent: Option<&Object<T>>,
    ) -> Result<Object<T>> {
        // Init ID of this object.
        let id = layer.increment_id();

        #[cfg(feature = "physics")]
        let mut rigid_body_parent;
        let parent: Option<NObject<T>> = if let Some(parent) = parent {
            let Ok(parent) = parent.as_node() else {
                return Err(anyhow!("Parent uninitialized"));
            };
            #[cfg(feature = "physics")]
            {
                rigid_body_parent = parent.lock().rigid_body_parent.clone();
            }
            Some(parent.clone())
        } else {
            #[cfg(feature = "physics")]
            {
                rigid_body_parent = None;
            }
            None
        };
        // Updates the physics side and returns the parent position.
        #[cfg(feature = "physics")]
        let parent_transform = {
            if let Some(parent) = &parent {
                let parent_transform = parent.lock().object.transform;
                self.physics
                    .update(
                        self.transform,
                        parent_transform,
                        &mut rigid_body_parent,
                        id as u128,
                        &mut layer.physics().lock(),
                    )
                    .ok_or(Error::msg(
                        "Could not update the physics side of this object.",
                    ))?
            } else {
                Transform::default()
            }
        };

        #[cfg(not(feature = "physics"))]
        let parent_transform = {
            if let Some(parent) = &parent {
                parent.lock().object.public_transform()
            } else {
                Transform::default()
            }
        };

        // Make yourself to a node.
        let node: NObject<T> = std::sync::Arc::new_cyclic(|weak| {
            let parent = parent.clone().map(|x| Arc::downgrade(&x));
            let object = Object {
                transform: self.transform,
                parent_transform,
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
        if let Some(parent) = parent {
            parent.lock().children.push(node.clone());
        } else {
            layer.objects.lock().push(node.clone());
        }

        Ok(object)
    }
}

/// Setters
impl<T: Loaded> NewObject<T> {
    /// Sets the position and rotation of an object.
    pub fn set_isometry(&mut self, position: Vec2, rotation: f32) {
        self.transform.position = position;
        self.transform.rotation = rotation;
    }
    /// Returns a reference to the appearance of the object.
    pub fn appearance(&self) -> &Appearance<T> {
        &self.appearance
    }
}

/// Physics
#[cfg(feature = "physics")]
impl<T: Loaded> NewObject<T> {
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

impl<T: Loaded> Object<T> {
    pub fn layer(&self) -> &Arc<Layer<T>> {
        self.layer.as_ref().unwrap()
    }

    pub(crate) fn parent_node(&self) -> Option<NObject<T>> {
        self.parent_node.as_ref().map(|x| x.upgrade().unwrap())
    }

    /// Returns false if the `remove` function was called on another instance of this object before.
    pub fn is_initialized(&self) -> bool {
        self.as_node().is_ok()
    }

    /// Removes the object from it's layer in case it is still initialized.
    #[allow(unused_mut)]
    pub fn remove(mut self) -> Result<NewObject<T>> {
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

        if let Some(mut parent_node) = self.parent_node() {
            parent_node.lock().remove_child(&node)?;
        };

        Ok(NewObject {
            transform: self.transform,
            appearance: self.appearance,
            #[cfg(feature = "physics")]
            physics: self.physics,
        })
    }

    /// Makes a new object from this object.
    pub fn to_new(&self) -> NewObject<T> {
        NewObject {
            transform: self.transform,
            appearance: self.appearance.clone(),
            #[cfg(feature = "physics")]
            physics: self.physics.clone(),
        }
    }

    /// Copies the data from a `NewObject` into itself.
    pub fn copy_new(&mut self, object: NewObject<T>) {
        self.transform = object.transform;
        #[cfg(feature = "physics")]
        {
            self.physics = object.physics;
        }
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
    pub fn appearance(&self) -> &Appearance<T> {
        &self.appearance
    }

    /// Returns the identification number of the object specific the layer it is inside right now.
    ///
    /// Returns 0 in case it is not initialized to a layer yet.
    pub fn id(&self) -> &usize {
        &self.id
    }

    pub(crate) fn as_node(&self) -> Result<NObject<T>, ObjectError> {
        self.node.upgrade().ok_or(ObjectError::Uninit)
    }

    /// Updates the object to match the object information located inside the system of the layer. Useful when having physics.
    pub fn update(&mut self) -> Result<(), ObjectError> {
        // receive
        let node = self.as_node()?;
        let object = &node.lock().object;
        self.transform = object.transform;
        self.appearance = object.appearance().clone();
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
            let parent_transform = parent_node
                .map(|x| x.lock().object.transform)
                // No parent -> Parent transform at origin
                .unwrap_or_default();

            let mut node = node.lock();
            let mut physics = layer.physics().lock();
            self.parent_transform = self
                .physics
                .update(
                    self.transform,
                    parent_transform,
                    &mut node.rigid_body_parent,
                    self.id as u128,
                    &mut physics,
                )
                .unwrap();
        }
        let mut node = node.lock();
        node.update_children_position(self.parent_transform);
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
impl<T: Loaded> Object<T> {
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

use crate::backend::graphics::Loaded;

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

    /// The object you are trying to access is not initialized anymore.
    #[error("This object was removed from the objects list.")]
    Uninit,
}
