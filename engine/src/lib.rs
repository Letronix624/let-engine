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
pub use game::*;
pub mod objects;
pub mod prelude;
#[cfg(feature = "client")]
pub mod resources;
#[cfg(feature = "client")]
pub(crate) mod utils;

pub use glam::{vec2, Vec2};

#[cfg(feature = "egui")]
mod check_feature_dependency {
    #[cfg(not(feature = "client"))]
    compile_error!("`egui` requires the `client` feature to be enabled.");
}

/// Egui feature on
#[cfg(feature = "egui")]
pub use egui_winit_vulkano::egui;

use once_cell::sync::Lazy;
pub use rapier2d::prelude::CoefficientCombineRule;
#[cfg(feature = "client")]
use winit::dpi::{PhysicalPosition, PhysicalSize};

/// Cardinal directions
pub mod directions {
    pub const CENTER: [f32; 2] = [0.5; 2];
    pub const N: [f32; 2] = [0.5, 0.0];
    pub const NO: [f32; 2] = [1.0, 0.0];
    pub const O: [f32; 2] = [1.0, 0.5];
    pub const SO: [f32; 2] = [1.0; 2];
    pub const S: [f32; 2] = [0.5, 1.0];
    pub const SW: [f32; 2] = [0.0, 1.0];
    pub const W: [f32; 2] = [0.0, 0.5];
    pub const NW: [f32; 2] = [0.0; 2];
}

/// The engine wide scene holding all objects in layers.
pub static SCENE: Lazy<objects::scenes::Scene> = Lazy::new(objects::scenes::Scene::default);
/// General time methods of the game engine.
pub static TIME: Lazy<Time> = Lazy::new(Time::default);
/// The input system holding the state of every key and the mouse position.
#[cfg(feature = "client")]
pub static INPUT: Lazy<input::Input> = Lazy::new(input::Input::new);
/// General settings for the game engine.
pub static SETTINGS: Lazy<game::Settings> = Lazy::new(game::Settings::new);
