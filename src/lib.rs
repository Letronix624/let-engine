pub mod discord;

mod game;
mod errors;

pub use game::{objects::Object, Game, GameBuilder, Resources};

/// Information about your game.
#[derive(Clone, Copy)]
pub struct AppInfo {
    pub app_name: &'static str,
}
