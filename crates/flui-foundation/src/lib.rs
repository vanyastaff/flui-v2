//! Stub for U3 — flui foundation primitives.
//!
//! Ported from v1 `flui-foundation` per `docs/plans/2026-05-19-001-feat-v1-port-track-2-egui-easy-plan.md`.
//! Track 2 (DX & low-ceremony onboarding) per `STRATEGY.md`.
//!
//! Implementation lands in U3 — per-module port: `assert`, `binding`, `callbacks`, `consts`,
//! `debug`, `error`, `id`, `key` (scoped under `foundation::key`, collision-aware vs
//! `flui_core::element::identity::Key`), `notifier`, `observer`.
//!
//! Parallel-impl with `flui-core` — fold deferred к future K-track audit.

// Real modules land in U3. Per-module compile-status audit
// (`docs/research/v1-compile-status-audit.md`) precedes U3 to classify each
// module as MECHANICAL / REPAIR / REWRITE.
