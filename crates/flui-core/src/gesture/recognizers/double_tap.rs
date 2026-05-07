//! `DoubleTapGestureRecognizer` + `DoubleTapDetails`.
//!
//! Two-tap state machine spanning two `Down`/`Up` sequences. Uses
//! arena `hold` between FirstUp and SecondDown.
//!
//! See the design doc § "DoubleTapGestureRecognizer".
