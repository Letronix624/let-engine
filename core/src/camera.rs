//! Camera and vision related settings.

use glam::{Vec2, vec2};

use crate::objects::Transform;

/// The Camera scaling modes determine how far you can see when the aspect ratio changes.
///
/// Each mode calculates how far the camera can see in world space units depending on the provided dimensions multiplied by the zoom value.
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub enum CameraScaling {
    /// Keeps the view size fixed from -1  to 1 in both axes regardless of provided dimensions.
    #[default]
    Stretch,

    /// Preserves the total visible surface area regardless of shape.
    ///
    /// This means the narrower the dimensions are, the farther you can see in that direction.
    ///
    /// The product of width and height equals 1 here.
    Linear,

    /// Constrains the visible area to fit within a circle centered at the camera origin.
    ///
    /// Just like `Box`, but instead of a rectangular box bound, the extend fits within a circle.
    ///
    /// This prevents viewing further in extreme aspect ratios, a problem with `Linear`.
    Circle,

    /// The largest side of the view is always exactly -1 to 1.
    ///
    /// The smaller the dimensions are, the more limited the view in the smaller dimension is.
    Box,

    /// Makes the view grow with the render size. This is good for UI and pixel perfect rendering.
    ///
    /// Here one pixel equates to one unit of world space. Here you should set your zoom of the camera with this mode set.
    Expand,

    /// The horizontal view area is locked at -1 to 1 and allows the vertical axis to expand or shrink.
    ///
    /// This one is useful for platformers where consistent horizontal field of view is important.
    KeepHorizontal,

    /// The vertical view area is locked at -1 to 1 and allows the horizontal axis to expand or shrink.
    ///
    /// This one is great for top down or vertical scrolling games.
    KeepVertical,

    /// User defined scaling option.
    Custom(fn(Vec2) -> Vec2),
}

impl CameraScaling {
    /// Scales the given dimensions using the given scaling algorithm.
    ///
    /// Takes a dimension and outputs the maximum field of view in both axis using those dimensions.
    pub fn scale(&self, dimensions: Vec2) -> Vec2 {
        match self {
            // Camera view x1 and y1 max and min.
            CameraScaling::Stretch => Vec2::ONE,
            CameraScaling::Linear => vec2(
                0.5 / (dimensions.y / (dimensions.x + dimensions.y)),
                0.5 / (dimensions.x / (dimensions.x + dimensions.y)),
            ),
            CameraScaling::Circle => {
                let radius = 1.0 / dimensions.length();
                dimensions * radius
            }
            CameraScaling::Box => vec2(
                1.0 / (dimensions.y / dimensions.x.clamp(0.0, dimensions.y)),
                1.0 / (dimensions.x / dimensions.y.clamp(0.0, dimensions.x)),
            ),
            CameraScaling::Expand => dimensions,
            CameraScaling::KeepHorizontal => vec2(1.0, 1.0 / (dimensions.x / dimensions.y)),
            CameraScaling::KeepVertical => vec2(1.0 / (dimensions.y / dimensions.x), 1.0),
            CameraScaling::Custom(f) => f(dimensions),
        }
    }
}

/// When using the transform as a camera, size determines the zoom in both axis.
pub type Camera = Transform;
