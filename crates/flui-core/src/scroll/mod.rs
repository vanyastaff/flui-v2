//! ADR-019: scroll-physics scaffolding for a future `Scrollable` widget.
//!
//! This module ships the **types and contracts** the future
//! `Scrollable` widget will compose with. The widget itself is not in
//! this commit — per the ADR, the deliverable here is the type-level
//! surface so that when somebody picks up the `Scrollable` work the
//! contract is not a research project on top of an implementation
//! project.
//!
//! See `docs/research/adr/ADR-019-scroll-physics.md`.

mod physics;
mod state;

pub use physics::*;
pub use state::*;
