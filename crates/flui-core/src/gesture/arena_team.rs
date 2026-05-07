//! `GestureArenaTeam` — captain-deferred grouping of recognizers.
//!
//! Constructed via [`GestureArenaTeam::with_captain`] plus
//! [`GestureArenaTeam::add_member`]; the internal
//! `Rc<RefCell<…>>` plumbing is hidden.
//!
//! See the design doc § "GestureArenaTeam".

use super::{GestureDisposition, GestureRecognizer};
use smallvec::SmallVec;
use std::cell::RefCell;
use std::rc::Rc;

/// A captain-led group of recognizers that **defer** disposition to
/// their captain. The captain is the only recognizer that may declare
/// `Accepted`; team members may declare `Rejected` to leave the team
/// but their `Accepted` is coerced to `Possible` by the team.
///
/// `#[non_exhaustive]` so future fields (e.g. team-priority hints)
/// are non-breaking additions.
#[non_exhaustive]
pub struct GestureArenaTeam {
    /// The captain recognizer — the only team member whose
    /// `Accepted` resolves the entire team. Currently stored but
    /// unread by the active arena flow because public team
    /// registration via `InteractiveElement` is deferred (the
    /// `with_captain` constructor exists for forward-compat use
    /// from a future `GestureDetector` widget). Property test P6
    /// covers `resolve_member` directly.
    #[allow(dead_code, reason = "future GestureDetector integration uses this")]
    pub(crate) captain: Rc<RefCell<Box<dyn GestureRecognizer>>>,
    pub(crate) members: SmallVec<[Rc<RefCell<Box<dyn GestureRecognizer>>>; 2]>,
}

impl GestureArenaTeam {
    /// Create a new team with `captain` as the captain recognizer.
    /// The captain is the only recognizer in the team that may
    /// declare [`GestureDisposition::Accepted`]; team members that
    /// return `Accepted` are coerced to `Possible` by the team via
    /// [`Self::resolve_member`].
    pub fn with_captain(captain: Box<dyn GestureRecognizer>) -> Self {
        Self {
            captain: Rc::new(RefCell::new(captain)),
            members: SmallVec::new(),
        }
    }

    /// Add a member recognizer to the team.
    pub fn add_member(&mut self, member: Box<dyn GestureRecognizer>) {
        self.members.push(Rc::new(RefCell::new(member)));
    }

    /// Resolve a member's reported disposition. Members that report
    /// `Accepted` are converted to `Possible` (deferred to captain);
    /// members that report `Rejected` keep that verdict; captain's
    /// `Accepted` resolves the entire team.
    ///
    /// Currently only exercised by property test P6 in `tests`. T15
    /// public registration on `InteractiveElement` (or a future
    /// `GestureDetector` widget) is the production caller — until
    /// then teams cannot enter the live arena flow.
    #[allow(dead_code, reason = "T15 GestureDetector registration target")]
    pub(crate) fn resolve_member(
        &self,
        is_captain: bool,
        reported: GestureDisposition,
    ) -> GestureDisposition {
        if is_captain {
            return reported;
        }
        match reported {
            // Members cannot accept on behalf of the team.
            GestureDisposition::Accepted => GestureDisposition::Possible,
            other => other,
        }
    }

    /// Number of members (excluding the captain).
    pub fn member_count(&self) -> usize {
        self.members.len()
    }
}

#[cfg(test)]
mod tests {
    //! T23 — Property test for the arena-team `resolve_member` rule.
    //!
    //! `resolve_member` is a pure function over `(is_captain,
    //! reported)`; we exercise the whole `(bool × {Accepted, Rejected,
    //! Possible})` cartesian product via `proptest::TestRunner` to
    //! lock the captain-deferral contract.

    use super::*;
    use crate::gesture::{PointerEvent, PointerId};
    use proptest::test_runner::{Config, TestRunner};

    /// Map a 0..3 selector to a `GestureDisposition`. Stable order so
    /// proptest's shrinker can converge predictably.
    fn disposition_from_u8(v: u8) -> GestureDisposition {
        match v % 3 {
            0 => GestureDisposition::Accepted,
            1 => GestureDisposition::Rejected,
            _ => GestureDisposition::Possible,
        }
    }

    /// Minimal recognizer stub used to construct a `GestureArenaTeam`
    /// in property tests. The captain identity is irrelevant for the
    /// pure `resolve_member` function under test.
    struct Stub;
    impl GestureRecognizer for Stub {
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
        fn name(&self) -> &'static str {
            "stub"
        }
        fn add_pointer(&mut self, _: PointerId, _: &PointerEvent) {}
        fn handle_event(
            &mut self,
            _: &PointerEvent,
            _: &mut crate::Window,
            _: &mut crate::App,
        ) -> GestureDisposition {
            GestureDisposition::Possible
        }
        fn sweep_accepted(&mut self, _: PointerId, _: &mut crate::Window, _: &mut crate::App) {}
        fn rejected(&mut self, _: PointerId, _: &mut crate::Window, _: &mut crate::App) {}
    }

    /// **P6** — `resolve_member` invariants:
    ///   - captain reports passthrough (returns the same value)
    ///   - non-captain `Accepted` → `Possible` (deferred to captain)
    ///   - non-captain `Rejected` → `Rejected`
    ///   - non-captain `Possible` → `Possible`
    #[test]
    fn prop_team_resolve_member_invariants() {
        let mut runner = TestRunner::new(Config {
            cases: 64,
            ..Config::default()
        });
        runner
            .run(
                &(proptest::bool::ANY, 0u8..3),
                |(is_captain, reported_sel)| {
                    let reported = disposition_from_u8(reported_sel);
                    let team = GestureArenaTeam::with_captain(Box::new(Stub));
                    let resolved = team.resolve_member(is_captain, reported);

                    if is_captain {
                        assert_eq!(resolved, reported, "captain disposition must pass through");
                    } else {
                        // Exhaustive over the three current dispositions.
                        // `GestureDisposition` is `#[non_exhaustive]`, so
                        // adding a new variant deliberately breaks this
                        // match — that forces an explicit decision on
                        // captain-deferral semantics for the new
                        // disposition rather than silently wildcarding.
                        match reported {
                            GestureDisposition::Accepted => assert_eq!(
                                resolved,
                                GestureDisposition::Possible,
                                "member Accepted must coerce to Possible"
                            ),
                            GestureDisposition::Rejected => assert_eq!(
                                resolved,
                                GestureDisposition::Rejected,
                                "member Rejected stays Rejected"
                            ),
                            GestureDisposition::Possible => assert_eq!(
                                resolved,
                                GestureDisposition::Possible,
                                "member Possible stays Possible"
                            ),
                        }
                    }
                    Ok(())
                },
            )
            .expect("P6 held for all sampled cases");
    }
}
