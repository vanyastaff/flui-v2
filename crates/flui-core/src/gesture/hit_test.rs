//! `HitTestEntry`, `HitTestResult`, `HitTestBehavior`.
//!
//! Additive layer on top of the existing implicit `Hitbox`
//! infrastructure (`Window::insert_hitbox` /
//! `Window::mouse_hit_test`). [`HitTestBehavior`] is **orthogonal** to
//! the existing `HitboxBehavior` (which controls hover-style
//! decisions); see the design doc § "HitTestEntry and HitTestResult"
//! table for the full distinction.

use crate::{HitboxId, Pixels, Point};
use smallvec::SmallVec;

/// One target identified during a hit-test pass, ordered front-to-back
/// (deepest paint wins index 0).
///
/// `#[non_exhaustive]` so future fields (e.g. `paint_layer`,
/// `clip_rect`) are non-breaking additions.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct HitTestEntry {
    /// The `HitboxId` of the committed hitbox that matched.
    pub hitbox_id: HitboxId,
    /// The hit-test position in window-local pixels (same as the
    /// source `PointerEvent.position`; carried for recognizers that
    /// need it).
    pub position: Point<Pixels>,
    /// The behavior of this entry — controls whether propagation
    /// continues past it.
    pub behavior: HitTestBehavior,
}

/// The ordered set of entries produced by [`crate::Window::hit_test`].
/// Front-to-back; index 0 is the deepest hitbox under the pointer.
///
/// The internal `entries` storage is private; consumers iterate via
/// [`Self::iter`] / [`Self::len`] / [`Self::is_empty`]. This keeps
/// the type `#[non_exhaustive]`-equivalent without committing to a
/// concrete container type in the public API.
#[derive(Clone, Debug, Default)]
pub struct HitTestResult {
    pub(crate) entries: SmallVec<[HitTestEntry; 8]>,
}

impl HitTestResult {
    /// Iterate front-to-back (deepest first).
    pub fn iter(&self) -> impl Iterator<Item = &HitTestEntry> {
        self.entries.iter()
    }

    /// Number of hit-test entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` iff no hitbox was hit.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Construct an empty result. Crate-internal — consumers receive
    /// this only from `Window::hit_test`.
    pub(crate) fn new() -> Self {
        Self {
            entries: SmallVec::new(),
        }
    }

    /// Push a new entry at the end (deeper paint = earlier index in
    /// `mouse_hit_test`, so callers push in front-to-back order).
    pub(crate) fn push(&mut self, entry: HitTestEntry) {
        self.entries.push(entry);
    }
}

/// How a hit-test entry interacts with propagation in the gesture
/// dispatch path.
///
/// `HitTestBehavior` is **orthogonal** to `HitboxBehavior`:
///
/// | Concept           | Owner              | Affects                                                |
/// |-------------------|--------------------|--------------------------------------------------------|
/// | `HitboxBehavior`  | paint-time hitbox  | `is_hovered`, `should_handle_scroll` (style decisions) |
/// | `HitTestBehavior` | gesture entry      | Arena participation + recognizer propagation           |
///
/// A single `Interactivity` may carry both, e.g. an overlay sets
/// `HitboxBehavior::BlockMouseExceptScroll` (so style `is_hovered`
/// returns `false` for elements behind it) **and**
/// `HitTestBehavior::Translucent` (so gesture recognizers behind it
/// still join the arena for the same pointer).
///
/// `#[non_exhaustive]` so future behaviors (e.g.
/// `OpaqueExceptScroll`) are non-breaking additions.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum HitTestBehavior {
    /// Receives events; stops propagation. Default for
    /// `InteractiveElement`, consistent with Flutter's default.
    #[default]
    Opaque,
    /// Receives events and forwards them to the next entry behind it.
    Translucent,
    /// Does not receive events itself; defers to its children. If no
    /// child matches, falls through to the next entry behind it.
    DeferToChild,
}
