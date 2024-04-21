pub mod camera;
pub mod draw;
pub mod objects;
pub mod resources;
pub mod utils;
pub mod window;

use thiserror::Error;

/// The game engine failed to start for the following reasons:
#[derive(Debug, Error)]
pub enum EngineError {
    /// Your device's specifications do not hold up to the minimum requirements of this engine.
    #[error(
        "Your device does not fulfill the required specification to run this application:\n{0}"
    )]
    RequirementError(String),
    /// Engine can only be made once.
    #[error("You can only initialize this game engine one single time.")]
    EngineInitialized,
    /// Failed to initialize drawing backend of the game engine.
    #[error("Failed to initialize drawing backend:\n{0}")]
    #[cfg(feature = "client")]
    DrawingBackendError(anyhow::Error),
    /// The game engine is not ready to load resources.
    #[error("The game engine is not ready to load resources right now. You have to initialize the game engine first.")]
    NotReady,
    #[error("Could not start the game engine for some reason:\n{0}")]
    Other(String),
}

/// Cardinal direction
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Center,
    N,
    No,
    O,
    So,
    S,
    Sw,
    W,
    Nw,
}

#[cfg(feature = "labels")]
impl From<Direction> for (glyph_brush::HorizontalAlign, glyph_brush::VerticalAlign) {
    fn from(value: Direction) -> Self {
        use glyph_brush::{HorizontalAlign, VerticalAlign};
        let horizontal = match value {
            Direction::Center => HorizontalAlign::Center,
            Direction::N => HorizontalAlign::Center,
            Direction::No => HorizontalAlign::Right,
            Direction::O => HorizontalAlign::Right,
            Direction::So => HorizontalAlign::Right,
            Direction::S => HorizontalAlign::Center,
            Direction::Sw => HorizontalAlign::Left,
            Direction::W => HorizontalAlign::Left,
            Direction::Nw => HorizontalAlign::Left,
        };

        let vertical = match value {
            Direction::Center => VerticalAlign::Center,
            Direction::N => VerticalAlign::Top,
            Direction::No => VerticalAlign::Top,
            Direction::O => VerticalAlign::Center,
            Direction::So => VerticalAlign::Bottom,
            Direction::S => VerticalAlign::Bottom,
            Direction::Sw => VerticalAlign::Bottom,
            Direction::W => VerticalAlign::Center,
            Direction::Nw => VerticalAlign::Top,
        };
        (horizontal, vertical)
    }
}
