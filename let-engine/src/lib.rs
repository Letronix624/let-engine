//! [![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/Letronix624/let-engine/rust.yml?style=for-the-badge&logo=github&label=GitHub&color=9376e0)](https://github.com/Letronix624/let-engine) [![Crates.io](https://img.shields.io/crates/d/let-engine?style=for-the-badge&logo=rust&label=Crates.io&color=e893cf)](https://crates.io/crates/let-engine) [![Static Badge](https://img.shields.io/badge/Docs-passing?style=for-the-badge&logo=docsdotrs&color=f3bcc8&link=docs.rs%2Flet_engine)](https://docs.rs/let-engine) [![Website](https://img.shields.io/website?up_message=Up&up_color=f6ffa6&down_message=Down&down_color=lightgrey&url=https%3A%2F%2Flet.software%2F&style=for-the-badge&logo=apache&color=f6ffa6&link=https%3A%2F%2Flet.software%2F)](https://let.software/) [![Static Badge](https://img.shields.io/badge/radicle-up-brightgreen?style=for-the-badge&logo=git&label=Radicle&link=https%3A%2F%2Fapp.radicle.xyz%2Fnodes%2Fseed.let.software%2Frad%3Az35VMD8yfGYcrvb7k2eyxiUL4VUko)](https://app.radicle.xyz/nodes/seed.let.software/rad:z35VMD8yfGYcrvb7k2eyxiUL4VUko)
//!
//! A Game engine made in Rust.

pub mod backend;
mod engine;
#[cfg(feature = "client")]
pub mod events;
#[cfg(feature = "client")]
pub mod input;
pub mod settings;
pub mod tick_system;

pub use engine::*;
#[cfg(feature = "asset_system")]
pub use let_engine_asset_system as asset_system;
pub mod prelude;

pub use glam;

#[cfg(feature = "client")]
pub use let_engine_core::resources;
pub use let_engine_core::{Direction, backend as core_backend, camera, objects};

#[cfg(feature = "client")]
pub mod window;

/// Cleans all caches for unused data. This decreases memory usage and may not
/// hurt to be called between levels from time to time.
pub fn clean_caches() {
    // #[cfg(feature = "default_networking_backend")] TODO
    // crate::backend::networking::LAST_ORDS.lock().clear();

    #[cfg(feature = "asset_system")]
    asset_system::clear_cache();
}
