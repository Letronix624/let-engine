pub mod discord;

mod game;

pub use game::{Game, GameBuilder};

#[derive(Clone, Copy)]
pub struct AppInfo {
    pub app_name: &'static str,
}
