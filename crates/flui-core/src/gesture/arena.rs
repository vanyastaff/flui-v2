//! `GestureArenaManager`, `GestureArena`, `GestureArenaEntry`,
//! `GestureDisposition`. The competition arbitrator.
//!
//! All three arena types (`GestureArena`, `GestureArenaEntry`,
//! `GestureArenaManager`) are `pub(crate)` — they have no public
//! method surface. Consumers reach the manager via `pub(crate)`
//! accessors on `GestureBinding`. `GestureDisposition` is `pub`
//! because `GestureRecognizer::handle_event` returns it.
//!
//! Auto-trait posture: `!Send + !Sync` due to
//! `Rc<RefCell<dyn GestureRecognizer>>` in entries.
//!
//! See the design doc § "GestureArena and GestureArenaManager".
