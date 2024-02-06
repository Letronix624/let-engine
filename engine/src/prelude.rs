//! Everything the game engine has.
//!
//! # usage:
//! ```rust
//! use let_engine::prelude::*;
//! ```
//! - imports everything this engine has to offer.

// Resources only exists if client is enabled.
#[cfg(feature = "client")]
pub use crate::resources::*;

pub use crate::{camera::*, objects::*, *};

// Client structs
#[cfg(feature = "client")]
pub use data::*;
#[cfg(feature = "client")]
pub use events::*;
#[cfg(feature = "client")]
pub use materials::*;
#[cfg(feature = "client")]
pub use settings::PresentMode;
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
pub use sounds::*;

// Other structs
pub use glam;
pub use glam::{vec2, Vec2};
pub use scenes::*;
pub use settings::{EngineSettings, EngineSettingsBuilder, EngineSettingsBuilderError};
