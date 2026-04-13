// crates/flui-core/src/animation/curve.rs

#![allow(missing_docs)] // animation subsystem is pre-1.0; full rustdoc coverage tracked separately

use std::f32::consts::PI;
use std::sync::Arc;

/// Easing curve for animations.
///
/// Standard variants are zero-allocation. `Custom` uses `Arc` for shared ownership.
#[derive(Clone)]
pub enum Curve {
    // Standard
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,

    // Dramatic
    Bounce,
    Elastic,

    // Parametric
    Spring { damping: f32, stiffness: f32 },
    CubicBezier { x1: f32, y1: f32, x2: f32, y2: f32 },

    // Composition
    /// Maps a sub-range of the timeline to 0.0..1.0.
    /// Used for staggered animations: `Interval { begin: 0.0, end: 0.33, .. }`.
    Interval { begin: f32, end: f32, curve: Box<Curve> },
    Reversed(Box<Curve>),

    // Custom
    Custom(Arc<dyn Fn(f32) -> f32 + Send + Sync>),
}

impl Curve {
    /// Transform `t` (0.0..=1.0) through this curve, returning a new value in 0.0..=1.0.
    pub fn transform(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Curve::Linear => t,
            Curve::EaseIn => t * t,
            Curve::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Curve::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    let x = -2.0 * t + 2.0;
                    1.0 - x * x / 2.0
                }
            }
            Curve::EaseInCubic => t * t * t,
            Curve::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            Curve::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Curve::Bounce => {
                let t = 1.0 - t;
                let n1 = 7.5625;
                let d1 = 2.75;
                let out = if t < 1.0 / d1 {
                    n1 * t * t
                } else if t < 2.0 / d1 {
                    let t = t - 1.5 / d1;
                    n1 * t * t + 0.75
                } else if t < 2.5 / d1 {
                    let t = t - 2.25 / d1;
                    n1 * t * t + 0.9375
                } else {
                    let t = t - 2.625 / d1;
                    n1 * t * t + 0.984375
                };
                1.0 - out
            }
            Curve::Elastic => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let p = 0.3;
                    -(2.0_f32.powf(10.0 * (t - 1.0))
                        * ((t - 1.0 - p / 4.0) * 2.0 * PI / p).sin())
                }
            }
            Curve::Spring { damping, stiffness } => {
                // Simplified spring: critically-damped approximation
                let omega = stiffness.sqrt();
                let zeta = damping / (2.0 * omega);
                if zeta < 1.0 {
                    // Underdamped
                    let wd = omega * (1.0 - zeta * zeta).sqrt();
                    1.0 - (-zeta * omega * t).exp()
                        * ((zeta * omega * t / wd).sin() + (wd * t).cos())
                } else {
                    // Critically/overdamped
                    1.0 - (1.0 + omega * t) * (-omega * t).exp()
                }
            }
            Curve::CubicBezier { x1, y1, x2, y2 } => {
                cubic_bezier_transform(t, *x1, *y1, *x2, *y2)
            }
            Curve::Interval { begin, end, curve } => {
                if t <= *begin {
                    0.0
                } else if t >= *end {
                    1.0
                } else {
                    let local_t = (t - begin) / (end - begin);
                    curve.transform(local_t)
                }
            }
            Curve::Reversed(inner) => inner.transform(1.0 - t),
            Curve::Custom(f) => f(t),
        }
    }
}

/// Solve cubic bezier curve using Newton's method.
fn cubic_bezier_transform(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    // Find parameter for x coordinate, then evaluate y
    let mut guess = t;
    for _ in 0..8 {
        let x = cubic_bezier_sample(guess, x1, x2) - t;
        if x.abs() < 1e-6 {
            break;
        }
        let dx = cubic_bezier_derivative(guess, x1, x2);
        if dx.abs() < 1e-6 {
            break;
        }
        guess -= x / dx;
    }
    cubic_bezier_sample(guess.clamp(0.0, 1.0), y1, y2)
}

fn cubic_bezier_sample(t: f32, a: f32, b: f32) -> f32 {
    // B(t) = 3(1-t)^2*t*a + 3(1-t)*t^2*b + t^3
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    3.0 * mt2 * t * a + 3.0 * mt * t2 * b + t3
}

fn cubic_bezier_derivative(t: f32, a: f32, b: f32) -> f32 {
    let mt = 1.0 - t;
    3.0 * mt * mt * a + 6.0 * mt * t * (b - a) + 3.0 * t * t * (1.0 - b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear() {
        assert_eq!(Curve::Linear.transform(0.0), 0.0);
        assert_eq!(Curve::Linear.transform(0.5), 0.5);
        assert_eq!(Curve::Linear.transform(1.0), 1.0);
    }

    #[test]
    fn test_ease_in_boundaries() {
        assert_eq!(Curve::EaseIn.transform(0.0), 0.0);
        assert_eq!(Curve::EaseIn.transform(1.0), 1.0);
        assert!(Curve::EaseIn.transform(0.5) < 0.5); // ease-in is slow at start
    }

    #[test]
    fn test_ease_out_boundaries() {
        assert_eq!(Curve::EaseOut.transform(0.0), 0.0);
        assert_eq!(Curve::EaseOut.transform(1.0), 1.0);
        assert!(Curve::EaseOut.transform(0.5) > 0.5); // ease-out is fast at start
    }

    #[test]
    fn test_ease_in_out_boundaries() {
        assert_eq!(Curve::EaseInOut.transform(0.0), 0.0);
        assert_eq!(Curve::EaseInOut.transform(1.0), 1.0);
        assert!((Curve::EaseInOut.transform(0.5) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_bounce_boundaries() {
        assert!((Curve::Bounce.transform(0.0)).abs() < 0.01);
        assert!((Curve::Bounce.transform(1.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_interval_before_begin() {
        let curve = Curve::Interval {
            begin: 0.3,
            end: 0.7,
            curve: Box::new(Curve::Linear),
        };
        assert_eq!(curve.transform(0.0), 0.0);
        assert_eq!(curve.transform(0.2), 0.0);
    }

    #[test]
    fn test_interval_after_end() {
        let curve = Curve::Interval {
            begin: 0.3,
            end: 0.7,
            curve: Box::new(Curve::Linear),
        };
        assert_eq!(curve.transform(0.8), 1.0);
        assert_eq!(curve.transform(1.0), 1.0);
    }

    #[test]
    fn test_interval_midpoint() {
        let curve = Curve::Interval {
            begin: 0.0,
            end: 0.5,
            curve: Box::new(Curve::Linear),
        };
        assert!((curve.transform(0.25) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_reversed() {
        let curve = Curve::Reversed(Box::new(Curve::EaseIn));
        // Reversed EaseIn = EaseOut behavior
        assert_eq!(curve.transform(0.0), 1.0);
        assert_eq!(curve.transform(1.0), 0.0);
    }

    #[test]
    fn test_custom() {
        let curve = Curve::Custom(Arc::new(|t| t * t * t));
        assert_eq!(curve.transform(0.0), 0.0);
        assert!((curve.transform(0.5) - 0.125).abs() < 0.01);
        assert_eq!(curve.transform(1.0), 1.0);
    }

    #[test]
    fn test_clamp() {
        assert_eq!(Curve::Linear.transform(-0.5), 0.0);
        assert_eq!(Curve::Linear.transform(1.5), 1.0);
    }
}
