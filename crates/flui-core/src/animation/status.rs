// crates/flui-core/src/animation/status.rs

#![allow(missing_docs)] // animation subsystem is pre-1.0; full rustdoc coverage is tracked under S21 phase 7

/// Status of an animation.
///
/// Mirrors Flutter's
/// [`AnimationStatus`](https://api.flutter.dev/flutter/animation/AnimationStatus.html).
///
/// Marked `#[non_exhaustive]` because the public API may grow new states in
/// later S21 phases (e.g. for `AnimationBehavior::Preserve` / muting). Match
/// arms in external code must include a `_ =>` arm — this is captured under
/// roadmap item A8 (`#[non_exhaustive]` audit).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AnimationStatus {
    /// At lower bound, idle.
    #[default]
    Dismissed,
    /// Animating toward upper bound.
    Forward,
    /// Animating toward lower bound.
    Reverse,
    /// At upper bound, idle.
    Completed,
}

impl AnimationStatus {
    /// Whether the animation is currently in motion (forward or reverse).
    pub fn is_animating(self) -> bool {
        matches!(self, Self::Forward | Self::Reverse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_dismissed() {
        assert_eq!(AnimationStatus::default(), AnimationStatus::Dismissed);
    }

    #[test]
    fn is_animating_only_for_forward_and_reverse() {
        assert!(!AnimationStatus::Dismissed.is_animating());
        assert!(AnimationStatus::Forward.is_animating());
        assert!(AnimationStatus::Reverse.is_animating());
        assert!(!AnimationStatus::Completed.is_animating());
    }
}
