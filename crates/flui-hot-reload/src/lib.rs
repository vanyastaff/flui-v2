//! Stub for U11/U12/U13 — flui hot-reload (active research).
//!
//! Status: **active research, ship gated on U11 outcome.** See
//! `docs/plans/2026-05-19-001-feat-v1-port-track-2-egui-easy-plan.md` U11.
//!
//! Track 2 (DX & low-ceremony onboarding) per `STRATEGY.md`. Supersedes prior
//! "Phase IV / R-track" deferral in `.ai-factory/RESEARCH.md` (audit-finding 8).
//!
//! v1's `ScenePlugin::build_scene -> flui_layer::Scene` FFI shape is structurally
//! incompatible with v2 — `flui_core::Scene` is a paint-operations buffer (not user-
//! constructable), and `Render::render` requires `&mut Window`+`&mut Context` (non-`Send`
//! entity-scoped). U12 is a redesign, NOT a mechanical port.
//!
//! Implementation pipeline:
//! - U11 — Rust ecosystem mechanism research (`subsecond` / `hot-lib-reloader` / custom dynlib),
//!         produces `docs/research/hot-reload-rust-2026.md` decision doc.
//! - U12 — port + re-arch over gpui-ce primitives (dynlib, driver, host, plugin, pipeline modules).
//! - U13 — K04 integration via `App::defer_to(NextFrameStart, ...)`; demo proving AE3 latency.

#![allow(dead_code)]

// Real modules land in U12. Mechanism choice driven by U11 research outcome.
