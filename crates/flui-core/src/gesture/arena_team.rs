//! `GestureArenaTeam` — captain-deferred grouping of recognizers.
//!
//! Constructed via `with_captain(Box<dyn GestureRecognizer>)` plus
//! `add_member(Box<dyn GestureRecognizer>)`. The internal
//! `Rc<RefCell<…>>` plumbing is hidden.
//!
//! See the design doc § "GestureArenaTeam".
