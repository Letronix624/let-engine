pub mod discord;

mod errors;
mod game;

pub use game::{Appearance, Data, Game, GameBuilder, Object};

/// Information about your game.
#[derive(Clone, Copy)]
pub struct AppInfo {
    pub app_name: &'static str,
}
