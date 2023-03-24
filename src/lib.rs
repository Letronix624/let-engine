mod errors;
mod game;

pub use errors::*;
pub use game::objects::data::Vertex;
pub use game::{Appearance, CameraOption, CameraScaling, Data, Game, GameBuilder, Object};

/// Information about your game.
#[derive(Clone, Copy)]
pub struct AppInfo {
    pub app_name: &'static str,
}
