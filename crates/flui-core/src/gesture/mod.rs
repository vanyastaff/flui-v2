//! Gesture arena, normalized pointer events, hit-test protocol, and
//! competing recognizers (`Tap`, `DoubleTap`, `LongPress`, `Drag`-family,
//! `Scale`).
//!
//! See `docs/superpowers/specs/2026-05-06-S07-gesture-arena-design.md`
//! for the full design.
//!
//! # Module overview
//!
//! - [`pointer_event`] — `PointerEvent`, `PointerKind`, `PointerPhase`,
//!   `PointerId`, `PointerButtons`. The normalized wire format.
//! - [`pointer_signal`] — `PointerSignalEvent` (`Scroll` | `Magnify`),
//!   non-competitive signals that bypass the arena.
//! - [`hit_test`] — `HitTestEntry`, `HitTestResult`, `HitTestBehavior`.
//!   Reuses the existing `Hitbox` infrastructure.
//! - [`gesture_settings`] — `GestureSettings` (Flutter-parity defaults).
//! - [`binding`] — `GestureBinding`, the per-`Window` owner of arena +
//!   settings + sanitizer.
//! - [`dispatch`] — `PlatformInput` → `PointerEvent` conversion +
//!   `PointerSanitizer`. `pub(crate)` only.
//! - [`arena`] — `GestureArenaManager` and friends. The arena types
//!   themselves are `pub(crate)`; consumers reach the manager via
//!   `pub(crate)` accessors on `GestureBinding`.
//! - [`arena_team`] — `GestureArenaTeam`, captain-deferred grouping.
//! - [`recognizer`] — `GestureRecognizer` trait + `SemanticAction` enum.
//! - [`velocity_tracker`] — `VelocityTracker` + `Velocity` +
//!   `PositionSample`. Flutter-LSQ port.
//! - [`recognizers`] — Five concrete recognizers + their `*Details`
//!   types.
//!
//! # Naming note
//!
//! `flui_core::gesture::TapGestureRecognizer` is reachable via this
//! module path. The flat path `flui_core::TapGestureRecognizer` is the
//! **canonical** consumer path (re-exported from `lib.rs`).
//!
//! # Relation to the existing `GestureEvent` trait
//!
//! `flui_core::GestureEvent` (defined at
//! `crates/flui-core/src/interactive.rs`) is the platform-input
//! marker trait (e.g. `impl GestureEvent for PinchEvent`). It is a
//! different concept from this module's `Gesture*` types
//! (recognizers, arenas, bindings). The two coexist: platform-side
//! `GestureEvent`s are translated into this module's `PointerEvent` /
//! `PointerSignalEvent` via the conversions in
//! [`dispatch`](self::dispatch).

pub mod arena;
pub mod arena_team;
pub mod binding;
pub(crate) mod dispatch;
pub mod gesture_settings;
pub mod hit_test;
pub mod pointer_event;
pub mod pointer_signal;
pub mod recognizer;
pub mod recognizers;
pub mod velocity_tracker;

// Per-symbol re-exports — kept in sync with the explicit `pub use
// gesture::{ … }` block in `crates/flui-core/src/lib.rs`. New
// pub items added under `gesture::` MUST be enumerated here.

pub use arena::GestureDisposition;
pub use arena_team::GestureArenaTeam;
pub use binding::GestureBinding;
pub use gesture_settings::GestureSettings;
pub use hit_test::{HitTestBehavior, HitTestEntry, HitTestResult};
pub use pointer_event::{PointerButtons, PointerEvent, PointerId, PointerKind, PointerPhase};
pub use pointer_signal::PointerSignalEvent;
pub use recognizer::{GestureRecognizer, SemanticAction};
pub use velocity_tracker::{PositionSample, Velocity, VelocityTracker};

// =====================================================================
// Internal fluent-builder helpers for `InteractiveElement` (T14).
// `#[doc(hidden)]` — they appear in the public surface only because
// the trait default-method bodies in `elements/div.rs` need to reach
// them across the module boundary. Renaming or removing them is not
// a public-API breaking change.
// =====================================================================

/// Find the recognizer of type `T` in `recs`, or push a new one
/// constructed from `Default::default()` settings.
#[doc(hidden)]
fn find_or_push<T: GestureRecognizer + 'static>(
    recs: &mut smallvec::SmallVec<[Box<dyn GestureRecognizer>; 4]>,
    new: impl FnOnce() -> T,
) -> &mut T {
    let pos = recs
        .iter_mut()
        .position(|r| r.as_any_mut().is::<T>());
    let idx = match pos {
        Some(idx) => idx,
        None => {
            recs.push(Box::new(new()));
            recs.len() - 1
        }
    };
    recs[idx]
        .as_any_mut()
        .downcast_mut::<T>()
        .expect("position() guarantees the type matches")
}

/// Borrow the gesture-recognizer vector from the supplied `Interactivity`.
/// Hidden helper used by the fluent-builder macros.
#[doc(hidden)]
pub fn __recognizers_mut(
    iv: &mut crate::elements::Interactivity,
) -> &mut smallvec::SmallVec<[Box<dyn GestureRecognizer>; 4]> {
    &mut iv.gesture_recognizers
}

#[doc(hidden)]
pub fn __internal_on_tap(
    iv: &mut crate::elements::Interactivity,
    f: impl FnMut(recognizers::TapDetails, &mut crate::Window, &mut crate::App) + 'static,
) {
    let r = find_or_push(__recognizers_mut(iv), || {
        recognizers::TapGestureRecognizer::new(&GestureSettings::default())
    });
    r.on_tap = Some(Box::new(f));
}

#[doc(hidden)]
pub fn __internal_on_double_tap(
    iv: &mut crate::elements::Interactivity,
    f: impl FnMut(recognizers::DoubleTapDetails, &mut crate::Window, &mut crate::App) + 'static,
) {
    let r = find_or_push(__recognizers_mut(iv), || {
        recognizers::DoubleTapGestureRecognizer::new(&GestureSettings::default())
    });
    r.on_double_tap = Some(Box::new(f));
}

#[doc(hidden)]
pub fn __internal_on_long_press_start(
    iv: &mut crate::elements::Interactivity,
    f: impl FnMut(recognizers::LongPressDetails, &mut crate::Window, &mut crate::App) + 'static,
) {
    let r = find_or_push(__recognizers_mut(iv), || {
        recognizers::LongPressGestureRecognizer::new(&GestureSettings::default())
    });
    r.on_long_press_start = Some(Box::new(f));
}

#[doc(hidden)]
pub fn __internal_on_long_press_move(
    iv: &mut crate::elements::Interactivity,
    f: impl FnMut(recognizers::LongPressDetails, &mut crate::Window, &mut crate::App) + 'static,
) {
    let r = find_or_push(__recognizers_mut(iv), || {
        recognizers::LongPressGestureRecognizer::new(&GestureSettings::default())
    });
    r.on_long_press_move = Some(Box::new(f));
}

#[doc(hidden)]
pub fn __internal_on_long_press_end(
    iv: &mut crate::elements::Interactivity,
    f: impl FnMut(recognizers::LongPressDetails, &mut crate::Window, &mut crate::App) + 'static,
) {
    let r = find_or_push(__recognizers_mut(iv), || {
        recognizers::LongPressGestureRecognizer::new(&GestureSettings::default())
    });
    r.on_long_press_end = Some(Box::new(f));
}

#[doc(hidden)]
pub fn __internal_on_pan_start(
    iv: &mut crate::elements::Interactivity,
    f: impl FnMut(recognizers::DragStartDetails, &mut crate::Window, &mut crate::App) + 'static,
) {
    let r = find_or_push(__recognizers_mut(iv), || {
        recognizers::PanGestureRecognizer::new(&GestureSettings::default())
    });
    *r = std::mem::replace(
        r,
        recognizers::PanGestureRecognizer::new(&GestureSettings::default()),
    )
    .on_start(f);
}

#[doc(hidden)]
pub fn __internal_on_pan_update(
    iv: &mut crate::elements::Interactivity,
    f: impl FnMut(recognizers::DragUpdateDetails, &mut crate::Window, &mut crate::App) + 'static,
) {
    let r = find_or_push(__recognizers_mut(iv), || {
        recognizers::PanGestureRecognizer::new(&GestureSettings::default())
    });
    *r = std::mem::replace(
        r,
        recognizers::PanGestureRecognizer::new(&GestureSettings::default()),
    )
    .on_update(f);
}

#[doc(hidden)]
pub fn __internal_on_pan_end(
    iv: &mut crate::elements::Interactivity,
    f: impl FnMut(recognizers::DragEndDetails, &mut crate::Window, &mut crate::App) + 'static,
) {
    let r = find_or_push(__recognizers_mut(iv), || {
        recognizers::PanGestureRecognizer::new(&GestureSettings::default())
    });
    *r = std::mem::replace(
        r,
        recognizers::PanGestureRecognizer::new(&GestureSettings::default()),
    )
    .on_end(f);
}

#[doc(hidden)]
pub fn __internal_on_horizontal_drag_start(
    iv: &mut crate::elements::Interactivity,
    f: impl FnMut(recognizers::DragStartDetails, &mut crate::Window, &mut crate::App) + 'static,
) {
    let r = find_or_push(__recognizers_mut(iv), || {
        recognizers::HorizontalDragGestureRecognizer::new(&GestureSettings::default())
    });
    *r = std::mem::replace(
        r,
        recognizers::HorizontalDragGestureRecognizer::new(&GestureSettings::default()),
    )
    .on_start(f);
}

#[doc(hidden)]
pub fn __internal_on_horizontal_drag_update(
    iv: &mut crate::elements::Interactivity,
    f: impl FnMut(recognizers::DragUpdateDetails, &mut crate::Window, &mut crate::App) + 'static,
) {
    let r = find_or_push(__recognizers_mut(iv), || {
        recognizers::HorizontalDragGestureRecognizer::new(&GestureSettings::default())
    });
    *r = std::mem::replace(
        r,
        recognizers::HorizontalDragGestureRecognizer::new(&GestureSettings::default()),
    )
    .on_update(f);
}

#[doc(hidden)]
pub fn __internal_on_horizontal_drag_end(
    iv: &mut crate::elements::Interactivity,
    f: impl FnMut(recognizers::DragEndDetails, &mut crate::Window, &mut crate::App) + 'static,
) {
    let r = find_or_push(__recognizers_mut(iv), || {
        recognizers::HorizontalDragGestureRecognizer::new(&GestureSettings::default())
    });
    *r = std::mem::replace(
        r,
        recognizers::HorizontalDragGestureRecognizer::new(&GestureSettings::default()),
    )
    .on_end(f);
}

#[doc(hidden)]
pub fn __internal_on_vertical_drag_start(
    iv: &mut crate::elements::Interactivity,
    f: impl FnMut(recognizers::DragStartDetails, &mut crate::Window, &mut crate::App) + 'static,
) {
    let r = find_or_push(__recognizers_mut(iv), || {
        recognizers::VerticalDragGestureRecognizer::new(&GestureSettings::default())
    });
    *r = std::mem::replace(
        r,
        recognizers::VerticalDragGestureRecognizer::new(&GestureSettings::default()),
    )
    .on_start(f);
}

#[doc(hidden)]
pub fn __internal_on_vertical_drag_update(
    iv: &mut crate::elements::Interactivity,
    f: impl FnMut(recognizers::DragUpdateDetails, &mut crate::Window, &mut crate::App) + 'static,
) {
    let r = find_or_push(__recognizers_mut(iv), || {
        recognizers::VerticalDragGestureRecognizer::new(&GestureSettings::default())
    });
    *r = std::mem::replace(
        r,
        recognizers::VerticalDragGestureRecognizer::new(&GestureSettings::default()),
    )
    .on_update(f);
}

#[doc(hidden)]
pub fn __internal_on_vertical_drag_end(
    iv: &mut crate::elements::Interactivity,
    f: impl FnMut(recognizers::DragEndDetails, &mut crate::Window, &mut crate::App) + 'static,
) {
    let r = find_or_push(__recognizers_mut(iv), || {
        recognizers::VerticalDragGestureRecognizer::new(&GestureSettings::default())
    });
    *r = std::mem::replace(
        r,
        recognizers::VerticalDragGestureRecognizer::new(&GestureSettings::default()),
    )
    .on_end(f);
}

#[doc(hidden)]
pub fn __internal_on_scale_start(
    iv: &mut crate::elements::Interactivity,
    f: impl FnMut(recognizers::ScaleStartDetails, &mut crate::Window, &mut crate::App) + 'static,
) {
    let r = find_or_push(__recognizers_mut(iv), || {
        recognizers::ScaleGestureRecognizer::new(&GestureSettings::default())
    });
    r.on_start = Some(Box::new(f));
}

#[doc(hidden)]
pub fn __internal_on_scale_update(
    iv: &mut crate::elements::Interactivity,
    f: impl FnMut(recognizers::ScaleUpdateDetails, &mut crate::Window, &mut crate::App) + 'static,
) {
    let r = find_or_push(__recognizers_mut(iv), || {
        recognizers::ScaleGestureRecognizer::new(&GestureSettings::default())
    });
    r.on_update = Some(Box::new(f));
}

#[doc(hidden)]
pub fn __internal_on_scale_end(
    iv: &mut crate::elements::Interactivity,
    f: impl FnMut(recognizers::ScaleEndDetails, &mut crate::Window, &mut crate::App) + 'static,
) {
    let r = find_or_push(__recognizers_mut(iv), || {
        recognizers::ScaleGestureRecognizer::new(&GestureSettings::default())
    });
    r.on_end = Some(Box::new(f));
}

