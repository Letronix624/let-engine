//! Everything the game engine has.
//!
//! # usage:
//! ```rust
//! use let_engine::prelude::*;
//! ```
//! - imports everything this engine has to offer.

#[cfg(any(feature = "client", feature = "audio"))]
pub use crate::resources::*;
pub use crate::{camera::*, directions::*, objects::*, *};
#[cfg(feature = "client")]
pub use appearance::*;
#[cfg(feature = "client")]
pub use color::*;
#[cfg(feature = "client")]
pub use data::*;
#[cfg(feature = "client")]
pub use events::*;
pub use glam;
pub use glam::{vec2, Vec2};
#[cfg(feature = "physics")]
pub use joints::*;
#[cfg(feature = "labels")]
pub use labels::*;
#[cfg(feature = "client")]
pub use materials::*;
#[cfg(feature = "physics")]
pub use physics::*;
pub use scenes::*;
#[cfg(feature = "audio")]
pub use sounds::*;
#[cfg(feature = "client")]
pub use textures::*;
#[cfg(feature = "client")]
pub use window::*;
