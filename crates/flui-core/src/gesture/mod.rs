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

