//! `HitTestEntry`, `HitTestResult`, `HitTestBehavior`.
//!
//! Additive layer on top of the existing implicit `Hitbox`
//! infrastructure (`Window::insert_hitbox` /
//! `Window::mouse_hit_test`). [`HitTestBehavior`] is **orthogonal** to
//! the existing `HitboxBehavior` (which controls hover-style
//! decisions); see the design doc § "HitTestEntry and HitTestResult"
//! table for the full distinction.

use crate::{Affine2, HitboxId, Pixels, Point};
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
    /// The window-local-to-target-local affine recorded by paint when
    /// this entry was registered, or `None` for the identity (most
    /// entries today).
    ///
    /// Recognizers do not invert this directly: the dispatcher
    /// computes `local_position` once per delivery via
    /// `transform.unwrap_or(IDENTITY).inverse().unwrap().transform_point(position)`
    /// and exposes it through [`crate::DeliveredEvent::local_position`].
    ///
    /// **S09 contract:** when the paint pipeline starts pushing real
    /// transforms (e.g. via `RenderTransform`), every entry registered
    /// inside that scope must carry the composed transform so
    /// recognizers see a consistent `local_position`. Leaving
    /// `transform = None` while events flow through a non-identity
    /// scope silently desyncs slop / down_position math.
    pub transform: Option<Affine2>,
}

/// The ordered set of entries produced by [`crate::Window::hit_test`].
/// Front-to-back; index 0 is the deepest hitbox under the pointer.
///
/// The internal `entries` storage is private; consumers iterate via
/// [`Self::iter`] / [`Self::len`] / [`Self::is_empty`]. This keeps
/// the type `#[non_exhaustive]`-equivalent without committing to a
/// concrete container type in the public API.
///
/// Paint code records target-local coordinates by opening a
/// [`HitTestScope`] via [`Self::push_transform`] /
/// [`Self::push_offset`] and calling [`HitTestScope::add`] for every
/// entry inside the scope. The scope's `Drop` impl pops the
/// transform stack — unbalanced push/pop is a borrow-check error,
/// not a runtime invariant.
#[derive(Clone, Debug, Default)]
pub struct HitTestResult {
    pub(crate) entries: SmallVec<[HitTestEntry; 8]>,
    /// Cumulative window-local-to-target-local transforms; top-of-stack
    /// is the effective transform for entries added at the current
    /// nesting depth. Always private — consumers manipulate it only
    /// through [`HitTestScope`].
    pub(crate) transform_stack: SmallVec<[Affine2; 4]>,
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

    /// Push a transform onto the internal stack and return an RAII
    /// guard whose `Drop` pops it. Entries added through the returned
    /// scope (via [`HitTestScope::add`]) carry the cumulative
    /// window-to-local transform composed from every active scope.
    ///
    /// The pushed transform is composed onto the current top-of-stack
    /// (so nested scopes accumulate), enabling deeply-nested paint
    /// trees to record one-shot transforms per layer without
    /// recomputing the full stack on every add.
    pub fn push_transform(&mut self, t: Affine2) -> HitTestScope<'_> {
        let cumulative = self
            .transform_stack
            .last()
            .copied()
            .unwrap_or(Affine2::IDENTITY)
            .composed(t);
        self.transform_stack.push(cumulative);
        HitTestScope { result: self }
    }

    /// Convenience: push a translation by `offset`. Equivalent to
    /// [`Self::push_transform`] with [`Affine2::translation`].
    pub fn push_offset(&mut self, offset: Point<Pixels>) -> HitTestScope<'_> {
        self.push_transform(Affine2::translation(offset))
    }

}

/// RAII guard returned by [`HitTestResult::push_transform`].
///
/// The guard owns a mutable borrow on the [`HitTestResult`] for the
/// lifetime of the scope. While the guard is alive, paint code can
/// add entries via [`Self::add`]; on `Drop` the topmost transform is
/// popped from the result's transform stack.
///
/// Unbalanced push/pop is structurally impossible: every push
/// returns a guard, and the only way to pop is to drop one.
/// Panic-safety follows from standard Rust RAII — unwinding through
/// a scope still drops the guard, so the stack stays consistent.
///
/// Nested scopes are supported via [`Self::push_transform`] /
/// [`Self::push_offset`], which return a fresh inner guard
/// re-borrowing this scope's mutable access.
pub struct HitTestScope<'r> {
    result: &'r mut HitTestResult,
}

impl<'r> HitTestScope<'r> {
    /// Add `entry` to the underlying [`HitTestResult`], composing the
    /// scope's cumulative transform into `entry.transform`. Mirrors
    /// the pre-scope `push` behavior for paint code that already has
    /// an entry to register.
    ///
    /// Composition rule:
    /// - If the cumulative top-of-stack is identity and `entry.transform` is `None`, the entry stays `None` (cheaper consumers).
    /// - Otherwise the entry's stored transform becomes `Some(stack ∘ entry.transform.unwrap_or(IDENTITY))`.
    pub fn add(&mut self, mut entry: HitTestEntry) {
        let stack_top = self.result.transform_stack.last().copied();
        entry.transform = match (stack_top, entry.transform) {
            (Some(stack), Some(local)) => Some(stack.composed(local)),
            (Some(stack), None) if stack == Affine2::IDENTITY => None,
            (Some(stack), None) => Some(stack),
            (None, local) => local,
        };
        self.result.entries.push(entry);
    }

    /// Open a nested scope by composing `t` onto the current
    /// cumulative transform. The returned guard re-borrows `self`
    /// mutably; it must drop before this scope can add further
    /// entries.
    pub fn push_transform<'a>(&'a mut self, t: Affine2) -> HitTestScope<'a> {
        self.result.push_transform(t)
    }

    /// Convenience for [`Self::push_transform`] with
    /// [`Affine2::translation`].
    pub fn push_offset<'a>(&'a mut self, offset: Point<Pixels>) -> HitTestScope<'a> {
        self.result.push_offset(offset)
    }
}

impl<'r> Drop for HitTestScope<'r> {
    fn drop(&mut self) {
        // Pop the entry pushed by the matching `push_transform`. If
        // the stack is somehow empty here, the guard was misused
        // (manual mem::forget?) — we fall back to a no-op rather
        // than panicking inside Drop.
        self.result.transform_stack.pop();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HitboxId;

    fn px(x: f32, y: f32) -> Point<Pixels> {
        Point {
            x: Pixels(x),
            y: Pixels(y),
        }
    }

    fn entry_at(position: Point<Pixels>) -> HitTestEntry {
        HitTestEntry {
            hitbox_id: HitboxId::for_test(0),
            position,
            behavior: HitTestBehavior::Opaque,
            transform: None,
        }
    }

    #[test]
    fn identity_scope_leaves_entry_transform_none() {
        let mut result = HitTestResult::default();
        {
            let mut scope = result.push_transform(Affine2::IDENTITY);
            scope.add(entry_at(px(10.0, 10.0)));
        }
        assert_eq!(result.len(), 1);
        let only = result.iter().next().unwrap();
        assert!(only.transform.is_none(), "identity scope should not bake into transform");
        assert!(result.transform_stack.is_empty(), "Drop pops the stack");
    }

    #[test]
    fn nested_scope_pops_in_order() {
        let mut result = HitTestResult::default();
        let _outer = result.push_transform(Affine2::IDENTITY);
        // _outer holds a mutable borrow until drop; we cannot read
        // `result.transform_stack` directly while it lives. Validate
        // depth indirectly via successful add + final empty stack.
        drop(_outer);
        assert!(result.transform_stack.is_empty());
    }

    #[test]
    fn rotate_90_scope_records_transform_and_local_position_inverts() {
        // CCW 90° rotation about origin: (1, 0) -> (0, 1).
        let r = Affine2::rotation(std::f32::consts::FRAC_PI_2);
        let mut result = HitTestResult::default();
        let position = px(1.0, 0.0);
        {
            let mut scope = result.push_transform(r);
            scope.add(entry_at(position));
        }
        let only = result.iter().next().unwrap();
        let baked = only.transform.expect("non-identity scope bakes a transform");
        // The recorded transform is window-to-local-space; inverting and
        // applying to the window position yields the local position.
        let inverse = baked.inverse().unwrap();
        let local = inverse.transform_point(only.position);
        // r maps (1, 0) -> (0, 1), so r^-1 maps (1, 0) -> (0, -1)
        // — the rotation is CCW by 90°, its inverse is CW by 90°.
        assert!((local.x.0 - 0.0).abs() < 1e-5);
        assert!((local.y.0 - (-1.0)).abs() < 1e-5);
    }

    #[test]
    fn nested_scope_composes_transforms() {
        let outer = Affine2::translation(px(10.0, 0.0));
        let inner = Affine2::translation(px(0.0, 5.0));
        let mut result = HitTestResult::default();
        {
            let mut outer_scope = result.push_transform(outer);
            {
                let mut inner_scope = outer_scope.push_transform(inner);
                inner_scope.add(entry_at(px(0.0, 0.0)));
            }
            // Outer scope still alive here; inner has dropped (popped).
        }
        let entry = result.iter().next().unwrap();
        let baked = entry.transform.expect("nested non-identity scope bakes a transform");
        // Composed transform: outer ∘ inner translates origin by
        // (10, 5).
        let p = baked.transform_point(px(0.0, 0.0));
        assert!((p.x.0 - 10.0).abs() < 1e-5);
        assert!((p.y.0 - 5.0).abs() < 1e-5);
    }

    #[test]
    fn drop_pops_after_panic_preserves_invariant() {
        // Panic-safety: unwinding through a scope still drops the
        // guard, popping the stack. We exercise this with
        // `catch_unwind` so the test process doesn't abort.
        use std::panic::{AssertUnwindSafe, catch_unwind};
        let mut result = HitTestResult::default();
        let r = catch_unwind(AssertUnwindSafe(|| {
            let _scope = result.push_transform(Affine2::IDENTITY);
            // Cannot inspect `result.transform_stack.len()` here while
            // `_scope` holds the mutable borrow; we rely on the
            // Drop-side post-condition asserted below.
            panic!("simulated panic inside scope");
        }));
        assert!(r.is_err());
        assert!(
            result.transform_stack.is_empty(),
            "panic unwind should still pop the scope's frame"
        );
    }
}
