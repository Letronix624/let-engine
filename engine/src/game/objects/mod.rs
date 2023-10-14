//! Objects to be drawn to the screen.

pub mod labels;
pub mod physics;
pub mod scenes;
use super::{AObject, NObject};
use crate::error::textures::*;
use crate::materials;
use crate::resources::Model;
use crate::{data::Data, utils::color_art_to_array};
use color_art::{color, Color};
use scenes::Layer;

use anyhow::Result;
use glam::f32::{vec2, Vec2};
use hashbrown::HashMap;
use parking_lot::Mutex;

use std::{
    any::Any,
    default,
    sync::{Arc, Weak},
};
pub type RigidBodyParent = Option<Option<Weak<Mutex<Node<AObject>>>>>;
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
        rigid_body_parent: RigidBodyParent,
        layer: &Layer,
    ) -> NObject;
    fn remove_event(&mut self);
    fn as_any(&self) -> &dyn Any;
    fn as_node(&self) -> NObject;
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
    pub rigid_body_parent: RigidBodyParent,
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
    visible: bool,
    material: Option<materials::Material>,
    model: Option<Model>,
    transform: Transform,
    color: [f32; 4],
    color_art: Color,
}

use paste::paste;
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
