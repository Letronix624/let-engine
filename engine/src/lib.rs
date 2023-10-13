pub mod error;
mod game;
pub mod prelude;
pub(crate) mod utils;

pub use game::{
    camera, data, events, materials, objects, physics, resources, tvert, vert, window, Game, Input,
    Layer, NObject, Scene, Time, Transform, Vertex, WeakObject,
};

pub use color_art::{color, Color};
pub use engine_macros::*;
pub use glam::{vec2, Vec2};
pub use once_cell::sync::Lazy;
pub use parking_lot::Mutex;
pub use rapier2d;

pub type _Resources = std::sync::Arc<Mutex<resources::Resources>>;

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
