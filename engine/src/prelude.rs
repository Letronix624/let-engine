pub use crate::{
    data::*, directions::*, make_circle, materials::*, objects::*, resources::*, tvert, vert, Game,
    GameBuilder, Scene, Time, Vertex,
};
pub use engine_macros::*;
pub use glam;
pub use glam::{vec2, Vec2};
pub use joints::*;
pub use physics::*;
pub use rapier2d::prelude::CoefficientCombineRule;
pub use textures::*;

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
