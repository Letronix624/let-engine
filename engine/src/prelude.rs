pub use crate::{
    make_circle, materials::*, vert, tvert,
    Game, GameBuilder, resources::*, Scene, Time,
    Vertex, directions::*, objects::*
};
pub use physics::*;
pub use joints::*;
pub use textures::*;
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
