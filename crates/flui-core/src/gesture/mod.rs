//! Gesture arena, normalized pointer events, hit-test protocol, and
//! competing recognizers (`Tap`, `DoubleTap`, `LongPress`, `Drag`-family,
//! `Scale`).
//!
//! See `docs/superpowers/specs/2026-05-06-S07-gesture-arena-design.md`
//! for the full design.
//!
//! # Architecture
//!
//! ```text
//!    PlatformInput (interactive.rs)
//!           │
//!           ▼
//!     dispatch::convert*  ──►  PointerEvent / PointerSignalEvent
//!           │                          │
//!           ▼                          │
//!     PointerSanitizer (orphan-Down,   │
//!     duplicate-Down rejection,        │
//!     Hover frame-to-frame Enter/Exit) │
//!           │                          │
//!           ▼                          ▼
//!     Window::hit_test()          dedicated listeners
//!           │                     (scroll/magnify;
//!           │                      bypass arena)
//!           ▼
//!     GestureBinding (per-Window)
//!     ├── GestureArenaManager
//!     │     ├── per-pointer GestureArena
//!     │     │     └── GestureArenaEntry × N
//!     │     │           (Rc<RefCell<Box<dyn GestureRecognizer>>>)
//!     │     └── arenas: SmallVec<[(PointerId, GestureArena); 4]>
//!     ├── GestureSettings (Flutter-parity defaults)
//!     └── PointerSanitizer
//!           │
//!           ▼
//!     dispatch loop:
//!       handle_event → Accepted? → declare winner, reject losers
//!                    → Rejected? → drop entry
//!                    → Possible? → keep
//!       on `Up`: sweep → first-registered wins
//! ```
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
//! # Performance characteristics
//!
//! Targets verified by the bench fixture
//! (`cargo run -p flui-core --release --example gesture_arena_bench`):
//!
//! | Operation                           | Budget (M2-class)   | Storage                          |
//! |-------------------------------------|---------------------|----------------------------------|
//! | `Window::hit_test`                  | < 2 µs/query        | `SmallVec<[HitboxId; 8]>`        |
//! | `arena.dispatch` per event-recognizer | < 1.25 µs           | `SmallVec<[GestureArenaEntry; 4]>` |
//! | Full frame at 120 Hz                | < 8 ms p99          | inline storage above             |
//!
//! Allocations on the dispatch hot path: zero. `VelocityTracker`
//! amortizes via a bounded `VecDeque` (max 20 samples by default).
//!
//! # S07.5 — completed
//!
//! S07.5 closed out the T15.5 backlog the merged S07 PR deferred:
//!
//! - **DoubleTap hold/release wired through dispatch.** `arena.hold`
//!   runs on `Down` for any recognizer that opts into
//!   [`recognizer::RecognizerLifecycle::needs_arena_hold`]; a
//!   per-pointer `Task<()>` stored on [`binding::GestureBinding`]
//!   schedules the deferred `arena.release` after
//!   `double_tap_timeout`. Cancellation paths drop the timer when
//!   the second tap accepts, when the arena is cancelled, or when
//!   the binding itself drops.
//! - **LongPress timer-driven acceptance.** `LongPress` registers a
//!   [`arena::ArenaBackChannel`] (a `Weak`-backed handle to the
//!   per-window `GestureArenaManager`) and the spawned timer task
//!   upgrades it to call `arena.declare_winner` on expiry. Window
//!   teardown is a silent no-op via the `Weak` upgrade contract.
//! - **`merge_by_pointer_id`.** The dispatcher's `mem::take`/restore
//!   dance now folds callback-time registrations back into the
//!   snapshot without producing duplicate `(PointerId, GestureArena)`
//!   pairs. Locked by P-T15.5-A property test.
//! - **`MouseExit` → `PointerPhase::Removed`.** Per-target leave
//!   events stay synthesized via [`dispatch::PointerSanitizer::diff_hover`]
//!   as `Exit`; device-leave is `Removed`. See
//!   [`pointer_event::PointerPhase`] for the distinction.
//! - **Per-window settings flow.** `GestureBinding::register_recognizer`
//!   invokes [`recognizer::RecognizerLifecycle::configure_settings`]
//!   so `window.gesture_settings_mut()` overrides actually take
//!   effect for recognizers built via fluent `__internal_on_*`
//!   helpers (which run inside `render()` and thus previously baked
//!   in `GestureSettings::default()` at construction).
//! - **State consolidation.** `Window::gesture_sanitizer` and
//!   `Window::gesture_pointer_state` are gone; both live inside
//!   `GestureBinding` now, accessed via `pub(crate)` accessors.
//! - **`test-support` decoupling.** The feature no longer pulls
//!   `wayland` + `x11` transitively; opt into
//!   `test-support-with-platform` for those. Locks Windows CI on
//!   `cargo check -p flui-core --features test-support`.
//! - **End-to-end integration test.** Paints a `div()` with each
//!   public recognizer family and drives `simulate_*` through
//!   `Window::dispatch_event`, locking the
//!   `paint → pending_recognizers → register_recognizer →
//!   arena.dispatch → callback` chain.
//!
//! Adding a new recognizer? See
//! `docs/superpowers/specs/2026-05-08-recognizer-extension.md`
//! for the canonical recipe.
//!
//! # Common pitfalls
//!
//! - **Do not call `cx.stop_propagation()` from inside
//!   [`recognizer::GestureRecognizer::handle_event`].** Propagation
//!   control belongs to the raw-listener chain
//!   (`on_mouse_*`/`on_click`); the arena declares winners via
//!   [`arena::GestureDisposition::Accepted`]. The dispatcher resets
//!   `cx.propagate_event = true` between the arena pass and the raw
//!   chain to preserve the `cx.active_drag`/`AnyDrag` contract.
//! - **`HitTestBehavior` ≠ `HitboxBehavior`.** They are orthogonal —
//!   see the table on [`hit_test::HitTestBehavior`]. Setting one does
//!   not change the other.
//! - **`PointerSignalEvent` (Scroll/Magnify) bypasses the arena.**
//!   Do not register a recognizer expecting to compete on scroll
//!   data. Use `on_scroll_wheel` or the dedicated pinch recognizer
//!   instead.
//! - **Recognizer drop must cancel async work.** `LongPress` stores
//!   its `Task<()>` in a field; dropping the recognizer drops the
//!   task, cancelling the future. Do not store the task anywhere
//!   else without thinking through cancellation.
//! - **`PointerEvent` is `#[non_exhaustive]`.** External crates
//!   cannot construct one — that is by design (the per-`Window`
//!   `PointerId` allocator lives inside `dispatch`). Synthesize
//!   events via integration-style tests with `TestAppContext`, not
//!   external `tests/`.
//!
//! # Explicit gaps (deferred to future specs)
//!
//! - Stylus tilt / orientation / azimuth — `PointerKind::Stylus`
//!   exists but the `tilt`/`orientation` fields are zero on every
//!   current platform. Closing this gap requires platform-layer
//!   work in `crates/flui-platform/`.
//! - Pinch rotation on desktop — `PinchEvent.delta` is scale-only;
//!   the recognizer's rotation field is always 0.0 today.
//! - Windows native pinch — `PinchEvent` is `#[cfg(any(target_os =
//!   "linux", target_os = "macos"))]`; Windows desktop trackpad does
//!   not produce pinch events.
//! - Spatial-index hit-test — `SmallVec<[HitboxId; 8]>` is a linear
//!   scan; BVH/quadtree upgrade is deferred to a P-track perf
//!   milestone (only relevant for trees > 100 hitboxes).
//! - `tracing` migration — current `log` + `kv` is the right call
//!   until cross-cutting milestone A4 picks the workspace policy.
//! - Public `GestureArenaTeam` registration on `InteractiveElement`
//!   — the team type itself is `pub`, but `Interactivity` does not
//!   yet expose a fluent builder for captain-deferred groupings.
//!   Future spec (likely `GestureDetector`) will surface this.
//!
//! # Migration guide (raw `on_mouse_*` → arena-driven recognizers)
//!
//! Existing raw listeners continue to fire in parallel with the
//! arena — the new system is additive. To migrate to arena-driven
//! recognition:
//!
//! ```text
//!   div().on_mouse_down(...)  ──►  div().on_tap(...)         (single tap)
//!   div().on_click(...)       ──►  div().on_double_tap(...)  (double tap)
//!   on_mouse_down + manual    ──►  div().on_pan_start(...)
//!   slop+timer for drag              .on_pan_update(...)
//!                                    .on_pan_end(...)
//!   on_mouse_down + manual    ──►  div().on_long_press_start(...)
//!   500ms timer for press            .on_long_press_end(...)
//! ```
//!
//! The arena resolves competition automatically: a `Tap` and `Pan`
//! on the same element will not both fire — slop crossing makes
//! `Pan` win and `Tap` lose. Mix and match recognizers on one
//! element via fluent builders on `InteractiveElement` (see T14).
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

// Submodules are `pub(crate)` so the public surface is **only** the
// per-symbol `pub use` block below (mirrored by the canonical flat
// path in `lib.rs`). Without this discipline, downstream code could
// reach `flui_core::gesture::arena::GestureDisposition` and
// `flui_core::gesture::recognizers::TapGestureRecognizer` via the
// module namespace, doubling the public path-set and pulling future
// `pub` items inside these modules into the semver surface for free.
pub(crate) mod arena;
pub(crate) mod arena_team;
pub(crate) mod binding;
pub(crate) mod dispatch;
pub(crate) mod gesture_settings;
pub(crate) mod hit_test;
pub(crate) mod pan_zoom_event;
pub(crate) mod pointer_event;
pub(crate) mod pointer_signal;
pub(crate) mod recognizer;
// `recognizers` stays `pub` because the canonical fluent-builder
// methods on `InteractiveElement` reference recognizer types via
// `crate::gesture::recognizers::TapDetails` — those references are
// inside the crate, so `pub(crate)` would also work. We keep `pub`
// here so the curated `pub use gesture::recognizers::{...}` block in
// `lib.rs` remains a stable canonical path and downstream consumers
// have one — and only one — module-qualified alternative
// (`flui_core::gesture::recognizers::TapGestureRecognizer`) for the
// `*Details` and recognizer types. The flat `flui_core::*` path is
// the recommended canonical form.
pub mod recognizers;
pub(crate) mod velocity_tracker;

// Per-symbol re-exports — kept in sync with the explicit `pub use
// gesture::{ … }` block in `crates/flui-core/src/lib.rs`. New
// pub items added under `gesture::` MUST be enumerated here.

pub use arena::{ArenaBackChannel, GestureDisposition};
pub use arena_team::GestureArenaTeam;
pub use binding::GestureBinding;
pub use gesture_settings::GestureSettings;
pub use hit_test::{HitTestBehavior, HitTestEntry, HitTestResult, HitTestScope};
pub use pan_zoom_event::{PanZoomPhase, PointerPanZoomEvent};
pub use pointer_event::{
    DeliveredEvent, PointerButtons, PointerEvent, PointerEventProvenance, PointerId, PointerKind,
    PointerPhase, PressureSample,
};
pub use pointer_signal::PointerSignalEvent;
pub use recognizer::{GestureRecognizer, RecognizerLifecycle, SemanticAction};
pub use velocity_tracker::{PositionSample, Velocity, VelocityTracker};

use crate::Modifiers;

/// Gating predicate evaluated by `GestureBinding::register_recognizer`
/// before the recognizer joins the arena, allowing per-recognizer
/// rejection rules that depend on the buttons + modifiers carried by
/// the registering pointer event.
///
/// **Why a newtype, not a `pub type X = dyn Fn(...)` alias.** A `dyn`
/// trait alias is unnameable in `impl Trait` return position, prints
/// as a verbose error message, cannot grow methods, and cannot
/// override auto-traits. The newtype wrapping a `Box<dyn Fn>` keeps
/// the surface short while leaving room to add observation methods
/// (e.g. a `name()` for trace logging) later — no breaking change.
///
/// **Why `Fn`, not `FnMut`.** The filter is queried once per
/// registration; it has no need to mutate captured state. Keeping
/// `Fn` also keeps the recognizer's interior-mutability surface flat
/// (audit cross-cut A7).
///
/// Construction: [`Self::new`] with any `Fn(PointerButtons,
/// Modifiers) -> bool + 'static` closure. Evaluation: [`Self::call`].
///
/// `register_recognizer` evaluates the filter **before** adding the
/// recognizer to the arena (Decision D10). On `false` the recognizer
/// short-circuits and is not registered — never enters the arena and
/// never returns `Possible` indefinitely.
pub struct AllowedButtonsFilter(Box<dyn Fn(PointerButtons, Modifiers) -> bool + 'static>);

impl AllowedButtonsFilter {
    /// Wrap an arbitrary `Fn` closure as a filter. The closure must
    /// be `'static` because filters outlive any specific stack frame
    /// (recognizers store them as fields on `'static` types).
    pub fn new(f: impl Fn(PointerButtons, Modifiers) -> bool + 'static) -> Self {
        Self(Box::new(f))
    }

    /// Evaluate the filter for the given button + modifier state.
    /// `true` means the recognizer should be admitted; `false` means
    /// `register_recognizer` will skip the `arena.add` call.
    pub fn call(&self, buttons: PointerButtons, modifiers: Modifiers) -> bool {
        (self.0)(buttons, modifiers)
    }
}

impl std::fmt::Debug for AllowedButtonsFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AllowedButtonsFilter(<closure>)")
    }
}

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
    let pos = recs.iter_mut().position(|r| r.as_any_mut().is::<T>());
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
