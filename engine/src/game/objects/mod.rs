//! Objects to be drawn to the screen.

pub mod labels;
pub mod physics;
pub mod scenes;
use crate::{
    error::{objects::NoObjectError, textures::*},
    prelude::*,
};
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
    color: Color,
}

use paste::paste;

use self::physics::Collider;
macro_rules! getters_and_setters {
    ($field:ident, $title:expr, $type:ty) => {
        #[doc=concat!("Sets ", $title, " of this appearance and returns self.")]
        #[inline]
        pub fn $field(mut self, $field: impl Into<$type>) -> Self {
            self.$field = $field.into();
            self
        }
        paste! {
            #[doc=concat!("Sets ", $title, " of this appearance.")]
            #[inline]
            pub fn [<set_ $field>](&mut self, $field: impl Into<$type>) {
                self.$field = $field.into();
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
    getters_and_setters!(color, "the color", Color);
}

impl default::Default for Appearance {
    fn default() -> Self {
        Self {
            visible: true,
            material: None,
            model: None,
            transform: Transform::default(),
            color: Color::WHITE,
        }
    }
}

/// Object to be rendered on the screen and get the physics processed of.
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

/// A struct that represents a color to use on objects, the clear color or labels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    rgba: [f32; 4],
}

/// Declaration
impl Color {
    pub const WHITE: Self = Self::from_rgb(1.0, 1.0, 1.0);
    pub const BLACK: Self = Self::from_r(0.0);

    /// Makes a color from red, green, blue and alpha.
    #[inline]
    pub const fn from_rgba(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            rgba: [red, green, blue, alpha],
        }
    }
    /// Makes a color from red, green and blue.
    #[inline]
    pub const fn from_rgb(red: f32, green: f32, blue: f32) -> Self {
        Self {
            rgba: [red, green, blue, 1.0],
        }
    }
    /// Makes a color from red and green.
    #[inline]
    pub const fn from_rg(red: f32, green: f32) -> Self {
        Self {
            rgba: [red, green, 0.0, 1.0],
        }
    }
    /// Makes a color from red and blue.
    #[inline]
    pub const fn from_rb(red: f32, blue: f32) -> Self {
        Self {
            rgba: [red, 0.0, blue, 1.0],
        }
    }
    /// Makes a color from red.
    #[inline]
    pub const fn from_r(red: f32) -> Self {
        Self {
            rgba: [red, 0.0, 0.0, 1.0],
        }
    }
    /// Makes a color from green and blue.
    #[inline]
    pub const fn from_gb(green: f32, blue: f32) -> Self {
        Self {
            rgba: [0.0, green, blue, 1.0],
        }
    }
    /// Makes a color from green.
    #[inline]
    pub const fn from_g(green: f32) -> Self {
        Self {
            rgba: [0.0, green, 0.0, 1.0],
        }
    }
    /// Makes a color from blue.
    #[inline]
    pub const fn from_b(blue: f32) -> Self {
        Self {
            rgba: [0.0, 0.0, blue, 1.0],
        }
    }
}

/// Usage
impl Color {
    /// Returns the red green blue and alpha of this color.
    #[inline]
    pub fn rgba(&self) -> [f32; 4] {
        self.rgba
    }

    /// Returns the red green and blue of this color.
    #[inline]
    pub fn rgb(&self) -> [f32; 3] {
        [self.rgba[0], self.rgba[1], self.rgba[2]]
    }

    /// Returns the red of this color.
    #[inline]
    pub fn r(&self) -> f32 {
        self.rgba[0]
    }

    /// Returns the green of this color.
    #[inline]
    pub fn g(&self) -> f32 {
        self.rgba[1]
    }

    /// Returns the blue of this color.
    #[inline]
    pub fn b(&self) -> f32 {
        self.rgba[2]
    }

    /// Returns the alpha or transparency of this color.
    #[inline]
    pub fn a(&self) -> f32 {
        self.rgba[3]
    }

    /// Sets the red channel of this color.
    #[inline]
    pub fn set_r(&mut self, red: f32) {
        self.rgba[0] = red;
    }

    /// Sets the green channel of this color.
    #[inline]
    pub fn set_g(&mut self, green: f32) {
        self.rgba[1] = green;
    }

    /// Sets the blue channel of this color.
    #[inline]
    pub fn set_b(&mut self, blue: f32) {
        self.rgba[2] = blue;
    }

    /// Sets the alpha channel of this color.
    #[inline]
    pub fn set_a(&mut self, alpha: f32) {
        self.rgba[3] = alpha;
    }

    /// Interpolates to the next color.
    #[inline]
    pub fn lerp(self, rhs: Self, s: f32) -> Self {
        self + ((rhs - self) * s)
    }
}

impl From<[f32; 4]> for Color {
    #[inline]
    fn from(value: [f32; 4]) -> Self {
        Color::from_rgba(value[0], value[1], value[2], value[3])
    }
}
impl From<[f32; 3]> for Color {
    #[inline]
    fn from(value: [f32; 3]) -> Self {
        Color::from_rgb(value[0], value[1], value[2])
    }
}
impl From<f32> for Color {
    #[inline]
    fn from(value: f32) -> Self {
        Color::from_r(value)
    }
}
impl From<Color> for f32 {
    #[inline]
    fn from(value: Color) -> f32 {
        value.r()
    }
}
impl From<Color> for [f32; 3] {
    #[inline]
    fn from(value: Color) -> [f32; 3] {
        value.rgb()
    }
}
impl From<Color> for [f32; 4] {
    #[inline]
    fn from(value: Color) -> [f32; 4] {
        value.rgba()
    }
}
impl std::ops::Add<Color> for Color {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            rgba: [
                self.rgba[0] + rhs.rgba[0],
                self.rgba[1] + rhs.rgba[1],
                self.rgba[2] + rhs.rgba[2],
                self.rgba[3] + rhs.rgba[3],
            ],
        }
    }
}

impl std::ops::Sub<Color> for Color {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            rgba: [
                self.rgba[0] - rhs.rgba[0],
                self.rgba[1] - rhs.rgba[1],
                self.rgba[2] - rhs.rgba[2],
                self.rgba[3] - rhs.rgba[3],
            ],
        }
    }
}

impl std::ops::Mul<Color> for Color {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            rgba: [
                self.rgba[0] * rhs.rgba[0],
                self.rgba[1] * rhs.rgba[1],
                self.rgba[2] * rhs.rgba[2],
                self.rgba[3] * rhs.rgba[3],
            ],
        }
    }
}

impl std::ops::Div<Color> for Color {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        Self {
            rgba: [
                self.rgba[0] / rhs.rgba[0],
                self.rgba[1] / rhs.rgba[1],
                self.rgba[2] / rhs.rgba[2],
                self.rgba[3] / rhs.rgba[3],
            ],
        }
    }
}

impl std::ops::Add<f32> for Color {
    type Output = Self;
    #[inline]
    fn add(self, rhs: f32) -> Self::Output {
        Self {
            rgba: self.rgba.map(|x| x + rhs),
        }
    }
}

impl std::ops::Sub<f32> for Color {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: f32) -> Self::Output {
        Self {
            rgba: self.rgba.map(|x| x - rhs),
        }
    }
}

impl std::ops::Mul<f32> for Color {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        Self {
            rgba: self.rgba.map(|x| x * rhs),
        }
    }
}

impl std::ops::Div<f32> for Color {
    type Output = Self;
    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        Self {
            rgba: self.rgba.map(|x| x / rhs),
        }
    }
}

impl std::ops::AddAssign<Color> for Color {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.rgba[0].add_assign(rhs.rgba[0]);
        self.rgba[1].add_assign(rhs.rgba[1]);
        self.rgba[2].add_assign(rhs.rgba[2]);
        self.rgba[3].add_assign(rhs.rgba[3]);
    }
}

impl std::ops::SubAssign<Color> for Color {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.rgba[0].sub_assign(rhs.rgba[0]);
        self.rgba[1].sub_assign(rhs.rgba[1]);
        self.rgba[2].sub_assign(rhs.rgba[2]);
        self.rgba[3].sub_assign(rhs.rgba[3]);
    }
}

impl std::ops::MulAssign<Color> for Color {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        self.rgba[0].mul_assign(rhs.rgba[0]);
        self.rgba[1].mul_assign(rhs.rgba[1]);
        self.rgba[2].mul_assign(rhs.rgba[2]);
        self.rgba[3].mul_assign(rhs.rgba[3]);
    }
}

impl std::ops::DivAssign<Color> for Color {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        self.rgba[0].div_assign(rhs.rgba[0]);
        self.rgba[1].div_assign(rhs.rgba[1]);
        self.rgba[2].div_assign(rhs.rgba[2]);
        self.rgba[3].div_assign(rhs.rgba[3]);
    }
}

impl std::ops::AddAssign<f32> for Color {
    #[inline]
    fn add_assign(&mut self, rhs: f32) {
        self.rgba.map(|mut x| x.add_assign(rhs));
    }
}

impl std::ops::SubAssign<f32> for Color {
    #[inline]
    fn sub_assign(&mut self, rhs: f32) {
        self.rgba.map(|mut x| x.sub_assign(rhs));
    }
}

impl std::ops::MulAssign<f32> for Color {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.rgba.map(|mut x| x.mul_assign(rhs));
    }
}

impl std::ops::DivAssign<f32> for Color {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        self.rgba.map(|mut x| x.div_assign(rhs));
    }
}

/// New
impl Object {
    /// Returns a default object
    pub fn new() -> Self {
        Self::default()
    }
    /// Initializes the object into a layer.
    pub fn init(&mut self, layer: &Layer) {
        self.init_with_optional_parent(layer, None);
    }
    /// Initializes the object into a layer with a parent object.
    pub fn init_with_parent(&mut self, layer: &Layer, parent: &Object) {
        self.init_with_optional_parent(layer, Some(parent));
    }
    /// Initializes the object into a layer with an optional parent object.
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
        self.id = 0;

        Ok(())
    }
}

/// Setters
impl Object {
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
    pub fn appearance(&self) -> &Appearance {
        &self.appearance
    }

    /// Returns the identification number of the object specific the layer it's inside right now.
    pub fn id(&self) -> usize {
        self.id
    }
    pub(crate) fn as_node(&self) -> Option<NObject> {
        self.reference.as_ref()?.upgrade()
    }
    pub(crate) fn rigidbody_handle(&self) -> Option<rapier2d::dynamics::RigidBodyHandle> {
        self.physics.rigid_body_handle
    }
    /// Updates the object to match the object information located inside the system of the layer. Useful when having physics.
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
    /// Updates the object inside the layer system to match with this one. Useful when doing anything to the object and submitting it with this function.
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
