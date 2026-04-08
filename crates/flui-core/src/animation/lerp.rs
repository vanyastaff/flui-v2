// crates/flui-core/src/animation/lerp.rs

use crate::{Hsla, Pixels, Point, Size};

/// Trait for types that can be linearly interpolated.
pub trait Lerp: Clone {
    /// Interpolate between `self` and `other` by factor `t` (0.0..=1.0).
    fn lerp(&self, other: &Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        self + (other - self) * t
    }
}

impl Lerp for f64 {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        self + (other - self) * t as f64
    }
}

impl Lerp for Pixels {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Pixels(self.0.lerp(&other.0, t))
    }
}

impl Lerp for Hsla {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Hsla {
            h: self.h + (other.h - self.h) * t,
            s: self.s + (other.s - self.s) * t,
            l: self.l + (other.l - self.l) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }
}

impl Lerp for Point<Pixels> {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Point {
            x: self.x.lerp(&other.x, t),
            y: self.y.lerp(&other.y, t),
        }
    }
}

impl Lerp for Size<Pixels> {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Size {
            width: self.width.lerp(&other.width, t),
            height: self.height.lerp(&other.height, t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::px;

    #[test]
    fn test_f32_lerp() {
        assert_eq!(0.0f32.lerp(&10.0, 0.0), 0.0);
        assert_eq!(0.0f32.lerp(&10.0, 0.5), 5.0);
        assert_eq!(0.0f32.lerp(&10.0, 1.0), 10.0);
    }

    #[test]
    fn test_pixels_lerp() {
        let a = px(0.0);
        let b = px(100.0);
        let mid = a.lerp(&b, 0.5);
        assert!((mid.0 - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_hsla_lerp() {
        let a = Hsla { h: 0.0, s: 0.0, l: 0.0, a: 1.0 };
        let b = Hsla { h: 1.0, s: 1.0, l: 1.0, a: 1.0 };
        let mid = a.lerp(&b, 0.5);
        assert!((mid.h - 0.5).abs() < 0.01);
        assert!((mid.s - 0.5).abs() < 0.01);
        assert!((mid.l - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_point_lerp() {
        let a = Point { x: px(0.0), y: px(0.0) };
        let b = Point { x: px(100.0), y: px(200.0) };
        let mid = a.lerp(&b, 0.5);
        assert!((mid.x.0 - 50.0).abs() < 0.01);
        assert!((mid.y.0 - 100.0).abs() < 0.01);
    }
}
