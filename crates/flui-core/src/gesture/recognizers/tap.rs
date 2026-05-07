//! `TapGestureRecognizer` + `TapDetails` / `TapDownDetails` /
//! `TapUpDetails`.
//!
//! Primary / secondary / tertiary buttons; `request_focus_on_tap_down`
//! wired through the `on_focus_request` (S12 seam) hook;
//! `semantic_actions()` returns `&[SemanticAction::Tap]` (S08 seam).
//!
//! See the design doc § "TapGestureRecognizer".
