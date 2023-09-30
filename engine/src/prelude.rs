//! # usage:
//! ```rust
//! use let_engine::prelude::*;
//! ```
//! - imports everything this engine has to offer.

pub use crate::{
    data::*, directions::*, materials::*, objects::*, resources::*, camera::*, *
};
pub use engine_macros::*;
pub use glam;
pub use glam::{vec2, Vec2};
pub use joints::*;
pub use physics::*;
pub use rapier2d::prelude::CoefficientCombineRule;
pub use textures::*;
pub use labels::*;

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
