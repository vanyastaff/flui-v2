//! ADR-019: scroll state observed by [`super::ScrollPhysics`].

use crate::{Pixels, Point, px};

/// Current state of a scrollable view, passed to
/// [`super::ScrollPhysics::apply_delta`] and
/// [`super::ScrollPhysics::fling`].
///
/// A `ScrollState` carries enough information for physics
/// implementations to decide rubber-band resistance at edges, axis
/// lock direction, overscroll allowance, and fling target. It does
/// NOT carry callbacks or mutable references — physics impls are
/// pure (input → output) so tests can drive them deterministically
/// with `MockPhysics`-style stubs.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ScrollState {
    /// Current scroll offset of the view, in pixels.
    pub offset: Point<Pixels>,
    /// Minimum scroll offset (typically `(0, 0)` for top-aligned
    /// content; non-zero for content with a header bias).
    pub min_offset: Point<Pixels>,
    /// Maximum scroll offset, derived from `content_size - viewport_size`
    /// clamped at zero.
    pub max_offset: Point<Pixels>,
    /// Most-recent pointer velocity, in pixels per second. Used by
    /// `fling` to compute simulation initial velocity.
    pub velocity: Point<Pixels>,
    /// Active axis lock direction when the pointer has crossed the
    /// slop threshold and physics committed to a primary axis. `None`
    /// while the gesture is still pre-slop or the physics permits
    /// free 2-D motion.
    pub axis_lock: Option<Axis>,
}

/// Axis identity, used by [`ScrollState::axis_lock`] and physics
/// `apply_delta` to commit a gesture to a single direction once the
/// dominant axis is determined.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Axis {
    /// Horizontal motion only.
    Horizontal,
    /// Vertical motion only.
    Vertical,
}

impl ScrollState {
    /// Returns `true` when the current `offset` is within the
    /// `min_offset..=max_offset` rectangle (no overscroll).
    pub fn in_bounds(&self) -> bool {
        self.offset.x >= self.min_offset.x
            && self.offset.x <= self.max_offset.x
            && self.offset.y >= self.min_offset.y
            && self.offset.y <= self.max_offset.y
    }

    /// Signed overscroll on each axis: positive when the offset is
    /// past the max, negative when below the min, zero when in
    /// bounds. Used by physics impls to compute rubber-band
    /// resistance.
    pub fn overscroll(&self) -> Point<Pixels> {
        let dx = if self.offset.x > self.max_offset.x {
            self.offset.x - self.max_offset.x
        } else if self.offset.x < self.min_offset.x {
            self.offset.x - self.min_offset.x
        } else {
            px(0.0)
        };
        let dy = if self.offset.y > self.max_offset.y {
            self.offset.y - self.max_offset.y
        } else if self.offset.y < self.min_offset.y {
            self.offset.y - self.min_offset.y
        } else {
            px(0.0)
        };
        Point::new(dx, dy)
    }
}
