//! `PanGestureRecognizer` (free pan), `HorizontalDragGestureRecognizer`
//! (axis-locked horizontal), `VerticalDragGestureRecognizer`
//! (axis-locked vertical) + shared `DragStartDetails` /
//! `DragUpdateDetails` / `DragEndDetails`.
//!
//! Each recognizer feeds a per-pointer `VelocityTracker` and emits
//! velocity at end. Coexists with the imperative `cx.active_drag` /
//! `AnyDrag` flow — both can be active simultaneously; the dispatcher
//! resets `cx.propagate_event = true` between the arena pass and
//! `dispatch_mouse_event` so `on_mouse_down` always fires.
//!
//! See the design doc § "Drag recognizers".
