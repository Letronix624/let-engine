//! [![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/Letronix624/let-engine/rust.yml?style=for-the-badge&logo=github&label=GitHub&color=9376e0)](https://github.com/Letronix624/let-engine)
//! [![Crates.io](https://img.shields.io/crates/d/let-engine?style=for-the-badge&logo=rust&label=Crates.io&color=e893cf)](https://crates.io/crates/let-engine)
//! [![Static Badge](https://img.shields.io/badge/Docs-passing?style=for-the-badge&logo=docsdotrs&color=f3bcc8&link=let-server.net%2Fdocs%2Flet_engine)](https://docs.rs/let_engine/)
//! [![Website](https://img.shields.io/website?up_message=Up&up_color=f6ffa6&down_message=Down&down_color=lightgrey&url=https%3A%2F%2Flet-server.net%2F&style=for-the-badge&logo=apache&color=f6ffa6&link=https%3A%2F%2Flet-server.net%2F)](https://let-server.net/)
//!
//! A Game engine made in Rust.
mod game;

#[cfg(feature = "asset_system")]
pub use asset_system;
pub use game::*;
pub mod prelude;
#[cfg(feature = "audio")]
pub use let_engine_audio;
pub use settings;

pub use glam::{vec2, Vec2};

#[cfg(not(feature = "client"))]
mod check_feature_dependency {
    #[cfg(feature = "egui")]
    compile_error!("`egui` requires the `client` feature to be enabled.");
    #[cfg(feature = "audio")]
    compile_error!("`audio` requires the `client` feature to be enabled.");
}

/// Egui feature on
#[cfg(feature = "egui")]
pub use egui_winit_vulkano::egui;

pub use let_engine_core::{camera, objects, resources, Direction};

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
pub static SETTINGS: Lazy<
    game::settings::Settings<std::sync::Arc<draw::Graphics>, let_engine_audio::Audio>,
> = Lazy::new(game::settings::Settings::new);
/// General settings for the game engine.
#[cfg(all(feature = "client", not(feature = "audio")))]
pub static SETTINGS: Lazy<game::settings::Settings<std::sync::Arc<Graphics>>> =
    Lazy::new(game::settings::Settings::new);
/// General settings for the game engine.
#[cfg(not(feature = "client"))]
pub static SETTINGS: Lazy<game::settings::Settings> = Lazy::new(game::settings::Settings::new);

/// A macro that makes it easy to create circles.
///
/// Returns [Data](let_engine_core::resources::data::Data) with vertices and indices.
///
/// ### $corners
/// Using this with a `u32` makes a circle fan with as many corners as given.
///
/// ### $percent
/// Using this with a `f64` makes a circle fan that looks like a pie with the given percentage missing.
///
/// ## usage:
/// ```rust
/// use let_engine::prelude::*;
///
/// let hexagon: Data = make_circle!(6); // Makes a hexagon.
///
/// // Makes a pie circle fan with 20 edges with the top right part missing a quarter piece.
/// let pie: Data = make_circle!(20, 0.75);
/// ```
#[macro_export]
macro_rules! make_circle {
    // Full circle fan
    ($corners:expr) => {{
        use let_engine::prelude::{vec2, Data, Vertex};

        let corners = $corners;
        if corners == 0 {
            panic!("Number of corners must be greater than zero")
        }

        let mut vertices: Vec<Vertex> = vec![];
        let mut indices: Vec<u32> = vec![];
        use core::f64::consts::TAU;

        // first point in the middle
        vertices.push(Vertex {
            position: vec2(0.0, 0.0),
            tex_position: vec2(0.0, 0.0),
        });

        // Generate vertices
        for i in 0..corners {
            vertices.push(vert(
                (TAU * ((i as f64) / corners as f64)).cos() as f32,
                (TAU * ((i as f64) / corners as f64)).sin() as f32,
            ));
        }

        // Generate indices
        for i in 0..corners - 1 {
            // -1 so the last index doesn't go above the total amounts of indices.
            indices.extend([0, i + 1, i + 2]);
        }
        indices.extend([0, corners, 1]);

        Data::Dynamic { vertices, indices }
    }};
    // Pie circle
    ($corners:expr, $percent:expr) => {{
        use let_engine::prelude::{vec2, Data, Vertex};

        let corners = $corners;
        if corners == 0 {
            panic!("Number of corners must be greater than zero")
        }

        let percent: f64 = ($percent as f64).clamp(0.0, 1.0);
        let mut vertices: Vec<Vertex> = vec![];
        let mut indices: Vec<u32> = vec![];
        use core::f64::consts::TAU;

        let count = TAU * percent;

        vertices.push(vert(0.0, 0.0));

        // Generate vertices
        for i in 0..corners + 1 {
            vertices.push(vert(
                (count * ((i as f64) / corners as f64)).cos() as f32,
                (count * ((i as f64) / corners as f64)).sin() as f32,
            ));
        }

        // Generate indices
        for i in 0..corners {
            indices.extend([0, i + 1, i + 2]);
        }

        Data::Dynamic { vertices, indices }
    }};
}
