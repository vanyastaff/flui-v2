//! Stub for U9/U10 — flui DevTools substrate.
//!
//! Ported from v1 `flui-devtools` per `docs/plans/2026-05-19-001-feat-v1-port-track-2-egui-easy-plan.md`.
//! Track 2 (DX & low-ceremony onboarding) per `STRATEGY.md`.
//!
//! Implementation pipeline:
//! - U8 — `flui_core::inspectable::InspectableElement` (K22 minimal substrate, `pub(crate)` initially).
//! - U9 — VM Service protocol layer (JSON-RPC over TCP, `smol::net`, `#[non_exhaustive]` enums).
//!         Defaults bind to `127.0.0.1`; `triggerHotReload(path)` validates path within watched dir.
//! - U10 — profiler subscribing to K04 `FrameProfile`; timeline export (Chrome trace JSON).
//!
//! v1's `memory.rs` / `network.rs` / `remote.rs` modules were stubs — replaced by the
//! `protocol/` subtree implementing Flutter VM Service protocol clone (subset).

// Real modules land in U9/U10.
