//! Camera and vision related settings.

/// The 4 Camera scaling modes determine how far you can see when the window changes scale.
/// For 2D games those are a problem because there will always be someone with a monitor or window with a weird aspect ratio that can see much more than others when it's not on stretch mode.
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
    Expand = 5,
}

impl Default for CameraScaling {
    fn default() -> Self {
        Self::Stretch
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