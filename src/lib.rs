pub mod discord;

mod errors;
mod game;

pub use errors::*;
pub use game::{Appearance, Data, Game, GameBuilder, Object, CameraScaling, CameraOption};
pub use game::objects::data::Vertex;

/// Information about your game.
#[derive(Clone, Copy)]
pub struct AppInfo {
    pub app_name: &'static str,
}
