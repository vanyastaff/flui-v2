//! `GestureBinding` — per-`Window` owner of the arena, settings, the
//! sanitizer, the per-pointer state cache, and the per-pointer
//! arena-hold timeout timers.
//!
//! Auto-trait posture: `!Send + !Sync` (transitively via
//! `Rc<RefCell<dyn GestureRecognizer>>` inside the arena).
//! Per-`Window` types are main-thread-only by construction; do
//! **not** wrap a `GestureBinding` in `Arc`.
//!
//! See the design doc § "GestureBinding".

use super::arena::GestureArenaManager;
use super::dispatch::{PointerSanitizer, WindowPointerState};
use super::{GestureRecognizer, GestureSettings, PointerId};
use crate::{AppContext, Task};
use std::cell::RefCell;
use std::rc::Rc;

/// Per-window owner of the gesture arena, the configurable
/// [`GestureSettings`], the [`PointerSanitizer`], the per-pointer
/// [`WindowPointerState`] cache, and the per-pointer arena-hold
/// timeout timers.
///
/// One instance lives inside every `Window`; access it via
/// `window.gesture_binding()` and `window.gesture_binding_mut()`.
///
/// **Auto-trait posture:** `!Send + !Sync` (transitively via
/// `Rc<RefCell<dyn GestureRecognizer>>` inside the arena).
/// Per-`Window` types are main-thread-only by construction; do
/// **not** wrap a `GestureBinding` in `Arc` — the borrow-check
/// failure points at the `Rc` directly.
///
/// **Recognizer registration seam (S07.5 T3):** call
/// [`Self::register_recognizer`] from the dispatcher's `Down` handler
/// to add a recognizer to the arena. That method drives the
/// [`RecognizerLifecycle`] hooks (per-window settings injection,
/// arena back-channel, arena-hold) so neither `Window::dispatch_event`
/// nor the recognizer impl needs to repeat the wiring boilerplate.
///
/// `#[non_exhaustive]` for forward-compatibility — future
/// per-`Window` gesture state (e.g. an explicit
/// `GestureArenaTeam` registry, an A4-driven `tracing::Span`
/// handle) can be added without a breaking change.
#[non_exhaustive]
pub struct GestureBinding {
    /// Per-window arena manager. Wrapped in `Rc<RefCell<…>>` so
    /// recognizers that opt into [`RecognizerLifecycle::needs_back_channel`]
    /// can hold a `Weak` handle and call back into the arena from
    /// async timer tasks (LongPress) without dangling on
    /// window-tear-down. The dispatch loop inside `Window::dispatch_event`
    /// uses [`Self::arena_take`] / [`Self::arena_restore`] for the
    /// `mem::take` snapshot it relies on for sibling-callback merge.
    arena: Rc<RefCell<GestureArenaManager>>,
    settings: GestureSettings,
    sanitizer: PointerSanitizer,
    pointer_state: WindowPointerState,
    /// Per-pointer release timers for arenas held by recognizers that
    /// opted into [`RecognizerLifecycle::needs_arena_hold`] (DoubleTap).
    /// Dropping a `Task<()>` cancels its underlying future, so:
    /// - successful second-tap acceptance: dispatcher removes the
    ///   timer from the map.
    /// - cancel / removed events: dispatcher drops the timer when it
    ///   calls `arena.cancel(pointer_id)`.
    /// - window teardown: this map drops with the binding, cancelling
    ///   every in-flight timeout.
    arena_hold_timers: collections::FxHashMap<PointerId, Task<()>>,
}

impl Default for GestureBinding {
    fn default() -> Self {
        Self::new()
    }
}

impl GestureBinding {
    /// Construct a new binding with default [`GestureSettings`].
    /// Called by [`Default::default`]; rustc's dead-code analysis
    /// does not always see the indirection on test builds, hence
    /// the explicit allow.
    #[allow(dead_code, reason = "called via the Default impl")]
    pub(crate) fn new() -> Self {
        Self {
            arena: Rc::new(RefCell::new(GestureArenaManager::default())),
            settings: GestureSettings::default(),
            sanitizer: PointerSanitizer,
            pointer_state: WindowPointerState::default(),
            arena_hold_timers: collections::FxHashMap::default(),
        }
    }

    /// Borrow the configured gesture settings. Cheap.
    pub fn settings(&self) -> &GestureSettings {
        &self.settings
    }

    /// Mutate settings. Wired to `window.gesture_settings_mut()`
    /// (the S14 `MediaQuery::gesture_settings` seam).
    pub fn settings_mut(&mut self) -> &mut GestureSettings {
        &mut self.settings
    }

    /// Number of pointers currently competing in any open arena.
    /// Read-only observer for tests and debug rendering.
    pub fn active_pointer_count(&self) -> usize {
        self.arena.borrow().arena_count()
    }

    /// Number of recognizers competing for `pointer_id`'s arena, or
    /// 0 if no arena is open for that pointer.
    pub fn arena_entry_count(&self, pointer_id: PointerId) -> usize {
        self.arena.borrow().entry_count(pointer_id)
    }

    /// Take the arena out of the binding (replacing it with `Default`),
    /// returning the previous contents. The dispatch loop uses this to
    /// snapshot the arena before the dispatch callbacks run, so any
    /// sibling-element registrations performed inside callbacks land
    /// in the live arena alongside the snapshot. Pair with
    /// [`Self::arena_restore`].
    ///
    /// **Borrow contract:** must not be called while another part of
    /// the program holds a borrow on the inner arena (e.g. during a
    /// recognizer-side `borrow_mut` of the arena via the back-channel).
    /// In practice all such borrows are scoped inside this module.
    pub(crate) fn arena_take(&mut self) -> GestureArenaManager {
        std::mem::take(&mut *self.arena.borrow_mut())
    }

    /// Restore an arena previously yielded by [`Self::arena_take`].
    pub(crate) fn arena_restore(&mut self, arena: GestureArenaManager) {
        *self.arena.borrow_mut() = arena;
    }

    /// Read-only access to the inner arena `Rc` — for taking
    /// `Weak` handles in registration code only. External callers
    /// cannot reach this.
    #[allow(
        dead_code,
        reason = "exposed for symmetry with the registration path; \
                  paint-time consumers (T15) will use this when they \
                  start passing pre-built recognizer Rcs through"
    )]
    pub(crate) fn arena_rc(&self) -> &Rc<RefCell<GestureArenaManager>> {
        &self.arena
    }

    /// Single-field sanitizer accessor. Reserved for future hover-listener
    /// dispatch that doesn't need `pointer_state` (the dispatch-loop sites
    /// inside `Window::dispatch_event` use [`Self::dispatch_split_mut`]
    /// instead because they mutate both halves at once).
    #[allow(
        dead_code,
        reason = "exposed as part of the consolidated GestureBinding API; \
                  call sites land with hover-listener wiring (T15 follow-on)"
    )]
    pub(crate) fn sanitizer_mut(&mut self) -> &mut PointerSanitizer {
        &mut self.sanitizer
    }

    pub(crate) fn pointer_state_mut(&mut self) -> &mut WindowPointerState {
        &mut self.pointer_state
    }

    #[allow(
        dead_code,
        reason = "read-only accessor reserved for paint-time / debug rendering"
    )]
    pub(crate) fn pointer_state(&self) -> &WindowPointerState {
        &self.pointer_state
    }

    /// Split-borrow access to the dispatch-time pieces. `Window::dispatch_event`
    /// holds both mutably at once (sanitizer mutates `pointer_state` while it
    /// converts events / diffs hover), so a single accessor avoids re-introducing
    /// the direct `Window::gesture_sanitizer` + `Window::gesture_pointer_state`
    /// fields the S07.5 T2 work consolidated away.
    pub(crate) fn dispatch_split_mut(
        &mut self,
    ) -> (&mut PointerSanitizer, &mut WindowPointerState) {
        (&mut self.sanitizer, &mut self.pointer_state)
    }

    /// Register a recognizer for `pointer_id`'s arena, driving every
    /// [`RecognizerLifecycle`] hook needed to make the recognizer
    /// production-correct end-to-end:
    ///
    /// 1. Apply per-window settings via
    ///    [`RecognizerLifecycle::configure_settings`] so
    ///    `window.gesture_settings_mut()` overrides take effect even
    ///    for recognizers built via the fluent `__internal_on_*`
    ///    helpers (which previously baked in `GestureSettings::default()`
    ///    at construction time inside `render()`).
    /// 2. Inject the per-window arena back-channel via
    ///    [`RecognizerLifecycle::set_arena_back_channel`] for
    ///    recognizers that opt in (LongPress timer-driven acceptance).
    ///    The handle is `Weak` so a window-tear-down race becomes a
    ///    no-op upgrade.
    /// 3. Add the recognizer to the arena.
    ///
    /// Returns `true` iff the recognizer asked the arena to enter
    /// `hold` mode via [`RecognizerLifecycle::needs_arena_hold`]. The
    /// caller (the dispatcher in `Window::dispatch_event`) follows up
    /// with `arena.hold(pointer_id)` plus a release-timer scheduled
    /// via [`Self::schedule_arena_release`].
    ///
    /// **Why this returns `bool` rather than scheduling the timer
    /// itself:** scheduling a release timer needs an `&mut App` (for
    /// `cx.spawn`) plus the `AnyWindowHandle` for the async `update_window`
    /// dance. The dispatcher already has both at the call site; routing
    /// them through `register_recognizer` would force every future
    /// caller (paint-time registration, integration tests) to wire the
    /// same arguments through. Keeping the seam narrow lets registration
    /// stay a pure binding-side operation.
    pub(crate) fn register_recognizer(
        &mut self,
        pointer_id: PointerId,
        recognizer: Rc<RefCell<Box<dyn GestureRecognizer>>>,
    ) -> bool {
        let entry_index = self.arena.borrow().entry_count(pointer_id);
        let mut needs_hold = false;
        {
            let mut rec = recognizer.borrow_mut();
            if let Some(lifecycle) = rec.lifecycle() {
                lifecycle.configure_settings(&self.settings);
                if lifecycle.needs_back_channel() {
                    let back_channel = GestureArenaManager::make_back_channel_from(&self.arena);
                    lifecycle.set_arena_back_channel(back_channel, entry_index);
                }
                needs_hold = lifecycle.needs_arena_hold();
            }
        }
        self.arena.borrow_mut().add(pointer_id, recognizer);
        log::trace!(
            target: "flui::gesture::binding",
            phase = "register",
            entry_index = entry_index,
            arena_state = if needs_hold { "needs_hold" } else { "open" };
            "register_recognizer"
        );
        needs_hold
    }

    /// Schedule a `double_tap_timeout`-deferred `arena.release` for
    /// `pointer_id`. Called by the dispatcher after registering at
    /// least one recognizer that opted into
    /// [`RecognizerLifecycle::needs_arena_hold`].
    ///
    /// The scheduled `Task<()>` is stored in `arena_hold_timers`,
    /// keyed by `pointer_id`. Cancellation paths:
    /// - Successful second-tap acceptance: the dispatcher calls
    ///   [`Self::cancel_arena_hold`] to drop the task.
    /// - `Cancel` / `Removed` events on the held pointer: `arena.cancel`
    ///   resolves the arena, and [`Self::cancel_arena_hold`] drops
    ///   the timer alongside.
    /// - Window teardown: `GestureBinding::Drop` (implicit) drops the
    ///   map and every in-flight task in it.
    ///
    /// If a timer was already pending for this `pointer_id` (e.g. the
    /// dispatcher hits a stray re-Down before the previous arena
    /// resolved), the previous timer is dropped and replaced.
    pub(crate) fn schedule_arena_release(
        &mut self,
        pointer_id: PointerId,
        window_handle: crate::AnyWindowHandle,
        cx: &mut crate::App,
    ) {
        let timeout = self.settings.double_tap_timeout;
        let arena_weak = Rc::downgrade(&self.arena);
        log::debug!(
            target: "flui::gesture::binding",
            phase = "hold_schedule",
            arena_state = "held",
            lifecycle = "schedule_release",
            timeout_ms = timeout.as_millis() as u64;
            "scheduling arena release timer"
        );
        // Use `BackgroundExecutor::timer` (not `smol::Timer::after`) so
        // the test-scheduler's virtual clock — driven by
        // `TestAppContext::executor().advance_clock` — wakes the timer
        // deterministically. Mirrors the LongPress timer pattern.
        let timer_future = cx.background_executor().timer(timeout);
        let task = cx.spawn(async move |async_cx| {
            timer_future.await;
            let Some(arena_rc) = arena_weak.upgrade() else {
                log::trace!(
                    target: "flui::gesture::binding",
                    phase = "hold_release",
                    lifecycle = "cancel";
                    "release timer fired after window teardown"
                );
                return;
            };
            let _ = async_cx.update_window(window_handle, |_, window, cx| {
                log::debug!(
                    target: "flui::gesture::binding",
                    phase = "hold_release",
                    lifecycle = "release",
                    arena_state = "released";
                    "release timer fired; calling arena.release"
                );
                arena_rc.borrow_mut().release(pointer_id, window, cx);
            });
        });
        self.arena_hold_timers.insert(pointer_id, task);
    }

    /// Drop the arena-hold timer for `pointer_id` if one is pending.
    /// Used by the dispatcher when a held arena resolves (successful
    /// second tap, or arena cancel) so the timer's future is cancelled.
    pub(crate) fn cancel_arena_hold(&mut self, pointer_id: PointerId) -> bool {
        self.arena_hold_timers.remove(&pointer_id).is_some()
    }
}
