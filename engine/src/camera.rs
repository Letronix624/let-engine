//! Camera and vision related settings.

use std::f32::consts::FRAC_1_SQRT_2;

use glam::{vec2, Vec2};

/// The 4 Camera scaling modes determine how far you can see when the window changes scale.
/// For 2D games those are a problem because there will always be someone with a monitor or window with a weird aspect ratio that can see much more than others when it's not on stretch mode.
///
/// The view size can be bigger or smaller depending on the zoom value. When -1 or 1 is mentioned we are talking about the default zoom of 1.
///
/// Those are the options in this game engine:

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CameraScaling {
    /// Goes from -1 to 1 in both x and y. So the camera view stretches when the window is not square.
    Stretch = 1,
    /// Tries to have the same width\*height surface area all the time. When Making the window really thin you can see the same surface area, so you could see really far.
    Linear = 2,
    /// It's similar to Linear but you can't look that far the tighter the window is.
    Circle = 3,
    /// The biggest side is always -1 to 1. Simple and more unfair the tighter your window is.
    Limited = 4,
    /// The bigger the window is the more you can see. Good for HUDs, text and textures.
    ///
    /// A window size of 1000 pixels gives a view from -1 to 1.
    Expand = 5,
    /// The horizontal view area is kept at -1 to 1, but y can expand or shrink giving more or less vertical view.
    KeepHorizontal,
    /// The vertical view area is kept at -1 to 1, but x can expand or shrink giving more or less horizontal view.
    KeepVertical,
}

impl Default for CameraScaling {
    fn default() -> Self {
        Self::Stretch
    }
}

impl CameraScaling {
    /// Scales the given dimensions using the given scaling algorithm.
    pub fn scale(&self, dimensions: Vec2) -> Vec2 {
        match self {
            // Camera view x1 and y1 max and min.
            CameraScaling::Stretch => vec2(1.0, 1.0),
            CameraScaling::Linear => vec2(
                0.5 / (dimensions.y / (dimensions.x + dimensions.y)),
                0.5 / (dimensions.x / (dimensions.x + dimensions.y)),
            ),
            CameraScaling::Circle => vec2(
                1.0 / (dimensions.y.atan2(dimensions.x).sin() / FRAC_1_SQRT_2),
                1.0 / (dimensions.y.atan2(dimensions.x).cos() / FRAC_1_SQRT_2),
            ),
            CameraScaling::Limited => vec2(
                1.0 / (dimensions.y / dimensions.x.clamp(0.0, dimensions.y)),
                1.0 / (dimensions.x / dimensions.y.clamp(0.0, dimensions.x)),
            ),
            CameraScaling::Expand => vec2(dimensions.x * 0.001, dimensions.y * 0.001),
            CameraScaling::KeepHorizontal => vec2(1.0, 1.0 / (dimensions.x / dimensions.y)),
            CameraScaling::KeepVertical => vec2(1.0 / (dimensions.y / dimensions.x), 1.0),
        }
    }
}

/// Settings that determine your camera vision.
#[derive(Clone, Copy)]
pub struct CameraSettings {
    /// The camera zoom level. Default is `1.0`.
    pub zoom: f32,
    /// The scaling mode. Default is `Stretch`.
    pub mode: CameraScaling,
}
impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            mode: CameraScaling::Stretch,
        }
    }
}
impl CameraSettings {
    /// Returns the zoom.
    #[inline]
    pub fn zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom;
        self
    }
    /// Returns the camera scaling mode.
    #[inline]
    pub fn mode(mut self, mode: CameraScaling) -> Self {
        self.mode = mode;
        self
    }
}
