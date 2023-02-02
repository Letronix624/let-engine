pub mod discord;

mod game;
mod errors;

pub use game::{Object, Game, GameBuilder, Resources, VisualObject, Display, Data};

/// Information about your game.
#[derive(Clone, Copy)]
pub struct AppInfo {
    pub app_name: &'static str,
}
