use crate::objects::GameObject;

/// The 4 Camera scaling modes determine how far you can see when the window changes scale.
/// For 2D games those are a problem because there will always be someone with a monitor or window with a weird aspect ratio that can see much more than others when it's not on stretch mode.
/// Those are the options in this game engine:

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CameraScaling {
    /// 1: Stretch - goes from -1 to 1 in both x and y. So the camera view stretches when the window is not square.
    Stretch = 1,
    /// 2: Linear - Tries to be fair with window scaling and tries to have the same width\*height surface all the time. But when Making the window really thin or something like that you can still see the same height\*width so you could see really far.
    Linear = 2,
    /// 3: Circle - Imagine a rope tied to itself to make a circle and imagine trying to fit 4 corners of a rectangle as far away from each other. It's similar to Linear but you can't look that far the tighter the window is.
    Circle = 3,
    /// 4: Limited - The biggest side is always -1 to 1. Simple and more unfair the tighter your window is.
    Limited = 4,
    /// 5: Expand - The bigger the window is the more you can see. Good for HUDs, fonts and textures.
    Expand = 5,
}

impl Default for CameraScaling {
    fn default() -> Self {
        Self::Stretch
    }
}

pub trait Camera: GameObject {
    fn settings(&self) -> CameraSettings;
}

#[derive(Clone, Copy)]
pub struct CameraSettings {
    pub zoom: f32,
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
    pub fn zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom;
        self
    }
    pub fn mode(mut self, mode: CameraScaling) -> Self {
        self.mode = mode;
        self
    }
}
