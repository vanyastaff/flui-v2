//! `VelocityTracker`, `Velocity`, `PositionSample`.
//!
//! Bounded least-squares velocity estimator. Direct port of Flutter's
//! `LeastSquaresSolver::solve` weighted-quadratic fit; bounded
//! `VecDeque<PositionSample>` configured via `GestureSettings`.
//!
//! See the design doc § "VelocityTracker".

use super::GestureSettings;
use crate::scheduler::Instant;
use crate::{Pixels, Point};
use std::collections::VecDeque;
use std::time::Duration;

/// One position sample with its timestamp.
///
/// `#[non_exhaustive]` so future fields (e.g. `pointer_id` for
/// multi-pointer LSQ fits) are non-breaking additions.
#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub struct PositionSample {
    /// Position in window-local logical pixels at sampling time.
    pub position: Point<Pixels>,
    /// Wall-clock timestamp of the sample.
    pub timestamp: Instant,
}

impl PositionSample {
    /// Construct a new sample.
    pub fn new(position: Point<Pixels>, timestamp: Instant) -> Self {
        Self {
            position,
            timestamp,
        }
    }
}

/// The result of a [`VelocityTracker::estimate`] call.
///
/// On insufficient samples (< 3 within the window),
/// `Velocity::default()` is returned (zero vector).
/// `VelocityTracker::estimate` guarantees non-NaN output —
/// [`Self::is_zero`] is safe to call on its result.
///
/// `#[non_exhaustive]` so future fields (e.g. `acceleration`) are
/// non-breaking additions.
#[derive(Copy, Clone, Debug, Default)]
#[non_exhaustive]
pub struct Velocity {
    /// Velocity vector in logical pixels per second.
    pub pixels_per_second: Point<f32>,
}

impl Velocity {
    /// Construct a velocity with the given pixels-per-second vector.
    pub fn new(pixels_per_second: Point<f32>) -> Self {
        Self { pixels_per_second }
    }

    /// Returns `true` iff both velocity components are exactly zero.
    pub fn is_zero(self) -> bool {
        self.pixels_per_second.x == 0.0 && self.pixels_per_second.y == 0.0
    }
}

/// Bounded least-squares velocity estimator. Drops samples older than
/// [`GestureSettings::velocity_tracker_window`]; caps the buffer at
/// [`GestureSettings::velocity_tracker_samples`].
pub struct VelocityTracker {
    samples: VecDeque<PositionSample>,
    max_samples: usize,
    max_age: Duration,
}

impl VelocityTracker {
    /// Construct a new tracker bounded by the supplied settings.
    pub fn new(settings: &GestureSettings) -> Self {
        Self {
            samples: VecDeque::with_capacity(settings.velocity_tracker_samples),
            max_samples: settings.velocity_tracker_samples,
            max_age: settings.velocity_tracker_window,
        }
    }

    /// Record a new position sample. Drops samples older than
    /// `max_age` and trims the buffer to `max_samples`.
    pub fn add_position(&mut self, sample: PositionSample) {
        // Drop samples older than max_age relative to the new sample.
        while let Some(front) = self.samples.front() {
            if sample.timestamp.saturating_duration_since(front.timestamp) > self.max_age {
                self.samples.pop_front();
            } else {
                break;
            }
        }
        // Push new sample; trim to max_samples.
        self.samples.push_back(sample);
        while self.samples.len() > self.max_samples {
            self.samples.pop_front();
        }
    }

    /// Estimate velocity using a weighted-quadratic least-squares
    /// fit (port of Flutter's `LeastSquaresSolver::solve`).
    ///
    /// Returns [`Velocity::default`] (zero) when fewer than 3 samples
    /// fall inside the configured window. Always produces non-NaN
    /// output — the result of [`Velocity::is_zero`] is meaningful.
    pub fn estimate(&self) -> Velocity {
        if self.samples.len() < 3 {
            return Velocity::default();
        }
        let last = match self.samples.back() {
            Some(s) => s,
            None => return Velocity::default(),
        };

        // Convert to (t, x, y, weight) tuples relative to `last`.
        let mut t = Vec::with_capacity(self.samples.len());
        let mut x = Vec::with_capacity(self.samples.len());
        let mut y = Vec::with_capacity(self.samples.len());
        let mut w = Vec::with_capacity(self.samples.len());
        for s in self.samples.iter() {
            // age is positive seconds, with `last` having age 0.
            let age = last
                .timestamp
                .saturating_duration_since(s.timestamp)
                .as_secs_f32();
            // Gaussian-like weight: exp(-(age / window)^2).
            let window = self.max_age.as_secs_f32().max(f32::EPSILON);
            let weight = (-((age / window).powi(2))).exp();
            t.push(-age);
            x.push(s.position.x.0);
            y.push(s.position.y.0);
            w.push(weight);
        }

        let vx = solve_quadratic_velocity(&t, &x, &w);
        let vy = solve_quadratic_velocity(&t, &y, &w);
        debug_assert!(!vx.is_nan(), "VelocityTracker produced NaN x velocity");
        debug_assert!(!vy.is_nan(), "VelocityTracker produced NaN y velocity");
        Velocity::new(Point::new(vx, vy))
    }

    /// Clear all samples.
    pub fn reset(&mut self) {
        self.samples.clear();
    }

    /// Number of samples currently buffered.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }
}

/// Weighted least-squares quadratic fit on `(t, value)`; returns the
/// derivative at t = 0 (i.e. the instantaneous velocity at the most
/// recent sample, since the caller passes negated ages).
///
/// On a degenerate fit (zero weight or near-singular matrix) returns
/// 0.0 — never NaN.
fn solve_quadratic_velocity(t: &[f32], v: &[f32], w: &[f32]) -> f32 {
    debug_assert_eq!(t.len(), v.len());
    debug_assert_eq!(t.len(), w.len());
    // Solve: argmin_a sum_i w_i * (a_0 + a_1 * t_i + a_2 * t_i^2 - v_i)^2
    // Closed-form via the normal equations for the 3x3 weighted Vandermonde.
    let mut s00 = 0.0f32;
    let mut s01 = 0.0f32;
    let mut s02 = 0.0f32;
    let mut s11 = 0.0f32;
    let mut s12 = 0.0f32;
    let mut s22 = 0.0f32;
    let mut b0 = 0.0f32;
    let mut b1 = 0.0f32;
    let mut b2 = 0.0f32;
    for i in 0..t.len() {
        let ti = t[i];
        let ti2 = ti * ti;
        let ti3 = ti2 * ti;
        let ti4 = ti2 * ti2;
        let wi = w[i];
        s00 += wi;
        s01 += wi * ti;
        s02 += wi * ti2;
        s11 += wi * ti2; // == s02 by construction
        s12 += wi * ti3;
        s22 += wi * ti4;
        b0 += wi * v[i];
        b1 += wi * ti * v[i];
        b2 += wi * ti2 * v[i];
    }
    // 3x3 normal-eqn matrix:
    //   [ s00  s01  s02 ]   [ a0 ]   [ b0 ]
    //   [ s01  s11  s12 ] * [ a1 ] = [ b1 ]
    //   [ s02  s12  s22 ]   [ a2 ]   [ b2 ]
    let det = s00 * (s11 * s22 - s12 * s12) - s01 * (s01 * s22 - s12 * s02)
        + s02 * (s01 * s12 - s11 * s02);
    if det.abs() < f32::EPSILON {
        return 0.0;
    }
    // Cramer's rule for `a1` (the linear coefficient = velocity at t = 0).
    let det_a1 =
        s00 * (b1 * s22 - s12 * b2) - b0 * (s01 * s22 - s12 * s02) + s02 * (s01 * b2 - b1 * s02);
    let a1 = det_a1 / det;
    if a1.is_nan() || a1.is_infinite() {
        0.0
    } else {
        a1
    }
}
