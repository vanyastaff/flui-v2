// crates/flui-core/src/animation/behavior.rs
//
// S21 phase 4: behaviour + style override types consumed by
// `AnimationController::animate_to` / `animate_back` / `fling` /
// `with_behavior` / `with_style`.

#![allow(missing_docs)] // animation subsystem is pre-1.0; rustdoc filled in under S21 phase 7

use std::time::Duration;

use super::curve::Curve;

/// How the controller should respond to system-level animation hints
/// (notably the future `MediaQueryData.disableAnimations` accessibility flag,
/// landing alongside S08 / S14).
///
/// **Flutter parity:** corresponds to
/// [`AnimationBehavior`](https://api.flutter.dev/flutter/animation/AnimationBehavior.html).
///
/// `#[non_exhaustive]` because future variants may be added (e.g. a
/// "ReduceMotion" variant once S14 lands).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AnimationBehavior {
    /// Default: respect any future system-level mute / reduce-motion hint.
    /// Ticking and listener fan-out remain enabled but the controller may
    /// short-circuit to the target state instantly when the platform asks.
    #[default]
    Normal,
    /// Preserve user-visible motion regardless of system hints.
    /// Phase 4 ships the variant; the actual integration with
    /// `MediaQueryData.disableAnimations` lands when S14 wires the
    /// accessibility flags.
    Preserve,
}

/// Override bag for ad-hoc per-call duration / curve customization on
/// [`animate_to`] / [`animate_back`] / [`with_style`].
///
/// **Flutter parity:** corresponds to
/// [`AnimationStyle`](https://api.flutter.dev/flutter/material/AnimationStyle-class.html).
/// All fields are `Option`; `None` means "fall back to the controller's
/// default". Construct via `AnimationStyle::default()` and chain setters,
/// or instantiate directly.
///
/// [`animate_to`]: crate::animation::AnimationController::animate_to
/// [`animate_back`]: crate::animation::AnimationController::animate_back
/// [`with_style`]: crate::animation::AnimationController::with_style
#[derive(Default)]
pub struct AnimationStyle {
    pub duration: Option<Duration>,
    pub reverse_duration: Option<Duration>,
    pub curve: Option<Box<dyn Curve>>,
    pub reverse_curve: Option<Box<dyn Curve>>,
}

impl AnimationStyle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn with_reverse_duration(mut self, duration: Duration) -> Self {
        self.reverse_duration = Some(duration);
        self
    }

    pub fn with_curve<C: Curve>(mut self, curve: C) -> Self {
        self.curve = Some(Box::new(curve));
        self
    }

    pub fn with_reverse_curve<C: Curve>(mut self, curve: C) -> Self {
        self.reverse_curve = Some(Box::new(curve));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::curve::EaseIn;

    #[test]
    fn default_animation_behavior_is_normal() {
        assert_eq!(AnimationBehavior::default(), AnimationBehavior::Normal);
    }

    #[test]
    fn animation_style_default_is_all_none() {
        let style = AnimationStyle::default();
        assert!(style.duration.is_none());
        assert!(style.reverse_duration.is_none());
        assert!(style.curve.is_none());
        assert!(style.reverse_curve.is_none());
    }

    #[test]
    fn animation_style_builder_chain() {
        let style = AnimationStyle::new()
            .with_duration(Duration::from_millis(500))
            .with_curve(EaseIn);
        assert_eq!(style.duration, Some(Duration::from_millis(500)));
        assert!(style.curve.is_some());
    }
}
