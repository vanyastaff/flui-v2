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

use super::{GestureRecognizer, PointerEvent, PointerId};
use smallvec::SmallVec;
use std::cell::RefCell;
use std::rc::Rc;

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
    /// `true` while a recognizer holds the arena open past `Up`
    /// (e.g. `DoubleTap` waiting for a second tap).
    pub(crate) is_held: bool,
}

impl GestureArena {
    fn new() -> Self {
        Self {
            entries: SmallVec::new(),
            winner: None,
            is_open: true,
            is_held: false,
        }
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
                r.handle_event(event, window, cx)
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
            // Close the arena unless the winner asked to hold.
            if !arena.is_held {
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
        if arena.is_held || arena.winner.is_some() || !arena.is_open {
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
    /// caller calls [`Self::release`].
    pub(crate) fn hold(&mut self, pointer_id: PointerId) {
        if let Some((_, arena)) = self.arenas.iter_mut().find(|(id, _)| *id == pointer_id) {
            arena.is_held = true;
        }
    }

    /// Release a held arena; resumes normal sweep semantics on the
    /// next `Up` (or the call site can call [`Self::sweep`]
    /// directly).
    pub(crate) fn release(
        &mut self,
        pointer_id: PointerId,
        window: &mut crate::Window,
        cx: &mut crate::App,
    ) {
        let Some((_, arena)) = self.arenas.iter_mut().find(|(id, _)| *id == pointer_id) else {
            return;
        };
        arena.is_held = false;
        let needs_sweep = arena.winner.is_none() && arena.is_open;
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
        arena.is_held = false;
        for r in entries.iter() {
            r.borrow_mut().rejected(pointer_id, window, cx);
        }
        self.gc(pointer_id);
    }

    /// Async back-channel for recognizers whose `Accepted` decision
    /// fires from outside `handle_event` (e.g.
    /// `LongPressGestureRecognizer`'s timer).
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
        if !arena.is_held {
            arena.is_open = false;
        }
        for loser in losers.iter() {
            loser.borrow_mut().rejected(pointer_id, window, cx);
        }
        self.gc(pointer_id);
    }

    /// Garbage-collect closed, empty arenas.
    fn gc(&mut self, pointer_id: PointerId) {
        if let Some(idx) = self
            .arenas
            .iter()
            .position(|(id, a)| *id == pointer_id && !a.is_open && a.entries.is_empty())
        {
            self.arenas.remove(idx);
        }
    }
}
