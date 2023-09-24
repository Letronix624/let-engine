pub use crate::{
    make_circle, materials, physics::*, vert, tvert, Appearance, CameraScaling, CameraSettings, Data,
    Font, Game, GameBuilder, GameObject, Label, LabelCreateInfo, Layer, Resources, Scene, Time,
    Transform, Vertex, CENTER, N, NO, NW, O, S, SO, SW, W,
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
pub struct Object {}

/// Default Camera object without any additional fields.
#[camera]
#[derive(Default)]
pub struct Camera {}
