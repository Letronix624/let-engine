//! Objects to be drawn to the screen.

pub mod labels;
pub mod physics;
pub mod scenes;
use crate::{
    error::{objects::NoObjectError, textures::*},
    prelude::*,
    utils::color_art_to_array,
};
use color_art::{color, Color};
use scenes::Layer;

use anyhow::Result;
use glam::f32::{vec2, Vec2};
use hashbrown::HashMap;
use parking_lot::Mutex;

use std::{
    default,
    sync::{Arc, Weak},
};
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

#[derive(Clone)]
pub(crate) struct VisualObject {
    pub transform: Transform,
    pub appearance: Appearance,
}
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
    pub rigid_body_parent: RigidBodyParent,
    pub children: Vec<Arc<Mutex<Node<T>>>>,
}

impl Node<Object> {
    /// Takes a vector of every object transform and appearance and fills it with the right drawing order based on the root node inserted.
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
    pub fn remove_children(&mut self, objects: &mut ObjectsMap, rigid_bodies: &mut ObjectsMap) {
        self.object.physics.remove();
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
            parent.end_transform().combine(self.object.transform)
        } else {
            self.object.transform
        }
    }
}

/// Holds everything about the appearance of objects like
/// textures, vetex/index data, color and material.
#[derive(Debug, Clone, PartialEq)]
pub struct Appearance {
    visible: bool,
    material: Option<materials::Material>,
    model: Option<Model>,
    transform: Transform,
    color: [f32; 4],
    color_art: Color,
}

use paste::paste;

use self::physics::Collider;
macro_rules! getters_and_setters {
    ($field:ident, $title:expr, $type:ty) => {
        #[doc=concat!("Sets ", $title, " of this appearance and returns self.")]
        #[inline]
        pub fn $field(mut self, $field: $type) -> Self {
            self.$field = $field;
            self
        }
        paste! {
            #[doc=concat!("Sets ", $title, " of this appearance.")]
            #[inline]
            pub fn [<set_ $field>](&mut self, $field: $type) {
                self.$field = $field;
            }
        }
        paste! {
            #[doc=concat!("Gets ", $title," of this appearance.")]
            #[inline]
            pub fn [<get_ $field>](&self) -> &$type {
                &self.$field
            }
        }
        paste! {
            #[doc=concat!("Gets a mutable reference to ", $title," of this appearance.")]
            #[inline]
            pub fn [<get_mut_ $field>](&mut self) -> &mut $type {
                &mut self.$field
            }
        }
    };
}

impl Appearance {
    /// Makes a default appearance.
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    /// Scales the object appearance according to the texture applied. Works best in Expand camera mode for best quality.
    pub fn auto_scale(&mut self) -> Result<(), NoTextureError> {
        let dimensions;
        if let Some(material) = &self.material {
            dimensions = if let Some(texture) = &material.texture {
                texture.dimensions()
            } else {
                return Err(NoTextureError);
            };
        } else {
            return Err(NoTextureError);
        };

        self.transform.size = vec2(dimensions.0 as f32 / 1000.0, dimensions.1 as f32 / 1000.0);

        Ok(())
    }

    getters_and_setters!(visible, "the visibility", bool);
    getters_and_setters!(model, "the model", Option<Model>);
    getters_and_setters!(transform, "the transform", Transform);
    getters_and_setters!(material, "the material", Option<materials::Material>);

    /// Sets the color of this appearance and returns self.
    #[inline]
    pub fn color(mut self, color: Color) -> Self {
        self.color_art = color;
        self.color = color_art_to_array(color);
        self
    }
    /// Gets the color of this appearance.
    #[inline]
    pub fn get_color(&self) -> Color {
        self.color_art
    }
    /// Returns the color as an array of 4 f32s symbolizing rgba going from 0 to 1.
    #[inline]
    pub fn get_color_array(&self) -> [f32; 4] {
        self.color
    }
    /// Sets the color of this appearance.
    #[inline]
    pub fn set_color(mut self, color: Color) {
        self.color_art = color;
        self.color = color_art_to_array(color);
    }
}

impl default::Default for Appearance {
    fn default() -> Self {
        Self {
            visible: true,
            material: None,
            model: None,
            transform: Transform::default(),
            color_art: color!(rgba(255, 255, 255, 1.0)),
            color: [1.0; 4],
        }
    }
}

#[derive(Default, Clone)]
pub struct Object {
    pub transform: Transform,
    parent_transform: Transform,
    pub appearance: Appearance,
    id: usize,
    reference: Option<WeakObject>,
    pub(crate) physics: ObjectPhysics,
    layer: Option<Layer>,
}

/// New
impl Object {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn init(&mut self, layer: &Layer) {
        self.init_with_optional_parent(layer, None);
    }
    pub fn init_with_parent(&mut self, layer: &Layer, parent: &Object) {
        self.init_with_optional_parent(layer, Some(parent));
    }
    pub fn init_with_optional_parent(&mut self, layer: &Layer, parent: Option<&Object>) {
        self.layer = Some(layer.clone());
        // Init ID of this object.
        self.id = layer.increment_id();
        // Set the physics reference of this object.
        self.physics.physics = Some(layer.physics().clone());

        let mut rigid_body_parent;
        let parent: NObject = if let Some(parent) = parent {
            let parent = parent.as_node().unwrap(); // Uninitialized parent error.
            rigid_body_parent = parent.lock().rigid_body_parent.clone();
            parent.clone()
        } else {
            rigid_body_parent = None;
            layer.root.clone()
        };
        // Updates the physics side and returns the parent position.
        self.parent_transform = self.physics.update(
            &self.transform,
            &parent,
            &mut rigid_body_parent,
            self.id as u128,
        );

        // Make yourself to a node.
        let node: NObject = std::sync::Arc::new(Mutex::new(Node {
            object: self.clone(),
            parent: Some(std::sync::Arc::downgrade(&parent)),
            rigid_body_parent: rigid_body_parent.clone(),
            children: vec![],
        }));

        // set reference to own node to manipulate.
        self.reference = Some(std::sync::Arc::downgrade(&node));

        // In case there is no rigid body roots make yourself one.
        if let Some(value) = &rigid_body_parent {
            if value.is_none() && self.physics.rigid_body.is_some() {
                layer
                    .rigid_body_roots()
                    .lock()
                    .insert(self.id, node.clone());
            }
        }

        // Add yourself to the objects map.
        layer.add_object(self.id, &node);

        // Add yourself to the list of children of the parent.
        parent.lock().children.push(node.clone());
    }

    /// Removes the object from it's layer.
    pub fn remove(&mut self) -> Result<(), NoObjectError> {
        let mut map = self
            .layer
            .as_ref()
            .expect("uninitialized")
            .objects_map
            .lock(); // uninitialized error
        let mut rigid_bodies = self.layer.as_ref().unwrap().rigid_body_roots().lock();
        let node = if let Some(object) = map.remove(&self.id) {
            object
        } else {
            return Err(NoObjectError);
        };
        rigid_bodies.remove(&self.id);
        // Remove self from the physics side.
        self.physics.remove();

        let mut object = node.lock();
        object.remove_children(&mut map, &mut rigid_bodies);

        let parent = object.parent.clone().unwrap().upgrade().unwrap();
        let mut parent = parent.lock();
        parent.remove_child(&node);

        Ok(())
    }
}

impl Object {
    pub fn set_isometry(&mut self, position: Vec2, rotation: f32) {
        self.transform.position = position;
        self.transform.rotation = rotation;
    }
    pub fn public_transform(&self) -> Transform {
        self.transform.combine(self.parent_transform)
    }
    pub(crate) fn set_parent_transform(&mut self, transform: Transform) {
        self.parent_transform = transform;
    }
    pub fn appearance(&self) -> &Appearance {
        &self.appearance
    }
    pub fn id(&self) -> usize {
        self.id
    }
    pub(crate) fn as_node(&self) -> Option<NObject> {
        self.reference.as_ref()?.upgrade()
    }
    pub(crate) fn rigidbody_handle(&self) -> Option<rapier2d::dynamics::RigidBodyHandle> {
        self.physics.rigid_body_handle
    }
    pub fn update(&mut self) {
        // receive
        if let Some(arc) = self.reference.clone().unwrap().upgrade() {
            let object = &arc.lock().object;
            self.transform = object.transform;
            self.appearance = object.appearance().clone();
        } else {
            self.physics.remove();
        }
    }
    pub fn sync(&mut self) {
        // send
        // update public position of all children recursively
        let node = self.as_node().expect("uninitialized"); // Return error in the future.
        {
            let mut node = node.lock();
            self.parent_transform = self.physics.update(
                &self.transform,
                &node.parent.clone().unwrap().upgrade().unwrap(),
                &mut node.rigid_body_parent,
                self.id as u128,
            );
        }
        node.lock()
            .update_children_position(self.public_transform());
        let arc = self.reference.clone().unwrap().upgrade().unwrap();
        let mut object = arc.lock();
        object.object = self.clone();
    }
    pub fn collider(&self) -> Option<&Collider> {
        self.physics.collider.as_ref()
    }
    pub fn set_collider(&mut self, collider: Option<Collider>) {
        self.physics.collider = collider;
    }
    pub fn collider_mut(&mut self) -> &mut Option<Collider> {
        &mut self.physics.collider
    }
    pub fn rigid_body(&self) -> Option<&RigidBody> {
        self.physics.rigid_body.as_ref()
    }
    pub fn set_rigid_body(&mut self, rigid_body: Option<RigidBody>) {
        self.physics.rigid_body = rigid_body;
    }
    pub fn rigid_body_mut(&mut self) -> &mut Option<RigidBody> {
        &mut self.physics.rigid_body
    }
    pub fn local_collider_position(&self) -> Vec2 {
        self.physics.local_collider_position
    }
    pub fn set_local_collider_position(&mut self, pos: Vec2) {
        self.physics.local_collider_position = pos;
    }
}
