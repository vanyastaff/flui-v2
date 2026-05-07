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
