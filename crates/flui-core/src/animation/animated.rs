// crates/flui-core/src/animation/animated.rs

#![allow(missing_docs)] // animation subsystem is pre-1.0; full rustdoc coverage tracked separately

use crate::animation::controller::AnimationController;
use crate::{App, Entity, IntoElement, Window};

/// Render with an AnimationController, automatically scheduling frame updates.
///
/// Reads the controller's current value, calls `builder` with it, and
/// schedules the next frame if the animation is still running.
/// Users never need to call `window.request_animation_frame()` manually.
///
/// # Example
/// ```ignore
/// animated(&self.fade, window, cx, |opacity| {
///     div().opacity(opacity).child("Fading in...")
/// })
/// ```
pub fn animated<E: IntoElement>(
    controller: &Entity<AnimationController>,
    window: &mut Window,
    cx: &App,
    builder: impl FnOnce(f32) -> E,
) -> E {
    let (value, animating) = {
        let ctrl = controller.read(cx);
        (ctrl.value(), ctrl.is_animating())
    };

    if animating {
        window.request_animation_frame();
    }

    builder(value)
}
