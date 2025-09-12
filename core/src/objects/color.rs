/// A struct that represents a color to use on objects, the clear color or labels.
use glam::{Vec3, Vec4, vec3, vec4};
use let_engine_macros::Vertex;

/// Representation of a color in form of 4 `f32`'s for R, G, B and A.
#[derive(Default, Clone, Copy, Debug, PartialEq, bytemuck::AnyBitPattern, Vertex)]
#[repr(C)]
pub struct Color {
    #[format(Rgba32Float)]
    rgba: [f32; 4],
}

/// Declaration
impl Color {
    /// Full white color
    pub const WHITE: Self = Self::from_rgb(1.0, 1.0, 1.0);

    /// Black color with full alpha channel
    pub const BLACK: Self = Self::from_r(0.0);

    /// Full Black transparent color
    pub const TRANSPARENT: Self = Self::from_rgba(0.0, 0.0, 0.0, 0.0);

    /// Opaque red color
    pub const RED: Self = Self::from_r(1.0);

    /// Opaque green color
    pub const GREEN: Self = Self::from_g(1.0);

    /// Opaque blue color
    pub const BLUE: Self = Self::from_b(1.0);

    /// Makes a color from red, green, blue and alpha.
    #[inline]
    pub const fn from_rgba(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            rgba: [red, green, blue, alpha],
        }
    }
    /// Makes a color from red, green and blue.
    #[inline]
    pub const fn from_rgb(red: f32, green: f32, blue: f32) -> Self {
        Self {
            rgba: [red, green, blue, 1.0],
        }
    }
    /// Makes a color from red and green.
    #[inline]
    pub const fn from_rg(red: f32, green: f32) -> Self {
        Self {
            rgba: [red, green, 0.0, 1.0],
        }
    }
    /// Makes a color from red and blue.
    #[inline]
    pub const fn from_rb(red: f32, blue: f32) -> Self {
        Self {
            rgba: [red, 0.0, blue, 1.0],
        }
    }
    /// Makes a color from red.
    #[inline]
    pub const fn from_r(red: f32) -> Self {
        Self {
            rgba: [red, 0.0, 0.0, 1.0],
        }
    }
    /// Makes a color from green and blue.
    #[inline]
    pub const fn from_gb(green: f32, blue: f32) -> Self {
        Self {
            rgba: [0.0, green, blue, 1.0],
        }
    }
    /// Makes a color from green.
    #[inline]
    pub const fn from_g(green: f32) -> Self {
        Self {
            rgba: [0.0, green, 0.0, 1.0],
        }
    }
    /// Makes a color from blue.
    #[inline]
    pub const fn from_b(blue: f32) -> Self {
        Self {
            rgba: [0.0, 0.0, blue, 1.0],
        }
    }
}

/// Usage
impl Color {
    /// Returns the red green blue and alpha of this color.
    #[inline]
    pub fn rgba(&self) -> [f32; 4] {
        self.rgba
    }

    /// Returns the red green and blue of this color.
    #[inline]
    pub fn rgb(&self) -> [f32; 3] {
        [self.rgba[0], self.rgba[1], self.rgba[2]]
    }

    /// Returns the red of this color.
    #[inline]
    pub fn r(&self) -> f32 {
        self.rgba[0]
    }

    /// Returns the green of this color.
    #[inline]
    pub fn g(&self) -> f32 {
        self.rgba[1]
    }

    /// Returns the blue of this color.
    #[inline]
    pub fn b(&self) -> f32 {
        self.rgba[2]
    }

    /// Returns the alpha or transparency of this color.
    #[inline]
    pub fn alpha(&self) -> f32 {
        self.rgba[3]
    }

    /// Sets the red channel of this color.
    #[inline]
    pub fn set_r(&mut self, red: f32) {
        self.rgba[0] = red;
    }

    /// Sets the green channel of this color.
    #[inline]
    pub fn set_g(&mut self, green: f32) {
        self.rgba[1] = green;
    }

    /// Sets the blue channel of this color.
    #[inline]
    pub fn set_b(&mut self, blue: f32) {
        self.rgba[2] = blue;
    }

    /// Sets the alpha channel of this color.
    #[inline]
    pub fn set_a(&mut self, alpha: f32) {
        self.rgba[3] = alpha;
    }

    /// Interpolates to the next color.
    #[inline]
    pub fn lerp(self, rhs: Self, s: f32) -> Self {
        self + ((rhs - self) * s)
    }
}

impl From<[f32; 4]> for Color {
    #[inline]
    fn from(value: [f32; 4]) -> Self {
        Color::from_rgba(value[0], value[1], value[2], value[3])
    }
}
impl From<[f32; 3]> for Color {
    #[inline]
    fn from(value: [f32; 3]) -> Self {
        Color::from_rgb(value[0], value[1], value[2])
    }
}
impl From<f32> for Color {
    #[inline]
    fn from(value: f32) -> Self {
        Color::from_r(value)
    }
}
impl From<Color> for f32 {
    #[inline]
    fn from(value: Color) -> f32 {
        value.r()
    }
}
impl From<Color> for [f32; 3] {
    #[inline]
    fn from(value: Color) -> [f32; 3] {
        value.rgb()
    }
}
impl From<Color> for [f32; 4] {
    #[inline]
    fn from(value: Color) -> [f32; 4] {
        value.rgba()
    }
}
impl From<Color> for Vec3 {
    #[inline]
    fn from(value: Color) -> Vec3 {
        vec3(value.r(), value.g(), value.b())
    }
}
impl From<Color> for Vec4 {
    #[inline]
    fn from(value: Color) -> Vec4 {
        vec4(value.r(), value.g(), value.b(), value.alpha())
    }
}
impl std::ops::Add<Color> for Color {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            rgba: [
                self.rgba[0] + rhs.rgba[0],
                self.rgba[1] + rhs.rgba[1],
                self.rgba[2] + rhs.rgba[2],
                self.rgba[3] + rhs.rgba[3],
            ],
        }
    }
}

impl std::ops::Sub<Color> for Color {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            rgba: [
                self.rgba[0] - rhs.rgba[0],
                self.rgba[1] - rhs.rgba[1],
                self.rgba[2] - rhs.rgba[2],
                self.rgba[3] - rhs.rgba[3],
            ],
        }
    }
}

impl std::ops::Mul<Color> for Color {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            rgba: [
                self.rgba[0] * rhs.rgba[0],
                self.rgba[1] * rhs.rgba[1],
                self.rgba[2] * rhs.rgba[2],
                self.rgba[3] * rhs.rgba[3],
            ],
        }
    }
}

impl std::ops::Div<Color> for Color {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        Self {
            rgba: [
                self.rgba[0] / rhs.rgba[0],
                self.rgba[1] / rhs.rgba[1],
                self.rgba[2] / rhs.rgba[2],
                self.rgba[3] / rhs.rgba[3],
            ],
        }
    }
}

impl std::ops::Add<f32> for Color {
    type Output = Self;
    #[inline]
    fn add(self, rhs: f32) -> Self::Output {
        Self {
            rgba: self.rgba.map(|x| x + rhs),
        }
    }
}

impl std::ops::Sub<f32> for Color {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: f32) -> Self::Output {
        Self {
            rgba: self.rgba.map(|x| x - rhs),
        }
    }
}

impl std::ops::Mul<f32> for Color {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        Self {
            rgba: self.rgba.map(|x| x * rhs),
        }
    }
}

impl std::ops::Div<f32> for Color {
    type Output = Self;
    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        Self {
            rgba: self.rgba.map(|x| x / rhs),
        }
    }
}

impl std::ops::AddAssign<Color> for Color {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.rgba[0].add_assign(rhs.rgba[0]);
        self.rgba[1].add_assign(rhs.rgba[1]);
        self.rgba[2].add_assign(rhs.rgba[2]);
        self.rgba[3].add_assign(rhs.rgba[3]);
    }
}

impl std::ops::SubAssign<Color> for Color {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.rgba[0].sub_assign(rhs.rgba[0]);
        self.rgba[1].sub_assign(rhs.rgba[1]);
        self.rgba[2].sub_assign(rhs.rgba[2]);
        self.rgba[3].sub_assign(rhs.rgba[3]);
    }
}

impl std::ops::MulAssign<Color> for Color {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        self.rgba[0].mul_assign(rhs.rgba[0]);
        self.rgba[1].mul_assign(rhs.rgba[1]);
        self.rgba[2].mul_assign(rhs.rgba[2]);
        self.rgba[3].mul_assign(rhs.rgba[3]);
    }
}

impl std::ops::DivAssign<Color> for Color {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        self.rgba[0].div_assign(rhs.rgba[0]);
        self.rgba[1].div_assign(rhs.rgba[1]);
        self.rgba[2].div_assign(rhs.rgba[2]);
        self.rgba[3].div_assign(rhs.rgba[3]);
    }
}

impl std::ops::AddAssign<f32> for Color {
    #[inline]
    fn add_assign(&mut self, rhs: f32) {
        for mut x in self.rgba {
            x.add_assign(rhs)
        }
    }
}

impl std::ops::SubAssign<f32> for Color {
    #[inline]
    fn sub_assign(&mut self, rhs: f32) {
        for mut x in self.rgba {
            x.sub_assign(rhs)
        }
    }
}

impl std::ops::MulAssign<f32> for Color {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        for mut x in self.rgba {
            x.mul_assign(rhs)
        }
    }
}

impl std::ops::DivAssign<f32> for Color {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        for mut x in self.rgba {
            x.div_assign(rhs)
        }
    }
}

impl std::ops::Deref for Color {
    type Target = [f32; 4];

    fn deref(&self) -> &Self::Target {
        &self.rgba
    }
}

impl std::ops::DerefMut for Color {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.rgba
    }
}
