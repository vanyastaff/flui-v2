//! `GestureRecognizer` trait + `SemanticAction` enum.
//!
//! `GestureRecognizer: ?Send + ?Sync`. Per-`Window` callback registry
//! is main-thread-only; matches the existing `Interactivity` posture.
//!
//! Trait contract: implementations MUST NOT call
//! `cx.stop_propagation()` from inside `handle_event`. The arena
//! declares its winner via `GestureDisposition::Accepted`, not via
//! propagation control. The dispatcher resets `cx.propagate_event = true`
//! between the arena pass and the existing raw-listener chain to
//! preserve the `cx.active_drag` / `AnyDrag` contract.
//!
//! See the design doc § "GestureRecognizer trait".
