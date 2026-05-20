//! Stub for U5/U6/U7 — `flui` CLI binary.
//!
//! Ported from v1 `flui-cli` per `docs/plans/2026-05-19-001-feat-v1-port-track-2-egui-easy-plan.md`.
//! Track 2 (DX & low-ceremony onboarding) per `STRATEGY.md`.
//!
//! Implementation lands in U5 (scaffolding) → U6 (commands) → U7 (templates).

use std::process::ExitCode;

fn main() -> ExitCode {
    // Real entry point lands in U5 — clap-derive `Cli` struct + Subcommand dispatch.
    eprintln!("flui-cli skeleton — U5/U6/U7 implementation pending");
    // Non-zero exit so CI / scripts treat skeleton invocation as "not yet implemented",
    // not "no-op success". Falling back to ExitCode::FAILURE preserves normal drop order
    // (vs `std::process::exit`, which bypasses destructors / stdio flush).
    ExitCode::FAILURE
}
