//! Everything the game engine has.
//!
//! # usage:
//! ```rust
//! use let_engine::prelude::*;
//! ```
//! - imports everything this engine has to offer.

// Resources only exists if client is enabled.

pub use buffer::*;
pub use data::*;
pub use let_engine_core::resources::*;
pub use material::*;
pub use model::*;
pub use texture::*;
pub use tick_system::*;

pub use let_engine_core::{camera::*, objects::*};

pub use crate::*;
pub use let_engine_core::{circle, model};

// Client structs
#[cfg(feature = "client")]
mod client {
    pub use super::window::*;
    pub use crate::events::*;
}

#[cfg(feature = "client")]
pub use client::*;

#[cfg(feature = "client")]
pub use let_engine_core::backend::audio::*;

// Physics structs
#[cfg(feature = "physics")]
pub use joints::*;
#[cfg(feature = "physics")]
pub use physics::*;

pub use backend::DefaultBackends;

// Asset system
#[cfg(feature = "asset_system")]
pub use asset_system::*;

// // Networking
// #[cfg(feature = "default_networking_backend")]
// pub use backend::networking::*;

// // Graphics
#[cfg(feature = "default_graphics_backend")]
pub use backend::graphics;

// Other structs
pub use crate::settings::*;
pub use glam;
pub use glam::{uvec2, vec2, UVec2, Vec2};
pub use scenes::*;
