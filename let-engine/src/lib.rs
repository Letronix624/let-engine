//! [![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/Letronix624/let-engine/rust.yml?style=for-the-badge&logo=github&label=GitHub&color=9376e0)](https://github.com/Letronix624/let-engine)
//! [![Crates.io](https://img.shields.io/crates/d/let-engine?style=for-the-badge&logo=rust&label=Crates.io&color=e893cf)](https://crates.io/crates/let-engine)
//! [![Static Badge](https://img.shields.io/badge/Docs-passing?style=for-the-badge&logo=docsdotrs&color=f3bcc8&link=let-server.net%2Fdocs%2Flet_engine)](https://docs.rs/let_engine/)
//! [![Website](https://img.shields.io/website?up_message=Up&up_color=f6ffa6&down_message=Down&down_color=lightgrey&url=https%3A%2F%2Flet-server.net%2F&style=for-the-badge&logo=apache&color=f6ffa6&link=https%3A%2F%2Flet-server.net%2F)](https://let-server.net/)
//!
//! A Game engine made in Rust.
mod game;
use std::sync::Arc;

#[cfg(feature = "asset_system")]
pub use asset_system;
pub use game::*;
pub mod prelude;
pub use settings;
#[cfg(feature = "audio")]
pub mod sounds;

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

pub use let_engine_core::{camera, make_circle, objects, resources, Direction};

/// Structs about drawing related things.
pub mod draw {
    pub use let_engine_core::draw::{Graphics, PresentMode, ShaderError, VulkanError};
}
use once_cell::sync::Lazy;

/// General time methods of the game engine.
pub static TIME: Lazy<Time> = Lazy::new(Time::default);
/// The input system holding the state of every key and the mouse position.
#[cfg(feature = "client")]
pub static INPUT: Lazy<input::Input> = Lazy::new(input::Input::new);

/// General settings for the game engine.
#[cfg(all(feature = "client", feature = "audio"))]
pub static SETTINGS: Lazy<game::settings::Settings<Arc<draw::Graphics>, sounds::Audio>> =
    Lazy::new(game::settings::Settings::new);
/// General settings for the game engine.
#[cfg(all(feature = "client", not(feature = "audio")))]
pub static SETTINGS: Lazy<game::settings::Settings<Arc<Graphics>>> =
    Lazy::new(game::settings::Settings::new);
/// General settings for the game engine.
#[cfg(not(feature = "client"))]
pub static SETTINGS: Lazy<game::settings::Settings> = Lazy::new(game::settings::Settings::new);
