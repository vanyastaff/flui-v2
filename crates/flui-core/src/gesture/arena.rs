//! `GestureArenaManager`, `GestureArena`, `GestureArenaEntry`,
//! `GestureDisposition`. The competition arbitrator.
//!
//! All three arena types (`GestureArena`, `GestureArenaEntry`,
//! `GestureArenaManager`) are `pub(crate)` — they have no public
//! method surface. Consumers reach the manager via `pub(crate)`
//! accessors on `GestureBinding`. [`GestureDisposition`] is `pub`
//! because [`super::GestureRecognizer::handle_event`] returns it.
//!
//! Auto-trait posture: `!Send + !Sync` due to
//! `Rc<RefCell<dyn GestureRecognizer>>` in entries.
//!
//! See the design doc § "GestureArena and GestureArenaManager".

use super::{DeliveredEvent, GestureRecognizer, PointerEvent, PointerId};
use smallvec::SmallVec;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// The disposition returned by [`GestureRecognizer::handle_event`]
/// and recorded by the arena manager.
///
/// `#[non_exhaustive]` to admit future dispositions (e.g. `Hold` for
/// gesture-yield semantics) without breaking changes.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GestureDisposition {
    /// "I want this gesture sequence; declare me the winner now."
    /// All other recognizers in the arena are notified `rejected`.
    Accepted,
    /// "I cannot win this gesture sequence; remove me from the
    /// arena." Other recognizers continue competing.
    Rejected,
    /// "I might still win — keep me in the arena."
    Possible,
}

/// One competitor entry in a [`GestureArena`].
///
/// Holds an `Rc<RefCell<dyn GestureRecognizer>>` because recognizers
/// self-mutate from inside the arena callback chain (eager-accept may
/// run user code that mutates the recognizer state).
///
/// A7-audit: the `Rc<RefCell<…>>` is bounded to the gesture-subsystem
/// internals. Public surface (`Interactivity::on_tap` and friends)
/// takes `Box<dyn GestureRecognizer>` and the arena promotes to
/// `Rc<RefCell<…>>` internally.
pub(crate) struct GestureArenaEntry {
    pub(crate) recognizer: Rc<RefCell<Box<dyn GestureRecognizer>>>,
}

/// One arena per active pointer. `entries` is registration order; the
/// captain is `entries[0]` (sweep on `Up` declares the first
/// registered the winner if no recognizer eagerly accepted).
#[derive(Default)]
pub(crate) struct GestureArena {
    pub(crate) entries: SmallVec<[GestureArenaEntry; 4]>,
    /// Index into `entries` of the recognizer that won, if any.
    pub(crate) winner: Option<usize>,
    /// `false` once the arena has resolved (sweep run, all rejected
    /// notified) — subsequent dispatches are no-ops.
    pub(crate) is_open: bool,
    /// Number of recognizers currently holding the arena open past
    /// `Up` (e.g. `DoubleTap` waiting for a second tap, future
    /// MultiTap waiting for additional taps). The arena is considered
    /// **held** iff `hold_count > 0`; sweep is gated on
    /// `hold_count == 0`. A `u32` counter (rather than a `bool`) is
    /// required because two recognizers on the same pointer may
    /// independently want hold semantics (e.g. DoubleTap + MultiTap)
    /// — under a boolean, one's `release` would silently clear the
    /// other's hold.
    pub(crate) hold_count: u32,
}

impl GestureArena {
    /// Open a new arena. Used by `GestureArenaManager::add`, called
    /// by `GestureBinding::register_recognizer` from the dispatcher's
    /// `Down` handler.
    fn new() -> Self {
        Self {
            entries: SmallVec::new(),
            winner: None,
            is_open: true,
            hold_count: 0,
        }
    }
}

/// Opaque, `Clone`-able handle a recognizer can hold to call back
/// into the per-window arena from outside `handle_event` (e.g. from
/// an async timer task). The handle is internally `Weak`, so:
///
/// - Window teardown drops the underlying arena `Rc`, and every
///   subsequent [`Self::declare_winner`] / [`Self::release`] call
///   returns silently without dispatching anything.
/// - Recognizer code never sees the private `GestureArenaManager`
///   type; the public surface is exactly the methods on this struct.
///
/// `#[non_exhaustive]` for forward-compatibility (additional
/// fire-and-forget arena operations may land in future specs).
#[derive(Clone)]
#[non_exhaustive]
pub struct ArenaBackChannel {
    /// Weak pointer back to the per-window `GestureArenaManager`.
    inner: Weak<RefCell<GestureArenaManager>>,
}

impl ArenaBackChannel {
    /// Construct an empty back-channel. Convenient for `Default`-style
    /// initialization in recognizer fields; calling any method on a
    /// `default()` instance returns silently.
    ///
    /// Production code receives the populated handle via
    /// [`super::recognizer::RecognizerLifecycle::set_arena_back_channel`].
    pub fn empty() -> Self {
        Self { inner: Weak::new() }
    }

    /// Asynchronous winner declaration. Mirrors
    /// `GestureArenaManager::declare_winner`: if the per-window arena
    /// is still alive and the entry index is in range, the named
    /// recognizer wins, every other entry on this pointer is notified
    /// `rejected`, and the arena is closed (unless held).
    ///
    /// Silent no-op if the arena has already been dropped (window
    /// teardown).
    pub fn declare_winner(
        &self,
        pointer_id: PointerId,
        entry_index: usize,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) {
        if let Some(rc) = self.inner.upgrade() {
            rc.borrow_mut()
                .declare_winner(pointer_id, entry_index, window, cx);
        }
    }

    /// Internal accessor for `GestureBinding` to look up the arena
    /// `Rc` and reach `entries[entry_index]` for `Rc::clone`. Hidden
    /// from downstream because cloning the recognizer `Rc` is an
    /// implementation detail of the timer/back-channel wiring.
    pub(crate) fn upgrade(&self) -> Option<Rc<RefCell<GestureArenaManager>>> {
        self.inner.upgrade()
    }
}

impl Default for ArenaBackChannel {
    fn default() -> Self {
        Self::empty()
    }
}

impl std::fmt::Debug for ArenaBackChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArenaBackChannel")
            .field("alive", &(self.inner.strong_count() > 0))
            .finish()
    }
}

/// One arena manager per [`super::GestureBinding`] per `Window`.
#[derive(Default)]
pub(crate) struct GestureArenaManager {
    /// Per-pointer arenas. The pair `(PointerId, GestureArena)`
    /// matches Flutter's `_GestureArena` map; we use a `SmallVec`
    /// because typical pointer counts are 1–2 on desktop, ≤ 4 on
    /// multi-touch.
    pub(crate) arenas: SmallVec<[(PointerId, GestureArena); 4]>,
}

impl GestureArenaManager {
    /// Number of pointers currently competing.
    pub(crate) fn arena_count(&self) -> usize {
        self.arenas.len()
    }

    /// Number of recognizers in `pointer_id`'s arena, or 0.
    pub(crate) fn entry_count(&self, pointer_id: PointerId) -> usize {
        self.arenas
            .iter()
            .find(|(id, _)| *id == pointer_id)
            .map(|(_, a)| a.entries.len())
            .unwrap_or(0)
    }

    /// Open an arena for `pointer_id` if none exists; insert
    /// `recognizer` at the back of the entries list (registration
    /// order).
    ///
    /// Production caller is `GestureBinding::register_recognizer`,
    /// invoked by `Window::dispatch_event` on the `Down` phase.
    pub(crate) fn add(
        &mut self,
        pointer_id: PointerId,
        recognizer: Rc<RefCell<Box<dyn GestureRecognizer>>>,
    ) {
        if let Some((_, arena)) = self.arenas.iter_mut().find(|(id, _)| *id == pointer_id) {
            arena.entries.push(GestureArenaEntry { recognizer });
        } else {
            let mut arena = GestureArena::new();
            arena.entries.push(GestureArenaEntry { recognizer });
            self.arenas.push((pointer_id, arena));
        }
    }

    /// Dispatch an event to all entries in `pointer_id`'s arena. If
    /// any returns `Accepted`, declare it winner and notify the rest
    /// `rejected`. If any returns `Rejected`, drop it.
    pub(crate) fn dispatch(
        &mut self,
        pointer_id: PointerId,
        event: &PointerEvent,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) {
        let Some((_, arena)) = self.arenas.iter_mut().find(|(id, _)| *id == pointer_id) else {
            return;
        };
        if !arena.is_open {
            return;
        }

        // Iterate over a snapshot of recognizers so that callback
        // mutations to sibling recognizer registries do not corrupt
        // the iteration.
        let snapshot: SmallVec<[Rc<RefCell<Box<dyn GestureRecognizer>>>; 4]> = arena
            .entries
            .iter()
            .map(|e| Rc::clone(&e.recognizer))
            .collect();

        let mut accepted_index: Option<usize> = None;
        let mut to_drop: SmallVec<[usize; 4]> = SmallVec::new();
        for (idx, recognizer) in snapshot.iter().enumerate() {
            let disposition = {
                let mut r = recognizer.borrow_mut();
                // S07.5b: arena does not yet track per-entry hit-test
                // transforms, so every recognizer sees the window-
                // local position as its local position. S09 will
                // store a per-entry inverse-transform alongside the
                // recognizer and compute a real `local_position`
                // here.
                r.handle_event(DeliveredEvent::at_event_position(event), window, cx)
            };
            match disposition {
                GestureDisposition::Accepted => {
                    accepted_index = Some(idx);
                    break;
                }
                GestureDisposition::Rejected => {
                    to_drop.push(idx);
                }
                GestureDisposition::Possible => {}
            }
        }

        if let Some(winner_idx) = accepted_index {
            // Re-borrow the arena after the callbacks ran (sibling
            // mutations may have changed it).
            let Some((_, arena)) = self.arenas.iter_mut().find(|(id, _)| *id == pointer_id) else {
                return;
            };
            arena.winner = Some(winner_idx);
            // Reject everyone else.
            let losers: SmallVec<[Rc<RefCell<Box<dyn GestureRecognizer>>>; 4]> = arena
                .entries
                .iter()
                .enumerate()
                .filter_map(|(i, e)| {
                    if i == winner_idx {
                        None
                    } else {
                        Some(Rc::clone(&e.recognizer))
                    }
                })
                .collect();
            for loser in losers.iter() {
                loser.borrow_mut().rejected(pointer_id, window, cx);
            }
            // Close the arena unless a recognizer is holding it.
            if arena.hold_count == 0 {
                arena.is_open = false;
            }
        } else {
            // Drop rejected entries (in reverse so indices stay valid).
            let Some((_, arena)) = self.arenas.iter_mut().find(|(id, _)| *id == pointer_id) else {
                return;
            };
            for &idx in to_drop.iter().rev() {
                if idx < arena.entries.len() {
                    arena.entries.remove(idx);
                }
            }
        }

        self.gc(pointer_id);
    }

    /// Sweep — called by the dispatcher on `Up`. If no winner has
    /// been declared and the arena is not held, declare the first
    /// remaining entry the winner via `sweep_accepted`. Then close
    /// the arena.
    pub(crate) fn sweep(
        &mut self,
        pointer_id: PointerId,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) {
        let Some((_, arena)) = self.arenas.iter_mut().find(|(id, _)| *id == pointer_id) else {
            return;
        };
        if arena.hold_count > 0 || arena.winner.is_some() || !arena.is_open {
            return;
        }

        let captain = arena.entries.first().map(|e| Rc::clone(&e.recognizer));
        let losers: SmallVec<[Rc<RefCell<Box<dyn GestureRecognizer>>>; 4]> = arena
            .entries
            .iter()
            .skip(1)
            .map(|e| Rc::clone(&e.recognizer))
            .collect();
        arena.winner = Some(0);
        arena.is_open = false;

        if let Some(c) = captain {
            c.borrow_mut().sweep_accepted(pointer_id, window, cx);
        }
        for loser in losers.iter() {
            loser.borrow_mut().rejected(pointer_id, window, cx);
        }

        self.gc(pointer_id);
    }

    /// Hold semantics — keep the arena open past `Up` until the
    /// caller calls [`Self::release`]. Used by recognizers like
    /// `DoubleTap` that span multiple Down/Up sequences.
    ///
    /// Production caller is `Window::dispatch_event`'s `Down` handler,
    /// invoked when a registered recognizer opted into
    /// [`crate::gesture::recognizer::RecognizerLifecycle::needs_arena_hold`]
    /// at registration time.
    pub(crate) fn hold(&mut self, pointer_id: PointerId) {
        if let Some((_, arena)) = self.arenas.iter_mut().find(|(id, _)| *id == pointer_id) {
            arena.hold_count = arena.hold_count.saturating_add(1);
        }
    }

    /// Release a held arena; resumes normal sweep semantics on the
    /// next `Up` (or runs sweep directly when the arena has no winner
    /// yet).
    ///
    /// Production caller is the per-pointer release task scheduled
    /// from `GestureBinding::schedule_arena_release` after the
    /// recognizer that initiated the hold (DoubleTap) does not see
    /// the second tap within `double_tap_timeout`.
    pub(crate) fn release(
        &mut self,
        pointer_id: PointerId,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) {
        let Some((_, arena)) = self.arenas.iter_mut().find(|(id, _)| *id == pointer_id) else {
            return;
        };
        // `release` without a matching `hold` is a real logic error
        // — either the dispatcher schedule_arena_release timer fired
        // twice, or a recognizer's needs_arena_hold contract was
        // violated. Catch it loudly in dev builds via debug_assert,
        // and degrade to saturating-sub in release builds so a stale
        // timer cannot underflow `hold_count` and trip the
        // `hold_count == 0` sweep gate elsewhere.
        debug_assert!(
            arena.hold_count > 0,
            "GestureArenaManager::release called without a matching hold (pointer_id={:?})",
            pointer_id,
        );
        arena.hold_count = arena.hold_count.saturating_sub(1);
        // Sweep only when nobody else is holding the arena.
        let needs_sweep =
            arena.hold_count == 0 && arena.winner.is_none() && arena.is_open;
        if needs_sweep {
            // Drop the borrow before calling `sweep` (which re-borrows).
            self.sweep(pointer_id, window, cx);
        }
    }

    /// Forcefully close the arena and notify every remaining entry
    /// `rejected`. Called by the sanitizer on `Cancel`.
    pub(crate) fn cancel(
        &mut self,
        pointer_id: PointerId,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) {
        let Some((_, arena)) = self.arenas.iter_mut().find(|(id, _)| *id == pointer_id) else {
            return;
        };
        let entries: SmallVec<[Rc<RefCell<Box<dyn GestureRecognizer>>>; 4]> = arena
            .entries
            .iter()
            .map(|e| Rc::clone(&e.recognizer))
            .collect();
        arena.winner = None;
        arena.is_open = false;
        arena.hold_count = 0;
        for r in entries.iter() {
            r.borrow_mut().rejected(pointer_id, window, cx);
        }
        self.gc(pointer_id);
    }

    /// Async back-channel for recognizers whose `Accepted` decision
    /// fires from outside `handle_event` (e.g.
    /// `LongPressGestureRecognizer`'s timer).
    ///
    /// Production caller is the LongPress timer task, which upgrades
    /// the per-pointer `Weak<RefCell<GestureArenaManager>>` injected
    /// at registration time and calls this method through the
    /// resulting `Rc<RefCell<…>>` borrow.
    pub(crate) fn declare_winner(
        &mut self,
        pointer_id: PointerId,
        recognizer_index: usize,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) {
        let Some((_, arena)) = self.arenas.iter_mut().find(|(id, _)| *id == pointer_id) else {
            return;
        };
        if !arena.is_open || arena.winner.is_some() || recognizer_index >= arena.entries.len() {
            return;
        }
        arena.winner = Some(recognizer_index);
        let losers: SmallVec<[Rc<RefCell<Box<dyn GestureRecognizer>>>; 4]> = arena
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                if i == recognizer_index {
                    None
                } else {
                    Some(Rc::clone(&e.recognizer))
                }
            })
            .collect();
        if arena.hold_count == 0 {
            arena.is_open = false;
        }
        for loser in losers.iter() {
            loser.borrow_mut().rejected(pointer_id, window, cx);
        }
        self.gc(pointer_id);
    }

    /// Construct an [`ArenaBackChannel`] handle pointing at this manager.
    /// Used by `GestureBinding::register_recognizer` to inject a
    /// back-channel into recognizers that opt into
    /// [`super::recognizer::RecognizerLifecycle::needs_back_channel`].
    /// The handle is `Weak`, so a window-tear-down race becomes a
    /// no-op upgrade.
    ///
    /// `manager_rc` MUST be the `Rc` that owns this manager — passing
    /// a fresh `Rc::new(...)` defeats the purpose. In practice the
    /// only caller is `GestureBinding`, which stores its arena inside
    /// an `Rc<RefCell<…>>` for exactly this reason.
    pub(crate) fn make_back_channel_from(
        manager_rc: &Rc<RefCell<GestureArenaManager>>,
    ) -> ArenaBackChannel {
        ArenaBackChannel {
            inner: Rc::downgrade(manager_rc),
        }
    }

    /// Merge `other` into `self`, deduplicating by `PointerId`.
    ///
    /// Used by `Window::dispatch_event` to fold the live arena (which
    /// may have grown new entries from sibling-element callbacks
    /// running inside the dispatch loop) back into the snapshot we
    /// took before dispatching. A naive `arenas.extend(other.arenas)`
    /// produces a second `(PointerId, GestureArena)` row for every
    /// callback-time registration on the same pointer; this method
    /// keeps the invariant "exactly one arena per active pointer"
    /// across the snapshot/dispatch/restore round-trip (S07.5 T7).
    ///
    /// **Collision policy** for the per-pointer scalar fields:
    /// - `entries`: append (incoming entries land at the back,
    ///   registration-order semantics preserved).
    /// - `winner`: incoming wins if `Some` (a callback that declared
    ///   a winner mid-dispatch wins over the outer pre-take state);
    ///   otherwise keep the existing.
    /// - `is_open`: `existing.is_open && incoming.is_open` (closing
    ///   on either side closes the merged arena — the more
    ///   conservative choice).
    /// - `hold_count`: `existing.hold_count + incoming.hold_count`
    ///   (counters compose additively; saturating-add against
    ///   `u32::MAX` overflow as a defensive measure for pathological
    ///   merges).
    pub(crate) fn merge_by_pointer_id(&mut self, mut other: GestureArenaManager) {
        for (pid, incoming) in other.arenas.drain(..) {
            if let Some((_, existing)) = self.arenas.iter_mut().find(|(id, _)| *id == pid) {
                existing.entries.extend(incoming.entries);
                if incoming.winner.is_some() {
                    existing.winner = incoming.winner;
                }
                existing.is_open = existing.is_open && incoming.is_open;
                existing.hold_count = existing.hold_count.saturating_add(incoming.hold_count);
            } else {
                self.arenas.push((pid, incoming));
            }
        }
    }

    /// Garbage-collect resolved arenas.
    ///
    /// An arena is removed when it is **closed** (`is_open == false`)
    /// and **not held** (`hold_count == 0`). The previous version
    /// only removed empty closed arenas, so a closed-but-non-empty
    /// arena (the normal post-resolution shape — winners and losers
    /// stay in `entries` until something explicit clears them, plus
    /// the post-`cancel` shape that keeps entries while
    /// `winner == None`) leaked into
    /// `GestureBinding::active_pointer_count` forever (Copilot
    /// review E).
    ///
    /// Held arenas (`hold_count > 0`) stay alive past `Up` for
    /// `DoubleTap`-style multi-Down sequences and only resolve via
    /// [`Self::release`].
    fn gc(&mut self, pointer_id: PointerId) {
        if let Some(idx) = self
            .arenas
            .iter()
            .position(|(id, a)| *id == pointer_id && !a.is_open && a.hold_count == 0)
        {
            self.arenas.remove(idx);
        }
    }
}

#[cfg(test)]
mod tests {
    //! T16 — Arena lifecycle tests.
    //!
    //! Each test constructs a real `TestAppContext` + `Window`,
    //! wires `MockRecognizer`s into a `GestureArenaManager`, and
    //! verifies state transitions via the recorded `handle_calls`,
    //! `sweep_calls`, and `rejected_calls` vectors on each mock.

    use super::*;
    use crate::gesture::{PointerButtons, PointerEvent, PointerId, PointerKind, PointerPhase};
    use crate::scheduler::Instant;
    use crate::{self as flui_core, AppContext as _, Modifiers, Point, TestAppContext};

    /// A scriptable mock recognizer. Pops dispositions off
    /// `script` per `handle_event` call; falls back to `Possible`.
    /// Records every call to `handle_event`, `sweep_accepted`,
    /// `rejected` for assertion.
    struct MockRecognizer {
        name: &'static str,
        script: std::collections::VecDeque<GestureDisposition>,
        handle_calls: Vec<(PointerId, PointerPhase)>,
        sweep_calls: Vec<PointerId>,
        rejected_calls: Vec<PointerId>,
    }

    impl MockRecognizer {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                script: Default::default(),
                handle_calls: Vec::new(),
                sweep_calls: Vec::new(),
                rejected_calls: Vec::new(),
            }
        }

        fn with_script(name: &'static str, script: &[GestureDisposition]) -> Self {
            let mut r = Self::new(name);
            r.script = script.iter().copied().collect();
            r
        }
    }

    impl GestureRecognizer for MockRecognizer {
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
        fn name(&self) -> &'static str {
            self.name
        }
        fn add_pointer(&mut self, _: PointerId, _: DeliveredEvent<'_>) {}
        fn handle_event(
            &mut self,
            event: DeliveredEvent<'_>,
            _: &mut crate::Window,
            _: &mut crate::App,
        ) -> GestureDisposition {
            self.handle_calls
                .push((event.event.pointer_id, event.event.phase));
            self.script
                .pop_front()
                .unwrap_or(GestureDisposition::Possible)
        }
        fn sweep_accepted(
            &mut self,
            pointer_id: PointerId,
            _: &mut crate::Window,
            _: &mut crate::App,
        ) {
            self.sweep_calls.push(pointer_id);
        }
        fn rejected(&mut self, pointer_id: PointerId, _: &mut crate::Window, _: &mut crate::App) {
            self.rejected_calls.push(pointer_id);
        }
    }

    fn pointer_event(phase: PointerPhase) -> PointerEvent {
        let now = Instant::now();
        PointerEvent {
            pointer_id: PointerId(0),
            kind: PointerKind::Mouse,
            phase,
            position: Point::default(),
            delta: Point::default(),
            buttons: PointerButtons::PRIMARY,
            modifiers: Modifiers::default(),
            timestamp: now,
            source_timestamp: now,
            provenance: crate::gesture::PointerEventProvenance::Platform,
            pressure: None,
            tilt: 0.0,
            orientation: 0.0,
        }
    }

    /// Wrap a `MockRecognizer` in the `Rc<RefCell<Box<...>>>` shape
    /// the arena expects. Returns the wrapped entry plus a shared
    /// handle for post-dispatch assertion.
    fn boxed_mock(m: MockRecognizer) -> Rc<RefCell<Box<dyn GestureRecognizer>>> {
        Rc::new(RefCell::new(Box::new(m) as Box<dyn GestureRecognizer>))
    }

    /// Borrow the mock back out of the arena entry for assertion.
    fn with_mock<R>(
        entry: &Rc<RefCell<Box<dyn GestureRecognizer>>>,
        f: impl FnOnce(&MockRecognizer) -> R,
    ) -> R {
        let mut e = entry.borrow_mut();
        let m = e
            .as_any_mut()
            .downcast_mut::<MockRecognizer>()
            .expect("entry is a MockRecognizer");
        f(m)
    }

    #[flui_core::test]
    fn arena_eager_accept_short_circuits_others(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let mut arena = GestureArenaManager::default();
                    let p = PointerId(0);
                    let r0 = boxed_mock(MockRecognizer::with_script(
                        "r0",
                        &[GestureDisposition::Accepted],
                    ));
                    let r1 = boxed_mock(MockRecognizer::new("r1"));
                    arena.add(p, Rc::clone(&r0));
                    arena.add(p, Rc::clone(&r1));
                    let evt = pointer_event(PointerPhase::Down);
                    arena.dispatch(p, &evt, window, cx);
                    with_mock(&r0, |m| {
                        assert_eq!(m.handle_calls.len(), 1, "winner saw the event");
                        assert!(m.rejected_calls.is_empty(), "winner not rejected");
                    });
                    with_mock(&r1, |m| {
                        // Loser may or may not have seen the event before
                        // r0 accepted (depends on iteration order).
                        // `dispatch` snapshots in registration order, so
                        // r0 (idx 0) runs first → r1 doesn't see it.
                        assert_eq!(m.rejected_calls, vec![p], "loser notified rejected");
                    });
                });
        });
    }

    #[flui_core::test]
    fn arena_sweep_first_registered_wins_on_up(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let mut arena = GestureArenaManager::default();
                    let p = PointerId(0);
                    let r0 = boxed_mock(MockRecognizer::new("r0"));
                    let r1 = boxed_mock(MockRecognizer::new("r1"));
                    arena.add(p, Rc::clone(&r0));
                    arena.add(p, Rc::clone(&r1));
                    arena.sweep(p, window, cx);
                    with_mock(&r0, |m| {
                        assert_eq!(m.sweep_calls, vec![p], "first registered swept");
                        assert!(m.rejected_calls.is_empty());
                    });
                    with_mock(&r1, |m| {
                        assert!(m.sweep_calls.is_empty());
                        assert_eq!(m.rejected_calls, vec![p], "loser rejected");
                    });
                });
        });
    }

    #[flui_core::test]
    fn arena_cancel_rejects_all_entries(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let mut arena = GestureArenaManager::default();
                    let p = PointerId(0);
                    let r0 = boxed_mock(MockRecognizer::new("r0"));
                    let r1 = boxed_mock(MockRecognizer::new("r1"));
                    arena.add(p, Rc::clone(&r0));
                    arena.add(p, Rc::clone(&r1));
                    arena.cancel(p, window, cx);
                    with_mock(&r0, |m| {
                        assert_eq!(m.rejected_calls, vec![p]);
                        assert!(m.sweep_calls.is_empty());
                    });
                    with_mock(&r1, |m| {
                        assert_eq!(m.rejected_calls, vec![p]);
                        assert!(m.sweep_calls.is_empty());
                    });
                });
        });
    }

    #[flui_core::test]
    fn arena_rejected_disposition_drops_entry(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let mut arena = GestureArenaManager::default();
                    let p = PointerId(0);
                    let r0 = boxed_mock(MockRecognizer::with_script(
                        "r0",
                        &[GestureDisposition::Rejected],
                    ));
                    let r1 = boxed_mock(MockRecognizer::new("r1"));
                    arena.add(p, Rc::clone(&r0));
                    arena.add(p, Rc::clone(&r1));
                    let evt = pointer_event(PointerPhase::Move);
                    arena.dispatch(p, &evt, window, cx);
                    assert_eq!(arena.entry_count(p), 1, "rejected entry dropped from arena");
                });
        });
    }

    #[flui_core::test]
    fn arena_hold_blocks_sweep(cx: &mut TestAppContext) {
        let _ = cx.update(|cx| {
            let _ = cx
                .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                .unwrap()
                .update(cx, |_, window, cx| {
                    let mut arena = GestureArenaManager::default();
                    let p = PointerId(0);
                    let r0 = boxed_mock(MockRecognizer::new("r0"));
                    arena.add(p, Rc::clone(&r0));
                    arena.hold(p);
                    arena.sweep(p, window, cx); // no-op while held
                    with_mock(&r0, |m| {
                        assert!(m.sweep_calls.is_empty(), "held arena did not sweep");
                    });
                    arena.release(p, window, cx); // release runs deferred sweep
                    with_mock(&r0, |m| {
                        assert_eq!(m.sweep_calls, vec![p], "released arena swept");
                    });
                });
        });
    }

    // =================================================================
    // T23 — Property-based tests over the arena state machine.
    //
    // Each property samples its strategy via `proptest::TestRunner`
    // inside a single `#[flui_core::test]` so the recognizers and
    // arena live in the same `TestAppContext` and `Window`. This is
    // the workspace-compatible substitute for the `flui_core::
    // property_test` macro: that macro forwards to
    // `proptest::property_test`, which is not available in
    // `proptest = "1"` without an extra crate (`test-strategy` /
    // `proptest-attr-macro`). Sampling 32 cases per property keeps
    // CI runtime modest while still exercising the bounded input
    // space.
    // =================================================================

    use proptest::test_runner::{Config as ProptestConfig, TestRunner};

    fn proptest_runner() -> TestRunner {
        TestRunner::new(ProptestConfig {
            cases: 32,
            ..ProptestConfig::default()
        })
    }

    /// **P1** — `cancel(p)` notifies every entry exactly once with
    /// `rejected`, regardless of how many recognizers were registered.
    #[flui_core::test]
    fn prop_arena_cancel_rejects_all_entries(cx: &mut TestAppContext) {
        let mut runner = proptest_runner();
        runner
            .run(&(1usize..10), |num_entries| {
                cx.update(|cx| {
                    let _ = cx
                        .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                        .unwrap()
                        .update(cx, |_, window, cx| {
                            let mut arena = GestureArenaManager::default();
                            let p = PointerId(0);
                            let mocks: Vec<_> = (0..num_entries)
                                .map(|_| boxed_mock(MockRecognizer::new("r")))
                                .collect();
                            for m in mocks.iter() {
                                arena.add(p, Rc::clone(m));
                            }
                            arena.cancel(p, window, cx);
                            for m in mocks.iter() {
                                with_mock(m, |mock| {
                                    assert_eq!(
                                        mock.rejected_calls,
                                        vec![p],
                                        "every entry rejected exactly once"
                                    );
                                    assert!(mock.sweep_calls.is_empty());
                                });
                            }
                        });
                });
                Ok(())
            })
            .expect("P1 held for all sampled cases");
    }

    /// **P2** — eager accept by recognizer at index `accept_idx`
    /// declares it the winner; every other entry receives `rejected`
    /// exactly once.
    #[flui_core::test]
    fn prop_eager_accept_rejects_all_others(cx: &mut TestAppContext) {
        let mut runner = proptest_runner();
        runner
            .run(&(2usize..8, 0usize..8), |(num_entries, accept_pos)| {
                let accept_idx = accept_pos % num_entries;
                cx.update(|cx| {
                    let _ = cx
                        .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                        .unwrap()
                        .update(cx, |_, window, cx| {
                            let mut arena = GestureArenaManager::default();
                            let p = PointerId(0);
                            let mocks: Vec<_> = (0..num_entries)
                                .map(|i| {
                                    let script = if i == accept_idx {
                                        vec![GestureDisposition::Accepted]
                                    } else {
                                        vec![]
                                    };
                                    boxed_mock(MockRecognizer::with_script("r", &script))
                                })
                                .collect();
                            for m in mocks.iter() {
                                arena.add(p, Rc::clone(m));
                            }
                            let evt = pointer_event(PointerPhase::Down);
                            arena.dispatch(p, &evt, window, cx);
                            for (i, m) in mocks.iter().enumerate() {
                                with_mock(m, |mock| {
                                    if i == accept_idx {
                                        assert!(
                                            mock.rejected_calls.is_empty(),
                                            "winner is not rejected"
                                        );
                                    } else {
                                        assert_eq!(
                                            mock.rejected_calls,
                                            vec![p],
                                            "loser i={} rejected exactly once",
                                            i
                                        );
                                    }
                                });
                            }
                        });
                });
                Ok(())
            })
            .expect("P2 held for all sampled cases");
    }

    /// **P3** — recognizers that return `Rejected` from
    /// `handle_event` are dropped from the arena's entry list.
    #[flui_core::test]
    fn prop_rejected_disposition_drops_entry(cx: &mut TestAppContext) {
        let mut runner = proptest_runner();
        runner
            .run(&(2usize..8, 0usize..8), |(num_entries, reject_pos)| {
                let reject_idx = reject_pos % num_entries;
                cx.update(|cx| {
                    let _ = cx
                        .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                        .unwrap()
                        .update(cx, |_, window, cx| {
                            let mut arena = GestureArenaManager::default();
                            let p = PointerId(0);
                            let mocks: Vec<_> = (0..num_entries)
                                .map(|i| {
                                    let script = if i == reject_idx {
                                        vec![GestureDisposition::Rejected]
                                    } else {
                                        vec![]
                                    };
                                    boxed_mock(MockRecognizer::with_script("r", &script))
                                })
                                .collect();
                            for m in mocks.iter() {
                                arena.add(p, Rc::clone(m));
                            }
                            let evt = pointer_event(PointerPhase::Move);
                            arena.dispatch(p, &evt, window, cx);
                            assert_eq!(
                                arena.entry_count(p),
                                num_entries - 1,
                                "exactly one entry dropped after Rejected"
                            );
                        });
                });
                Ok(())
            })
            .expect("P3 held for all sampled cases");
    }

    /// **P4** — `sweep` declares the first-registered (index 0) entry
    /// the winner; every other entry receives exactly one `rejected`.
    #[flui_core::test]
    fn prop_sweep_first_registered_wins(cx: &mut TestAppContext) {
        let mut runner = proptest_runner();
        runner
            .run(&(1usize..10), |num_entries| {
                cx.update(|cx| {
                    let _ = cx
                        .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                        .unwrap()
                        .update(cx, |_, window, cx| {
                            let mut arena = GestureArenaManager::default();
                            let p = PointerId(0);
                            let mocks: Vec<_> = (0..num_entries)
                                .map(|_| boxed_mock(MockRecognizer::new("r")))
                                .collect();
                            for m in mocks.iter() {
                                arena.add(p, Rc::clone(m));
                            }
                            arena.sweep(p, window, cx);
                            for (i, m) in mocks.iter().enumerate() {
                                with_mock(m, |mock| {
                                    if i == 0 {
                                        assert_eq!(
                                            mock.sweep_calls,
                                            vec![p],
                                            "captain sweep_accepted"
                                        );
                                        assert!(mock.rejected_calls.is_empty());
                                    } else {
                                        assert!(mock.sweep_calls.is_empty());
                                        assert_eq!(
                                            mock.rejected_calls,
                                            vec![p],
                                            "loser i={} rejected",
                                            i
                                        );
                                    }
                                });
                            }
                        });
                });
                Ok(())
            })
            .expect("P4 held for all sampled cases");
    }

    /// **P5** — `hold` blocks `sweep`; `release` runs the deferred
    /// sweep, declaring the captain.
    #[flui_core::test]
    fn prop_hold_blocks_sweep_until_release(cx: &mut TestAppContext) {
        let mut runner = proptest_runner();
        runner
            .run(&(1usize..6), |num_entries| {
                cx.update(|cx| {
                    let _ = cx
                        .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                        .unwrap()
                        .update(cx, |_, window, cx| {
                            let mut arena = GestureArenaManager::default();
                            let p = PointerId(0);
                            let mocks: Vec<_> = (0..num_entries)
                                .map(|_| boxed_mock(MockRecognizer::new("r")))
                                .collect();
                            for m in mocks.iter() {
                                arena.add(p, Rc::clone(m));
                            }
                            arena.hold(p);
                            arena.sweep(p, window, cx);
                            for m in mocks.iter() {
                                with_mock(m, |mock| {
                                    assert!(
                                        mock.sweep_calls.is_empty(),
                                        "held arena does not sweep"
                                    );
                                    assert!(mock.rejected_calls.is_empty());
                                });
                            }
                            arena.release(p, window, cx);
                            for (i, m) in mocks.iter().enumerate() {
                                with_mock(m, |mock| {
                                    if i == 0 {
                                        assert_eq!(mock.sweep_calls, vec![p]);
                                    } else {
                                        assert_eq!(
                                            mock.rejected_calls,
                                            vec![p],
                                            "loser i={} rejected after release",
                                            i
                                        );
                                    }
                                });
                            }
                        });
                });
                Ok(())
            })
            .expect("P5 held for all sampled cases");
    }

    // ============================================================
    // S07.5 T11 — Arena lifecycle property locks.
    // ============================================================

    /// **P-T15.5-A** — `merge_by_pointer_id` never produces duplicate
    /// `PointerId` entries in the merged manager. Generates a random
    /// pair of arenas (each with up to 4 distinct pointers, up to 4
    /// entries per pointer), merges them, and asserts every
    /// `PointerId` appears at most once in the result.
    #[flui_core::test]
    fn prop_merge_by_pointer_id_no_duplicates(cx: &mut TestAppContext) {
        use proptest::collection::vec as pvec;
        let mut runner = proptest_runner();
        let strategy = (pvec(0u64..4u64, 0..6), pvec(0u64..4u64, 0..6));
        runner
            .run(&strategy, |(left_ids, right_ids)| {
                cx.update(|cx| {
                    let _ = cx
                        .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                        .unwrap()
                        .update(cx, |_, _window, _cx| {
                            let mut left = GestureArenaManager::default();
                            let mut right = GestureArenaManager::default();
                            for raw in left_ids.iter() {
                                let p = PointerId(*raw);
                                left.add(p, boxed_mock(MockRecognizer::new("l")));
                            }
                            for raw in right_ids.iter() {
                                let p = PointerId(*raw);
                                right.add(p, boxed_mock(MockRecognizer::new("r")));
                            }
                            left.merge_by_pointer_id(right);
                            // Invariant: each PointerId appears at
                            // most once after the merge.
                            let mut seen: std::collections::HashSet<u64> =
                                std::collections::HashSet::new();
                            for (pid, _) in left.arenas.iter() {
                                assert!(
                                    seen.insert(pid.0),
                                    "duplicate PointerId {pid:?} after merge_by_pointer_id"
                                );
                            }
                        });
                });
                Ok(())
            })
            .expect("P-T15.5-A held for all sampled cases");
    }

    /// **P-T15.5-B** — `active_pointer_count` is bounded by the number
    /// of distinct `PointerId`s that have been added (open `add` calls
    /// on the same pointer collapse into one arena entry; only
    /// successful `gc` after sweep / cancel decrements the count).
    /// The invariant: `active_pointer_count() <= distinct_pointers_added`.
    #[flui_core::test]
    fn prop_active_pointer_count_upper_bound(cx: &mut TestAppContext) {
        use proptest::collection::vec as pvec;
        let mut runner = proptest_runner();
        let strategy = pvec(0u64..4u64, 1..16);
        runner
            .run(&strategy, |adds| {
                cx.update(|cx| {
                    let _ = cx
                        .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                        .unwrap()
                        .update(cx, |_, _window, _cx| {
                            let mut arena = GestureArenaManager::default();
                            let mut distinct: std::collections::HashSet<u64> =
                                std::collections::HashSet::new();
                            for raw in adds.iter() {
                                let p = PointerId(*raw);
                                distinct.insert(p.0);
                                arena.add(p, boxed_mock(MockRecognizer::new("r")));
                            }
                            assert!(
                                arena.arena_count() <= distinct.len(),
                                "active_pointer_count {} exceeded distinct pointer count {}",
                                arena.arena_count(),
                                distinct.len()
                            );
                        });
                });
                Ok(())
            })
            .expect("P-T15.5-B held for all sampled cases");
    }

    /// **P-T15.5-C** — `hold` / `release` symmetry: any sequence that
    /// holds an arena and then either releases it or cancels it leaves
    /// the arena in a non-held state. Equivalent to "every `hold` is
    /// followed by exactly one resolving event (release or cancel)".
    #[flui_core::test]
    fn prop_hold_release_symmetry(cx: &mut TestAppContext) {
        let mut runner = proptest_runner();
        // 0 = release, 1 = cancel
        let strategy = (1usize..5, 0u8..2);
        runner
            .run(&strategy, |(num_entries, resolver)| {
                cx.update(|cx| {
                    let _ = cx
                        .open_window(Default::default(), |_, cx| cx.new(|_| crate::EmptyView))
                        .unwrap()
                        .update(cx, |_, window, cx| {
                            let mut arena = GestureArenaManager::default();
                            let p = PointerId(0);
                            for _ in 0..num_entries {
                                arena.add(p, boxed_mock(MockRecognizer::new("r")));
                            }
                            arena.hold(p);
                            assert!(
                                arena
                                    .arenas
                                    .iter()
                                    .find(|(pid, _)| *pid == p)
                                    .map(|(_, a)| a.hold_count > 0)
                                    .unwrap_or(false),
                                "hold(p) increments hold_count"
                            );
                            match resolver {
                                0 => arena.release(p, window, cx),
                                _ => arena.cancel(p, window, cx),
                            }
                            // Either resolver path drops hold_count
                            // back to zero (release decrements + may
                            // sweep + close; cancel closes directly
                            // and zeroes hold_count). The arena may
                            // have been gc'd — that is trivially
                            // hold_count == 0.
                            let still_held = arena
                                .arenas
                                .iter()
                                .find(|(pid, _)| *pid == p)
                                .map(|(_, a)| a.hold_count > 0)
                                .unwrap_or(false);
                            assert!(
                                !still_held,
                                "hold without matching resolve leaked: hold_count > 0"
                            );
                        });
                });
                Ok(())
            })
            .expect("P-T15.5-C held for all sampled cases");
    }
}
