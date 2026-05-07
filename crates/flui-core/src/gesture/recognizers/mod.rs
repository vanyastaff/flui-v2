//! Concrete `GestureRecognizer` implementations.
//!
//! Five recognizers, each with its `*Details` callback payload type:
//!
//! - [`tap`] — `TapGestureRecognizer` (single-tap) +
//!   `TapDownDetails` / `TapUpDetails` / `TapDetails`.
//! - [`double_tap`] — `DoubleTapGestureRecognizer` +
//!   `DoubleTapDetails`.
//! - [`long_press`] — `LongPressGestureRecognizer` +
//!   `LongPressDetails`.
//! - [`drag`] — `PanGestureRecognizer`,
//!   `HorizontalDragGestureRecognizer`,
//!   `VerticalDragGestureRecognizer` + shared
//!   `DragStartDetails` / `DragUpdateDetails` / `DragEndDetails`.
//! - [`scale`] — `ScaleGestureRecognizer` +
//!   `ScaleStartDetails` / `ScaleUpdateDetails` / `ScaleEndDetails`.

pub mod double_tap;
pub mod drag;
pub mod long_press;
pub mod scale;
pub mod tap;
