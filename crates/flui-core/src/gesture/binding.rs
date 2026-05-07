//! `GestureBinding` — per-`Window` owner of the arena, settings, and
//! sanitizer.
//!
//! Auto-trait posture: `!Send + !Sync` (transitively via
//! `Rc<RefCell<dyn GestureRecognizer>>` inside the arena). Per-`Window`
//! types are main-thread-only by construction.
//!
//! See the design doc § "GestureBinding".
