//! `VelocityTracker`, `Velocity`, `PositionSample`.
//!
//! Bounded least-squares velocity estimator. Direct port of Flutter's
//! `LeastSquaresSolver::solve` weighted-quadratic fit; bounded
//! `VecDeque<PositionSample>` configured via `GestureSettings`.
//!
//! See the design doc § "VelocityTracker".
