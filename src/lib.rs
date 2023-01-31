pub mod discord;

mod game;

pub use game::Game;

#[derive(Clone, Copy)]
pub struct AppInfo {
    pub app_name: &'static str,
}
