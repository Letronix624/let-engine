//! [![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/Letronix624/let-engine/rust.yml?style=for-the-badge&logo=github&label=GitHub&color=9376e0)](https://github.com/Letronix624/let-engine)
//! [![Crates.io](https://img.shields.io/crates/d/let-engine?style=for-the-badge&logo=rust&label=Crates.io&color=e893cf)](https://crates.io/crates/let-engine)
//! [![Static Badge](https://img.shields.io/badge/Docs-passing?style=for-the-badge&logo=docsdotrs&color=f3bcc8&link=let-server.net%2Fdocs%2Flet_engine)](https://docs.rs/let_engine/)
//! [![Website](https://img.shields.io/website?up_message=Up&up_color=f6ffa6&down_message=Down&down_color=lightgrey&url=https%3A%2F%2Flet-server.net%2F&style=for-the-badge&logo=apache&color=f6ffa6&link=https%3A%2F%2Flet-server.net%2F)](https://let-server.net/)
//!
//! A Game engine made in Rust.
pub mod camera;
#[cfg(feature = "client")]
pub(crate) mod draw;
pub mod error;
mod game;
#[cfg(feature = "asset_system")]
pub use asset_system;
pub use game::*;
pub mod objects;
pub mod prelude;
#[cfg(feature = "client")]
pub mod resources;
#[cfg(feature = "client")]
pub(crate) mod utils;

pub use glam::{vec2, Vec2};

#[cfg(not(feature = "client"))]
mod check_feature_dependency {
    #[cfg(feature = "egui")]
    compile_error!("`egui` requires the `client` feature to be enabled.");
    #[cfg(feature = "labels")]
    compile_error!("`labels` requires the `client` feature to be enabled.");
    #[cfg(feature = "audio")]
    compile_error!("`audio` requires the `client` feature to be enabled.");
}

/// Egui feature on
#[cfg(feature = "egui")]
pub use egui_winit_vulkano::egui;

use once_cell::sync::Lazy;

/// Cardinal direction
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Center,
    N,
    No,
    O,
    So,
    S,
    Sw,
    W,
    Nw,
}

#[cfg(feature = "labels")]
impl From<Direction> for (glyph_brush::HorizontalAlign, glyph_brush::VerticalAlign) {
    fn from(value: Direction) -> Self {
        use glyph_brush::{HorizontalAlign, VerticalAlign};
        let horizontal = match value {
            Direction::Center => HorizontalAlign::Center,
            Direction::N => HorizontalAlign::Center,
            Direction::No => HorizontalAlign::Right,
            Direction::O => HorizontalAlign::Right,
            Direction::So => HorizontalAlign::Right,
            Direction::S => HorizontalAlign::Center,
            Direction::Sw => HorizontalAlign::Left,
            Direction::W => HorizontalAlign::Left,
            Direction::Nw => HorizontalAlign::Left,
        };

        let vertical = match value {
            Direction::Center => VerticalAlign::Center,
            Direction::N => VerticalAlign::Top,
            Direction::No => VerticalAlign::Top,
            Direction::O => VerticalAlign::Center,
            Direction::So => VerticalAlign::Bottom,
            Direction::S => VerticalAlign::Bottom,
            Direction::Sw => VerticalAlign::Bottom,
            Direction::W => VerticalAlign::Center,
            Direction::Nw => VerticalAlign::Top,
        };
        (horizontal, vertical)
    }
}

/// The engine wide scene holding all objects in layers.
pub static SCENE: Lazy<objects::scenes::Scene> = Lazy::new(objects::scenes::Scene::default);
/// General time methods of the game engine.
pub static TIME: Lazy<Time> = Lazy::new(Time::default);
/// The input system holding the state of every key and the mouse position.
#[cfg(feature = "client")]
pub static INPUT: Lazy<input::Input> = Lazy::new(input::Input::new);
/// General settings for the game engine.
pub static SETTINGS: Lazy<game::settings::Settings> = Lazy::new(game::settings::Settings::new);
