pub mod discord;

mod errors;
mod game;

pub use game::{Data, Display, Game, GameBuilder, Object, ObjectNode, Resources, VisualObject};

/// Information about your game.
#[derive(Clone, Copy)]
pub struct AppInfo {
    pub app_name: &'static str,
}
