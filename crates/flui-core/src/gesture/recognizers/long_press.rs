//! `LongPressGestureRecognizer` + `LongPressDetails`.
//!
//! Async timer via `window.spawn(async { smol::Timer::after(d).await })`.
//! Async back-channel to the arena via
//! `Weak<RefCell<GestureArenaManager>>` plus `pointer_index`. Drop
//! cancels the timer task.
//!
//! See the design doc § "LongPressGestureRecognizer".
