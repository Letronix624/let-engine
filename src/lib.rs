pub mod error;
mod game;
pub mod texture;

pub use game::{
    materials, Appearance, CameraOption, CameraScaling, Data, Game, GameBuilder, Layer, Object,
    Resources, Scene, Time, Vertex,
};

pub use game::objects::data::{CENTER, N, NO, NW, O, S, SO, SW, W};
//RE EXPORTS

pub use glam::{vec2, Vec2};
