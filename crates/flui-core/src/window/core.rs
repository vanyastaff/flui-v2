//! Private state container for [`Window`](super::Window) — A10a PR 1.0 foundation.
//!
//! See spec: `docs/superpowers/specs/2026-05-13-A10-xl-file-split-design.md`.
//! Policy: `docs/research/adr/ADR-021-xl-file-split-discipline.md` Practice 1.
//!
//! # Purpose
//!
//! `WindowCore` is the private struct that owns the ~140 fields of [`Window`]. It exists so
//! future sibling submodules under `window/` (focus, hitbox, draw, layout, event_dispatch,
//! etc., landing in PRs 1.3-1.11) can be split across files without each new file gaining
//! direct read/write access to every other cluster's private state. Sibling modules see
//! only what `pub(super)` exposes here; the rest of the crate continues to reach `Window`
//! via its existing public API.
//!
//! # Contract (binding — ADR-021 Practice 1)
//!
//! - **Visibility:** `pub(super) struct WindowCore`. Never `pub`, never `pub(crate)`.
//! - **Embedding:** [`Window`] holds `pub(super) core: WindowCore` as a **plain field**.
//!   - `impl Deref<Target = WindowCore> for Window` is **prohibited** — auto-deref would
//!     leak `WindowCore`'s method-resolution surface to any caller holding `&Window`
//!     (even outside `window/`), defeating the `pub(super)` boundary. Audited by
//!     `rust-api-migration-auditor` on 2026-05-13.
//!   - `Box<WindowCore>` / `Arc<WindowCore>` / `Rc<WindowCore>` are **prohibited**. Several
//!     `Rc<Cell<bool>>` / `Rc<RefCell<...>>` fields inside `WindowCore` (`active`,
//!     `needs_present`, `input_rate_tracker`, ...) are cloned and shared with platform
//!     callbacks; the wrapper layout must not relocate them across the heap, otherwise
//!     `Rc::ptr_eq` comparisons in platform code silently break.
//! - **Field access from `impl Window` blocks:** use `self.core.<field>` explicitly.
//! - **No new `pub` symbols:** PR 1.0 is API-neutral. Public API guarantees are verified
//!   via `cargo public-api diff` against `main`.
//!
//! # Status (2026-05-13)
//!
//! PR 1.0 phase 2 — struct definition lands in Task #9. This file currently exists as the
//! scaffolded module slot (Task #8) so the `mod core;` line in `window.rs` compiles
//! against an empty module before fields are extracted.

// Defensive note for future contributors: this module is named `core`, which shadows
// Rust's built-in `core` crate name inside this file's local scope. Macros emitted by
// `slotmap`, `serde`, `derive_more`, etc. use leading-`::` paths (`::core::option::Option`)
// and remain unaffected. If you need to reference the standard `core` crate from within
// `window/core.rs` (rare), use the absolute path `::core::*`.
