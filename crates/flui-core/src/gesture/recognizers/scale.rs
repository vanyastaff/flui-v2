//! `ScaleGestureRecognizer` + `ScaleStartDetails` /
//! `ScaleUpdateDetails` / `ScaleEndDetails`.
//!
//! ≥2 pointers; focal point + scale + rotation. **Rotation is always
//! 0.0 on current desktop platforms** — `PinchEvent.delta` is
//! scale-only, and Windows desktop has no native pinch at all (see
//! the design doc's "Explicit gaps" matrix).
//!
//! See the design doc § "ScaleGestureRecognizer".
