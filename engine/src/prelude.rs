pub use crate::{
    Appearance, CameraScaling, CameraSettings, Data, Font, Game, GameBuilder, Label,
    LabelCreateInfo, Layer, Resources, Scene, Time, Transform, Vertex,
};
pub use engine_macros::*;
pub use glam;
pub use glam::{vec2, Vec2};
pub use rapier2d::prelude::CoefficientCombineRule;

use crate as let_engine;
//use crate::game::objects::GameObject;
// default objects

/// Default object without any additional fields.
#[object]
#[derive(Default)]
struct Object {}

/// Default Camera object without any additional fields.
#[camera]
#[derive(Default)]
struct Camera {}

/// Default Collider object without any additional fields.
#[collider]
#[derive(Default)]
struct ColliderObject {}
