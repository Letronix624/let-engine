//! Objects to be drawn to the screen.

mod appearance;
mod color;
pub use appearance::*;
use bytemuck::AnyBitPattern;
pub use color::Color;

#[cfg(feature = "physics")]
pub mod physics;
use let_engine_macros::Vertex;
#[cfg(feature = "physics")]
use physics::*;

use super::scenes::LayerId;

use derive_builder::Builder;
use slotmap::new_key_type;

use glam::{Mat4, Quat, Vec2};

/// Holds position size and rotation of an object.
#[derive(Clone, Copy, Debug, PartialEq, Vertex, AnyBitPattern)]
pub struct Transform {
    #[format(Rg32Float)]
    pub position: Vec2,
    #[format(Rg32Float)]
    pub size: Vec2,
    #[format(R32Float)]
    pub rotation: f32,
}

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

        // Combine the sizes
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

//     /// Takes a vector of every object transform and appearance and fills it with the right client order based on the root node inserted.
//     pub fn order_position(order: &mut Vec<VisualObject<T>>, objects: &Self) {
//         for child in objects.children.iter() {
//             let child = child.lock();
//             if !child.object.appearance.visible() {
//                 continue;
//             }
//             let object = VisualObject::combined(&child.object, &objects.object);
//             order.push(object.clone());
//             for child in child.children.iter() {
//                 let child = child.lock();
//                 if !child.object.appearance.visible() {
//                     continue;
//                 }
//                 order.push(VisualObject {
//                     transform: child.object.transform.combine(object.transform),
//                     appearance: child.object.appearance().clone(),
//                 });
//                 Self::order_position(order, &child);
//             }
//         }
//     }

/// Object to be initialized to the layer.
#[derive(Clone, Builder)]
pub struct ObjectBuilder<T: Loaded> {
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
    pub appearance: Appearance<T>,
    pub(crate) children: Vec<ObjectId>,
    pub(crate) parent_id: Option<ObjectId>,
    pub(crate) layer_id: LayerId,
    #[cfg(feature = "physics")]
    pub(crate) physics: ObjectPhysics,
}

new_key_type! { pub struct ObjectId; }

impl<T: Loaded> ObjectBuilder<T> {
    /// Returns a default object
    pub fn new(appearance: Appearance<T>) -> Self {
        Self {
            transform: Transform::default(),
            appearance,
            #[cfg(feature = "physics")]
            physics: ObjectPhysics::default(),
        }
    }
}

/// Setters
impl<T: Loaded> ObjectBuilder<T> {
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
impl<T: Loaded> ObjectBuilder<T> {
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
    pub fn layer_id(&self) -> LayerId {
        self.layer_id
    }

    pub fn parent(&self) -> Option<ObjectId> {
        self.parent_id
    }

    // /// Combines the object position data.
    // pub fn combined(object: &Object<T>, parent: &Object<T>) -> Self {
    //     let transform = object.transform.combine(parent.public_transform());
    //     Self {
    //         transform,
    //         appearance: object.appearance.clone(),
    //     }
    // }

    /// Creates a model matrix for the given object.
    pub fn make_model_matrix(&self) -> Mat4 {
        let transform = self.appearance.transform().combine(self.transform);

        Mat4::from_scale_rotation_translation(
            transform.size.extend(0.0),
            Quat::from_rotation_z(transform.rotation),
            transform.position.extend(0.0),
        )
    }

    // /// Removes the object from it's layer in case it is still initialized.
    // #[allow(unused_mut)]
    // pub fn remove(mut self) -> Result<ObjectBuilder<T>> {
    //     let layer = self.layer_id.as_ref().unwrap();
    //     let mut map = layer.objects_map.lock();
    //     #[cfg(feature = "physics")]
    //     let mut rigid_bodies = layer.rigid_body_roots().lock();
    //     let node = map.remove(&self.id).ok_or(ObjectError::Uninit)?;

    //     #[cfg(feature = "physics")]
    //     {
    //         rigid_bodies.remove(&self.id);
    //         // Remove self from the physics side.
    //         self.physics.remove(layer.physics());
    //     }

    //     let mut object = node.lock();
    //     object.remove_children(
    //         &mut map,
    //         #[cfg(feature = "physics")]
    //         &mut rigid_bodies,
    //     );

    //     if let Some(mut parent_node) = self.parent_node() {
    //         parent_node.lock().remove_child(&node)?;
    //     };

    //     Ok(ObjectBuilder {
    //         transform: self.transform,
    //         appearance: self.appearance,
    //         #[cfg(feature = "physics")]
    //         physics: self.physics,
    //     })
    // }

    /// Makes a new object from this object.
    pub fn to_builder(&self) -> ObjectBuilder<T> {
        ObjectBuilder {
            transform: self.transform,
            appearance: self.appearance.clone(),
            #[cfg(feature = "physics")]
            physics: self.physics.clone(),
        }
    }

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

use crate::backend::gpu::Loaded;

/// Errors that happen in object and layer functions.
#[derive(Error, Debug)]
pub enum ObjectError {
    /// The object you are trying to access is not initialized anymore.
    #[error("This object was removed from the objects list.")]
    NoObject,
}
