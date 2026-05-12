// crates/flui-core/src/animation/animated.rs

use crate::animation::controller::AnimationController;
use crate::{App, Entity, IntoElement, Window};

/// Render with an AnimationController, automatically scheduling frame updates.
///
/// Reads the controller's current value, calls `builder` with it, and — if
/// the animation is still running — calls
/// [`Window::request_animation_frame`](crate::Window::request_animation_frame)
/// internally so the next frame redraws this view. Consumers of `animated()`
/// do not need to drive `request_animation_frame` themselves; the helper owns
/// that scheduling.
///
/// Per K04 Task 32, `request_animation_frame` is idempotent within a frame —
/// calling `animated()` from every layout pass during an in-flight animation
/// collapses to one frame request.
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
