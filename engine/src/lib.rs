pub mod error;
mod game;
pub mod texture;

pub use game::{
    materials, Appearance, Camera, CameraObject, CameraScaling, CameraSettings, Data, Font, Game,
    GameBuilder, GameObject, Label, LabelCreateInfo, Layer, Resources, Scene, Time, Transform,
    Vertex,
};

pub use game::objects::data::{CENTER, N, NO, NW, O, S, SO, SW, W};
//RE EXPORTS

pub use crossbeam::atomic::AtomicCell;
pub use engine_macros::*;
pub use glam::{vec2, Vec2};
