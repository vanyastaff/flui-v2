//! Animation tick scheduling: sealed [`TickTarget`] trait and `App::active_animations` set.
//!
//! The `AnimationTick` phase of each frame walks an active-controller set populated
//! by [`TickTarget`] implementors (initially only `AnimationController`). Inactive
//! controllers are NOT visited — the cost of an animation-free frame is one
//! `FxHashSet::is_empty()` check.
//!
//! [`TickTarget`] is sealed via the crate-private `seal::Sealed` supertrait so
//! Tier-C cannot inject arbitrary tick targets in K04. Future SF08 (async widgets)
//! and audio / spring / particle controllers will add `impl TickTarget` for their
//! types additively; opening the trait (removing the sealing supertrait) is itself
//! an additive change.
//!
//! Implementation lands in Task 29 of the K04 plan.
