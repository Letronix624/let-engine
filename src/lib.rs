pub mod error;
mod game;
pub mod texture;

pub use game::{
    materials, Appearance, CameraOption, CameraScaling, Data, Game, GameBuilder, Object, Vertex,
};

pub use game::objects::data::{CENTER, N, NO, NW, O, S, SO, SW, W};

/// Information about your game.
#[derive(Clone, Copy)]
pub struct AppInfo {
    pub app_name: &'static str,
}
