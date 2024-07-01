//! Everything the game engine has.
//!
//! # usage:
//! ```rust
//! use let_engine::prelude::*;
//! ```
//! - imports everything this engine has to offer.

// Resources only exists if client is enabled.

#[cfg(feature = "client")]
pub use let_engine_core::resources::*;

pub use let_engine_core::{camera::*, objects::*};

pub use crate::*;
#[cfg(feature = "client")]
pub use data::*;

// Client structs
#[cfg(feature = "client")]
mod client {
    pub use super::materials::*;
    pub use super::textures::*;
    pub use super::window::*;
    pub use crate::events::*;
    pub use let_engine_core::draw::PresentMode;
}
#[cfg(feature = "client")]
pub use client::*;

// Physics structs
#[cfg(feature = "physics")]
pub use joints::*;
#[cfg(feature = "physics")]
pub use physics::*;

// Audio structs
#[cfg(feature = "audio")]
pub use let_engine_audio::*;

// Asset system
#[cfg(feature = "asset_system")]
pub use asset_system::*;

// Networking
#[cfg(feature = "networking")]
pub use networking::*;

// Other structs
pub use crate::settings::{EngineSettings, EngineSettingsBuilder, EngineSettingsBuilderError};
pub use glam;
pub use glam::{vec2, Vec2};
pub use scenes::*;
