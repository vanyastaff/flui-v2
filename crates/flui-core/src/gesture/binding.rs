//! `GestureBinding` — per-`Window` owner of the arena, settings, and
//! sanitizer.
//!
//! Auto-trait posture: `!Send + !Sync` (transitively via
//! `Rc<RefCell<dyn GestureRecognizer>>` inside the arena).
//! Per-`Window` types are main-thread-only by construction; do
//! **not** wrap a `GestureBinding` in `Arc`.
//!
//! See the design doc § "GestureBinding".

use super::arena::GestureArenaManager;
use super::dispatch::PointerSanitizer;
use super::{GestureSettings, PointerId};

/// Per-window owner of the gesture arena, the configurable
/// [`GestureSettings`], and the [`PointerSanitizer`].
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
/// `#[non_exhaustive]` for forward-compatibility — future
/// per-`Window` gesture state (e.g. an explicit
/// `GestureArenaTeam` registry, an A4-driven `tracing::Span`
/// handle) can be added without a breaking change.
#[non_exhaustive]
pub struct GestureBinding {
    arena: GestureArenaManager,
    settings: GestureSettings,
    sanitizer: PointerSanitizer,
}

impl Default for GestureBinding {
    fn default() -> Self {
        Self::new()
    }
}

impl GestureBinding {
    /// Construct a new binding with default [`GestureSettings`].
    pub(crate) fn new() -> Self {
        Self {
            arena: GestureArenaManager::default(),
            settings: GestureSettings::default(),
            sanitizer: PointerSanitizer::default(),
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
        self.arena.arena_count()
    }

    /// Number of recognizers competing for `pointer_id`'s arena, or
    /// 0 if no arena is open for that pointer.
    pub fn arena_entry_count(&self, pointer_id: PointerId) -> usize {
        self.arena.entry_count(pointer_id)
    }

    // The full `GestureArenaManager` is intentionally pub(crate)-only.
    // External callers cannot mutate arena state directly; the
    // dispatch flow inside `Window::dispatch_event` is the single
    // source of truth for arena transitions.
    pub(crate) fn arena_mut(&mut self) -> &mut GestureArenaManager {
        &mut self.arena
    }
    pub(crate) fn arena(&self) -> &GestureArenaManager {
        &self.arena
    }
    pub(crate) fn sanitizer_mut(&mut self) -> &mut PointerSanitizer {
        &mut self.sanitizer
    }
}
