pub mod error;
mod game;
pub mod texture;

pub use game::{
    materials, Appearance, CameraOption, CameraScaling, Data, Game, GameBuilder, Object, Resources,
    Vertex, Scene
};

pub use game::objects::data::{CENTER, N, NO, NW, O, S, SO, SW, W};
