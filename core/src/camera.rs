//! Camera and vision related settings.

use std::f32::consts::FRAC_1_SQRT_2;

use glam::{vec2, Mat4, UVec2, Vec2};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::objects::Transform;

/// The Camera scaling modes determine how far you can see when the window changes scale.
/// For 2D games those are a problem because there will always be someone with a monitor or window with a weird aspect ratio that can see much more than others when it is not on stretch mode.
///
/// The view size can be bigger or smaller depending on the zoom value. When -1 or 1 is mentioned we are talking about the default zoom of 1.
///
/// Those are the options in this game engine:
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CameraScaling {
    /// Goes from -1 to 1 in both x and y. So the camera view stretches when the window is not square.
    Stretch = 1,
    /// Tries to have the same width\*height surface area all the time. When Making the window really thin you can see the same surface area, so you could see really far.
    Linear = 2,
    /// It is similar to Linear but you can not look that far the tighter the window is.
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

/// The camera instance that when used in a LayerView determines your view.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Copy)]
pub struct Camera {
    /// The position, rotation and zoom of the camera. Size in the transform works as zoom. Default zoom is 'vec2(1.0, 1.0)'.
    pub transform: Transform,
    /// The scaling mode. Default is `Stretch`.
    pub scaling: CameraScaling,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            transform: Transform {
                position: Vec2::ZERO,
                size: Vec2::ONE,
                rotation: 0.0,
            },
            scaling: CameraScaling::Stretch,
        }
    }
}

impl Camera {
    /// Sets the transform and returns self.
    #[inline]
    pub fn transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    /// Sets the camera scaling mode and returns self.
    #[inline]
    pub fn scaling(mut self, scaling: CameraScaling) -> Self {
        self.scaling = scaling;
        self
    }

    /// Creates a view matrix for the camera.
    pub fn make_view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(
            self.transform.position.extend(1.0),
            self.transform.position.extend(0.0),
            Vec2::from_angle(self.transform.rotation).extend(0.0),
        )
    }

    /// Creates a projection matrix for the camera.
    pub fn make_projection_matrix(&self, dimensions: UVec2) -> Mat4 {
        let zoom = 1.0 / self.transform.size;
        let position = self.transform.position;
        let dimensions = self.scaling.scale(dimensions.as_vec2());

        Mat4::orthographic_rh(
            position.x - zoom.x * dimensions.x,
            position.x + zoom.x * dimensions.x,
            position.y - zoom.y * dimensions.y,
            position.y + zoom.y * dimensions.y,
            -1.0,
            1.0,
        )
    }
}
