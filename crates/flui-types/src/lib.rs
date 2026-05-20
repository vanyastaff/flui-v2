//! Stub for U4 — flui net-new geometry + styling types.
//!
//! Ported from v1 `flui-types` per `docs/plans/2026-05-19-001-feat-v1-port-track-2-egui-easy-plan.md`.
//! Track 2 (DX & low-ceremony onboarding) per `STRATEGY.md`.
//!
//! Implementation lands in U4 — net-new types only (Bezier, Circle, Line, Matrix4, Offset,
//! Rect, RRect, Transform, Transform2d, Vector, RSuperellipse; Border, BorderRadius, Color32,
//! Decoration, Gradient, Shadow). Overlap types (Point<T>, Size<T>, Edges<T>, Corner, Pixels,
//! DevicePixels, ScaledPixels, Rems, Axis, Length) are already in `flui-core::geometry`/`color`
//! and NOT duplicated.
//!
//! Parallel-impl with `flui-core` — fold deferred к future K-track audit.

// Real modules land in U4. Per-module compile-status audit
// (`docs/research/v1-compile-status-audit.md`) precedes U4.
