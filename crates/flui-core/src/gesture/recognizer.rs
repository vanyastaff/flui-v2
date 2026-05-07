//! `GestureRecognizer` trait + `SemanticAction` enum.
//!
//! `GestureRecognizer: ?Send + ?Sync`. Per-`Window` callback registry
//! is main-thread-only; matches the existing `Interactivity` posture.
//!
//! See the design doc § "GestureRecognizer trait".

use super::{GestureDisposition, PointerEvent, PointerId};
use crate::FocusHandle;

/// One competitor in the gesture arena.
///
/// **Object-safety:** the trait is `dyn`-compatible — verified by use
/// in `GestureArenaEntry`'s `Rc<RefCell<dyn GestureRecognizer>>` and
/// by [`__assert_object_safe`] below.
///
/// **Threading:** `?Sync` (and `?Send`) — recognizers self-mutate
/// from inside arena callbacks on the main thread only.
///
/// **Trait contract:** implementations MUST NOT call
/// `cx.stop_propagation()` from inside [`Self::handle_event`]. The
/// arena declares its winner via [`GestureDisposition::Accepted`],
/// not via propagation control. The dispatcher resets
/// `cx.propagate_event = true` between the arena pass and the
/// existing raw-listener chain to preserve the `cx.active_drag` /
/// `AnyDrag` contract.
///
/// **Drop guarantee:** dropping a recognizer must cancel any
/// in-flight asynchronous work (e.g. `LongPress` timers).
/// Implementations MUST store `Task` handles such that dropping the
/// recognizer drops the `Task` and cancels its future.
pub trait GestureRecognizer: 'static {
    /// Downcast hook for the fluent-builder API on `InteractiveElement`.
    /// Implementations should return `self` directly. The trait remains
    /// object-safe; this method is the standard pattern for typed
    /// downcasting through `Box<dyn GestureRecognizer>`.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    /// A short human-readable name (e.g. `"tap"`, `"long_press"`).
    /// Used in `log::*` `kv` fields.
    fn name(&self) -> &'static str;

    /// The recognizer is being added to the arena for `pointer_id`.
    /// Recognizers track per-pointer state internally.
    fn add_pointer(&mut self, pointer_id: PointerId, event: &PointerEvent);

    /// A new event arrived for a tracked pointer. Recognizers may
    /// **eagerly accept** by returning [`GestureDisposition::Accepted`]
    /// or **eagerly reject** with [`GestureDisposition::Rejected`].
    /// Returning [`GestureDisposition::Possible`] keeps the recognizer
    /// in the arena.
    fn handle_event(
        &mut self,
        event: &PointerEvent,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) -> GestureDisposition;

    /// Sweep — fire delegated callbacks if this recognizer "won" the
    /// arena via sweep semantics (last competitor remaining on `Up`).
    /// Called by the arena manager exactly once per pointer when the
    /// arena resolves; recognizers that already returned `Accepted`
    /// from [`Self::handle_event`] will not see a `sweep_accepted`
    /// call.
    fn sweep_accepted(
        &mut self,
        pointer_id: PointerId,
        window: &mut crate::Window,
        cx: &mut crate::App,
    );

    /// The arena resolved against this recognizer. Recognizers MUST
    /// reset any in-flight visual state (LongPress feedback, cursor
    /// styling) without firing user callbacks.
    fn rejected(
        &mut self,
        pointer_id: PointerId,
        window: &mut crate::Window,
        cx: &mut crate::App,
    );

    /// **S08 seam.** The set of semantic actions this recognizer
    /// surfaces to the accessibility tree. Default empty; S08 will
    /// populate Tap/DoubleTap/LongPress recognizers' overrides.
    fn semantic_actions(&self) -> &'static [SemanticAction] {
        &[]
    }

    /// **S12 seam.** The focus handle this recognizer wishes to
    /// claim on accept (e.g. a button claims focus on tap-down).
    /// Default `None`; `TapGestureRecognizer` overrides when
    /// `request_focus_on_tap_down` is set.
    fn on_focus_request(&self) -> Option<FocusHandle> {
        None
    }
}

/// Semantic-action enum (S08 seam — default-empty here, populated in S08).
///
/// `#[non_exhaustive]` so S08 may add `Increment`, `Decrement`,
/// `Move`, etc. without a breaking change.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum SemanticAction {
    /// Single-tap action.
    Tap,
    /// Double-tap action.
    DoubleTap,
    /// Long-press action.
    LongPress,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time assertion that `GestureRecognizer` is object-safe.
    /// Will fail to compile if a future change makes the trait
    /// non-`dyn`-compatible.
    fn _assert_object_safe(_: &dyn GestureRecognizer) {}
}

/// Compile-time assertion that `GestureRecognizer` is object-safe.
/// `Box<dyn GestureRecognizer>` would fail to typecheck if a future
/// signature change broke object-safety.
///
/// The `__` prefix marks this as an internal helper that happens to
/// be `pub` only because trait-method default bodies in
/// `crates/flui-core/src/elements/div.rs` reach it across the module
/// boundary. The function is `#[doc(hidden)]`, but the prefix makes
/// the intent visible in IDE autocomplete listings — match the
/// `__internal_*` convention used by the fluent-builder helpers in
/// `gesture/mod.rs`.
#[doc(hidden)]
pub fn __assert_object_safe(_: Box<dyn GestureRecognizer>) {}
