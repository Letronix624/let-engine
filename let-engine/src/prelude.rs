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

// Client structs
#[cfg(feature = "client")]
pub use crate::events::*;
#[cfg(feature = "client")]
pub use crate::*;
#[cfg(feature = "client")]
pub use data::*;
#[cfg(feature = "client")]
pub use let_engine_core::draw::PresentMode;
#[cfg(feature = "client")]
pub use materials::*;
#[cfg(feature = "client")]
pub use textures::*;
#[cfg(feature = "client")]
pub use window::*;

// Physics structs
#[cfg(feature = "physics")]
pub use joints::*;
#[cfg(feature = "physics")]
pub use physics::*;

// Label structs
#[cfg(feature = "labels")]
pub use labels::*;

// Audio structs
#[cfg(feature = "audio")]
pub use crate::sounds::*;

// Asset system
#[cfg(feature = "asset_system")]
pub use asset_system::*;

// Other structs
pub use crate::settings::{EngineSettings, EngineSettingsBuilder, EngineSettingsBuilderError};
pub use glam;
pub use glam::{vec2, Vec2};
pub use scenes::*;
