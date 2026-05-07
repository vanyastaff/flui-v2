//! `GestureRecognizer` trait + `RecognizerLifecycle` trait + `SemanticAction` enum.
//!
//! `GestureRecognizer: ?Send + ?Sync`. Per-`Window` callback registry
//! is main-thread-only; matches the existing `Interactivity` posture.
//!
//! See the design doc § "GestureRecognizer trait".

use super::arena::ArenaBackChannel;
use super::{AllowedButtonsFilter, DeliveredEvent, GestureDisposition, GestureSettings, PointerId};
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
    ///
    /// `event.local_position` is the hit-target-local pointer position
    /// at the moment the recognizer was registered; recognizers must
    /// use it (rather than `event.event.position`) when initialising
    /// per-pointer state such as `down_position`.
    fn add_pointer(&mut self, pointer_id: PointerId, event: DeliveredEvent<'_>);

    /// A new event arrived for a tracked pointer. Recognizers may
    /// **eagerly accept** by returning [`GestureDisposition::Accepted`]
    /// or **eagerly reject** with [`GestureDisposition::Rejected`].
    /// Returning [`GestureDisposition::Possible`] keeps the recognizer
    /// in the arena.
    ///
    /// The dispatcher passes a [`DeliveredEvent`] carrying the
    /// underlying `&PointerEvent` plus a per-recognizer
    /// `local_position`. Recognizers must read
    /// `event.local_position` for any in-target geometry (slop,
    /// distance, drag delta) and `event.event.<field>` for everything
    /// else (kind, phase, buttons, timestamps, pressure).
    fn handle_event(
        &mut self,
        event: DeliveredEvent<'_>,
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
    fn rejected(&mut self, pointer_id: PointerId, window: &mut crate::Window, cx: &mut crate::App);

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

    /// Optional per-recognizer button + modifier gating predicate.
    ///
    /// Returned by recognizers that opt into [`AllowedButtonsFilter`]
    /// gating (typically via a `with_allowed_buttons_filter`
    /// builder). [`super::GestureBinding::register_recognizer`]
    /// evaluates this filter on the registering pointer event before
    /// `arena.add`; on rejection the recognizer is not added (Decision
    /// D10). Default `None` — no extra gating beyond the recognizer's
    /// own `add_pointer` button check.
    fn allowed_buttons_filter(&self) -> Option<&AllowedButtonsFilter> {
        None
    }

    /// Optional access to per-recognizer lifecycle hooks.
    ///
    /// Default returns `None`; recognizers that opt in (e.g. `LongPress`
    /// for the back-channel, `DoubleTap` for arena hold) override to
    /// return `Some(self)`. `RecognizerLifecycle` is a sibling trait —
    /// **not** a supertrait — so existing impls (mocks, third-party
    /// recognizers, S08/S12 stubs) compile unchanged.
    ///
    /// **Why `Option` instead of `Any`-downcast:** zero-cost no-op for
    /// recognizers that opt out, and the override is one line per impl.
    /// **Why not a supertrait:** that would force every existing
    /// `GestureRecognizer` impl to also `impl RecognizerLifecycle`,
    /// which is a breaking change to the public surface.
    fn lifecycle(&mut self) -> Option<&mut dyn RecognizerLifecycle> {
        None
    }
}

/// Per-recognizer lifecycle hooks invoked by [`super::GestureBinding`]
/// at registration time.
///
/// Recognizers that need any of:
/// - the per-window arena back-channel (e.g. `LongPress` to
///   `arena.declare_winner` on its timer fire),
/// - arena `hold` semantics (e.g. `DoubleTap` extending past `Up`),
/// - per-window [`GestureSettings`] applied at registration (rather
///   than at construction, so `window.gesture_settings_mut()` overrides
///   take effect),
///
/// override the matching method.
///
/// The trait is a **sibling** to [`GestureRecognizer`], reachable only
/// through [`GestureRecognizer::lifecycle`]. Existing impls are
/// unaffected; new recognizers opt in by overriding `lifecycle` to
/// return `Some(self)` and implementing this trait. See
/// `docs/superpowers/specs/2026-05-08-recognizer-extension.md` for the
/// step-by-step recipe.
///
/// All methods have default no-op bodies, so opting in to one method
/// does not require implementing the others. Adding a new method with
/// a default body is a non-breaking change for the same reason.
pub trait RecognizerLifecycle {
    /// Whether this recognizer wants the per-window arena back-channel
    /// injected via [`Self::set_arena_back_channel`].
    ///
    /// Default `false` — most recognizers (Tap, Drag, Scale) do not
    /// need to call back into the arena from external state (e.g.
    /// timers) and so do not need a `Weak` handle.
    fn needs_back_channel(&self) -> bool {
        false
    }

    /// Inject the per-window arena back-channel + the recognizer's
    /// entry index into the arena. Called only when
    /// [`Self::needs_back_channel`] returns `true`.
    ///
    /// `pointer_id` is the pointer for which the recognizer is being
    /// registered. Multi-pointer recognizers (e.g. `LongPress`,
    /// future `MultiTap`) MUST keep one `(pointer_id, entry_index)`
    /// pair per pointer they track — a single boolean / scalar slot
    /// silently conflates concurrent in-flight presses.
    ///
    /// `back_channel` is an opaque [`ArenaBackChannel`] that stays
    /// valid while the per-window arena is alive and degrades into a
    /// silent no-op once the `Window` (and its `GestureBinding`)
    /// drops. This avoids dangling pointers during window-resize /
    /// window-close races where a recognizer's timer might still fire
    /// after the window is gone.
    ///
    /// `entry_index` is the recognizer's slot in the arena's
    /// `entries` vec at registration time, suitable for
    /// `back_channel.declare_winner(pointer_id, entry_index, ...)`
    /// from a timer callback.
    fn set_arena_back_channel(
        &mut self,
        _pointer_id: PointerId,
        _back_channel: ArenaBackChannel,
        _entry_index: usize,
    ) {
    }

    /// Whether this recognizer wants the arena to enter `hold` mode on
    /// the initial `Down`, deferring the sweep-on-`Up` resolution until
    /// an explicit `release` (typically from a timer expiry).
    ///
    /// Default `false`. `DoubleTap` overrides to `true` so the arena
    /// stays open after the first `Up` long enough for the second tap
    /// to land.
    fn needs_arena_hold(&self) -> bool {
        false
    }

    /// Apply the per-window [`GestureSettings`] to the recognizer's
    /// thresholds. Called at registration time so that
    /// `window.gesture_settings_mut()` overrides take effect for
    /// recognizers built via the fluent `__internal_on_*` helpers
    /// (which run inside `render()` and therefore previously baked in
    /// `GestureSettings::default()` at construction).
    ///
    /// Default no-op. Each recognizer overrides to read the relevant
    /// fields (`pan_slop`, `long_press_timeout`, `double_tap_timeout`,
    /// etc.) from `settings`.
    fn configure_settings(&mut self, _settings: &GestureSettings) {}
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
    use crate::Modifiers;
    use crate::gesture::{
        PointerButtons, recognizers::DoubleTapGestureRecognizer,
        recognizers::HorizontalDragGestureRecognizer, recognizers::LongPressGestureRecognizer,
        recognizers::PanGestureRecognizer, recognizers::ScaleGestureRecognizer,
        recognizers::TapGestureRecognizer, recognizers::VerticalDragGestureRecognizer,
    };

    /// Compile-time assertion that `GestureRecognizer` is object-safe.
    /// Will fail to compile if a future change makes the trait
    /// non-`dyn`-compatible.
    fn _assert_object_safe(_: &dyn GestureRecognizer) {}

    /// T20 canary — every recognizer that ships
    /// `with_allowed_buttons_filter` must surface the closure through
    /// `GestureRecognizer::allowed_buttons_filter`. The default trait
    /// method returns `None`; overrides return `Some(self.<field>)`.
    /// Without that override, `register_recognizer` would never see
    /// the filter and Decision D10's gating becomes a silent no-op.
    ///
    /// All seven recognizer families use the documented fluent
    /// `with_allowed_buttons_filter` builder — keeping the test
    /// shape uniform with the canonical `recognizer-extension.md`
    /// recipe (rather than mixing with raw `pub field = Some(...)`
    /// assignment, which works but is the unidiomatic form for
    /// downstream readers).
    #[test]
    fn every_recognizer_surfaces_allowed_buttons_filter() {
        use crate::gesture::GestureSettings;

        let s = GestureSettings::default();

        // Each recognizer family installs an always-false filter via
        // the canonical builder, then we read it back through the
        // trait method and verify the closure is reachable. The
        // closure body is not exercised here — the binding-side
        // integration test in `gesture_dispatch_integration.rs`
        // covers the full register_recognizer rejection path.

        let tap = TapGestureRecognizer::new(&s).with_allowed_buttons_filter(|_, _| false);
        assert!(
            tap.allowed_buttons_filter()
                .is_some_and(|f| !f.call(PointerButtons::PRIMARY, Modifiers::default())),
            "tap surfaces filter and forwards arguments"
        );

        let dt = DoubleTapGestureRecognizer::new(&s).with_allowed_buttons_filter(|_, _| false);
        assert!(dt.allowed_buttons_filter().is_some(), "double_tap surfaces filter");

        let lp = LongPressGestureRecognizer::new(&s).with_allowed_buttons_filter(|_, _| false);
        assert!(lp.allowed_buttons_filter().is_some(), "long_press surfaces filter");

        let pan = PanGestureRecognizer::new(&s).with_allowed_buttons_filter(|_, _| false);
        assert!(pan.allowed_buttons_filter().is_some(), "pan surfaces filter");

        let hdrag =
            HorizontalDragGestureRecognizer::new(&s).with_allowed_buttons_filter(|_, _| false);
        assert!(hdrag.allowed_buttons_filter().is_some(), "hdrag surfaces filter");

        let vdrag =
            VerticalDragGestureRecognizer::new(&s).with_allowed_buttons_filter(|_, _| false);
        assert!(vdrag.allowed_buttons_filter().is_some(), "vdrag surfaces filter");

        let scale = ScaleGestureRecognizer::new(&s).with_allowed_buttons_filter(|_, _| false);
        assert!(scale.allowed_buttons_filter().is_some(), "scale surfaces filter");
    }

    /// Default trait body returns `None` — recognizers that opt out
    /// of `allowed_buttons_filter` (the common case) keep the
    /// register-recognizer fast-path.
    #[test]
    fn allowed_buttons_filter_default_is_none() {
        use crate::gesture::GestureSettings;
        let s = GestureSettings::default();

        // No filter installed → `allowed_buttons_filter()` returns
        // `None`.
        let tap = TapGestureRecognizer::new(&s);
        assert!(tap.allowed_buttons_filter().is_none());
    }
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
