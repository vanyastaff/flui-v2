# ADR-021: Facade-pattern decomposition policy for oversized modules

**Date:** 2026-05-13
**Status:** Proposed — escalates to Accepted after A10a PR 1.0 (`WindowCore` foundation) merges and validates the pattern.
**Scope:** `crates/flui-core` (and future Tier-A / Tier-B crates).
**Related specs:** `docs/superpowers/specs/2026-05-13-A10-xl-file-split-design.md` (A10 implementation).
**Related roadmap items:** A2 (remaining globs), A8 (`#[non_exhaustive]` audit), A9 (crate-boundary review), K91 (29 globs → explicit).
**Precedent ADRs:** ADR-001 (invalidation scope), ADR-008 (window chrome contract), ADR-018 (modal overlay layering).

## Context

`crates/flui-core` (~80k LoC across 200+ files) has accumulated several oversized modules. As of 2026-05-13:

| File | LoC | KB | Top `impl` blocks |
|---|---:|---:|---:|
| `window.rs` | 6036 | 264 | 44 |
| `geometry.rs` | 3802 | 126 | 192 |
| `elements/div.rs` | 3673 | 166 | 21 |
| `app.rs` | 3368 | 150 | 19 |
| `platform.rs` | 2419 | 97 | 36 |
| `element.rs` | 1902 | 72 | 16 |

Symptoms observed:

1. **Reviewer context loss.** Single-file size exceeds typical agent context windows; CodeRabbit / Copilot / `flui-arch-reviewer` issue irrelevant comments on parts they didn't actually re-read.
2. **PR conflict surface.** Concurrent K-track / S-track work touches the same monolithic files, creating merge friction.
3. **Cargo-doc rebuild cost.** `rustdoc` regenerates the whole file on any change inside.
4. **IDE jump-to-definition discoverability.** When `impl<T> Bounds<T> where T: ...` appears 5+ times in one file, navigation becomes guesswork.
5. **Duplicated test fixtures.** When tests grow next to XL-files, `make_test_window()` and analogues drift between locations.

The project already practises facade-pattern decomposition for 5 modules (`app`, `element`, `platform`, `keymap`, `text_system`). The pattern exists but lacks codified policy: when to apply it, what required practices come along, what's prohibited. This ADR codifies it.

Sub-crate extraction (e.g. moving `Window` into `flui-window`) is **not** part of this policy. Phase I (platform extraction) is frozen until Phase III; cyclic deps with `App` / `Element` / `FramePhase` block crate-level splits. This ADR addresses **module-level** decomposition only.

## Decision

### Threshold: when to apply

A file in `crates/flui-core/` (or any Tier-A / Tier-B crate) becomes a candidate for facade-pattern decomposition when **at least one** of:

| Trigger | Threshold |
|---|---|
| Total lines of code in one `.rs` file | **> 2500** |
| Number of `impl` blocks on a single top-level type | **> 50** |
| Distinct semantic clusters identifiable by `Explore`-style inventory | **> 10** |
| Cross-track PR friction (≥ 3 K-/S-/SF-/A-track PRs colliding in one file within 90 days) | qualitative, but binding |

Once any trigger fires, a corresponding sub-track (A10, A11, …) opens with its own design-spec following the A10 template.

### Required structure

```text
crates/<crate>/src/
├── X.rs                  (facade — top-level types + re-exports, no business logic)
└── X/
    ├── core.rs           (optional — private `XCore` struct holding fields, see Practice 1)
    ├── <cluster>.rs      (impl X methods + cluster-private types)
    └── test_fixtures.rs  (#[cfg(test)] pub(super) fn ... — shared helpers, see Practice 3)
```

The facade `X.rs` MUST contain:

1. Module-level rustdoc (`//! See spec: <path>`) describing the architecture.
2. Top-level type declarations (struct/enum/trait) — these stay in the facade.
3. `pub use X::sub::...` re-exports (style chosen per Practice 4).
4. Constructor methods (`fn new`, `fn from_*`, `fn with_*`) on the top-level type.

The facade `X.rs` MUST NOT contain:

- Business-logic methods (those live in submodules' `impl X { ... }` blocks).
- New `pub fn` not present before the split.
- Implementation details that aren't required by other modules in the same crate.

### Required practices

1. **Foundation `Core` struct** — for top-level types with >100 fields (`Window` qualifies; `App` may qualify after K07 lands). Extract fields into `pub(super) struct XCore` in `X/core.rs`; the public type wraps `Core` **via a plain `pub(super) core: XCore` field**. Sibling submodules access shared state through `self.core.<field>` or `pub(super)` methods on `XCore`. Pattern reference: `bevy_ecs::world::World` (private `entities`/`storages`/`bundles` indirection), `wgpu::Device` (private `inner` field with method-style access).

   **PROHIBITED**: `impl Deref<Target = XCore> for X` (auto-deref escape). The `Deref` impl makes `XCore`'s method-resolution surface reachable to anyone holding `&X` even though `XCore` is `pub(super)` — a caller in `crate::other_module` can write `*window_ref` and obtain `&XCore`, then invoke its `pub(super)` methods (which are visible at the call site because the indirect type is opaque, but method dispatch still works). Use plain field access — `self.core.field` inside `impl X` blocks — instead. Audited by `rust-api-migration-auditor` review of A10a, 2026-05-13.

   **Embedding rules** for `XCore`:
   - `XCore` must be embedded **by value**, not boxed (`Box<XCore>` / `Arc<XCore>` / `Rc<XCore>` ALL prohibited). Reason: any `Rc<...>` / `Arc<...>` fields *inside* `XCore` are shared with external callbacks (e.g. platform on-frame callbacks holding `Rc<Cell<bool>>` clones); reallocating the wrapper would not move the heap-allocated `Rc` payload but would break code that compares struct addresses. By-value embedding preserves heap-layout semantics of `Rc::ptr_eq`.

2. **State-machine markers stay in the facade.** Small enums that encode invariants (`DispatchPhase`, `DrawPhase`, `WindowControlArea`) remain in `X.rs` next to their owner. Transition methods (`with_draw_phase`, etc.) move to the relevant cluster submodule. Rationale: `debug_assert!` invariant checks need the enum and its declaration in the same readable scope for reviewer audit.

3. **Shared `test_fixtures` module** — one `#[cfg(test)] pub(super) mod test_fixtures` in the facade (or `X/test_fixtures.rs`) hosts `fn make_test_X()`, mock-handler builders, and any helper used by ≥ 2 cluster submodules. Without this, every cluster grows its own copy and they drift.

4. **Re-export style choice** (per facade):

| Re-export style | Use when | Examples |
|---|---|---|
| **Explicit per-symbol** (`pub use sub::{A, B, C};`) | semver-stable public API; risk of accidental re-export of internal types | `window/`, `gesture/`, `platform/`, `elements/div/` |
| **Glob** (`pub use sub::*;`) | mathematically related primitives where every type is intended public | `geometry/`, `animation/` (curated `mod.rs` already uses this), `keymap/` |

5. **Cfg-gate parity.** When a submodule is `#[cfg(...)]`-gated (e.g. `inspect_state.rs` behind `feature = "inspector"`), the gate must appear on:
   - the `mod` declaration in the facade,
   - every call-site in **other** submodules that invokes its methods.

   Missing parity creates compile-time errors that only surface under a specific cfg configuration — easy to miss in CI.

6. **Naming: avoid collisions with crate-root modules.** A `window/inspector.rs` next to `crate::inspector` is forbidden. Use a qualified name like `window/inspect_state.rs` or `window/inspector_glue.rs`. Same for `elements/div/inspect_state.rs` vs `crate::inspector`.

7. **Visibility ladder** (minimum-up). For every item touched during a split, ask: what is the smallest visibility that still compiles?

   | Visibility | Use for |
   |---|---|
   | `pub` | only items that were `pub` **before** the split, OR new re-exports from the facade |
   | `pub(crate)` | items shared between facade subtree and the rest of the crate (most `Window` fields qualify) |
   | `pub(super)` | items shared between sibling submodules within `X/` |
   | (private) | items used only inside one cluster file |

   **No new `pub` symbols** during a split unless the spec explicitly justifies it. The split is API-neutral by construction.

8. **`#[non_exhaustive]` is out-of-scope** for splits. It belongs to a separate API-stabilization track (A8). Mixing structural and API-semantic refactors in one PR makes review harder and CI breaks ambiguous to attribute.

9. **No new tests during the split** — only move existing tests. Coverage extension is a separate T-track concern. Mixing them obscures whether a test failure is from the split or from a new assertion.

10. **Module-level rustdoc anchor** — every new submodule file starts with:

    ```rust
    //! See spec: docs/superpowers/specs/<YYYY-MM-DD>-<track>-<topic>-design.md
    ```

    This makes the spec discoverable from the code via `rust-analyzer` "Show docs".

### Migration discipline

Each split sub-track is a sequence of small PRs, one cluster per PR, ordered by increasing risk. Foundation work (Practice 1) is a separate first PR. For each PR:

- Public API diff must be empty (`cargo public-api diff`, or manual `pub` symbol grep).
- `cargo build -p <crate>` + `cargo test --workspace` green.
- For high-risk PRs (touching K-track / SF-track invariants), pre-PR triple launch of reviewers (`flui-arch-reviewer` + `migration-risk-adversary` + `rust-api-migration-auditor`) is mandatory.

## Consequences

### Positive

- IDE / agent context fits per cluster — reviewers can re-read whole submodules.
- PR-coverage surface narrows; concurrent K/SF/A-track work has more file-level isolation.
- `cargo doc` and `rust-analyzer` rebuilds are faster on per-cluster edits.
- New contributors orient by cluster-name (`focus.rs`, `paint.rs`, `event_dispatch.rs`) instead of scrolling 6000-line files.
- Sub-crate extraction (Phase III) becomes a mechanical follow-up, not a redesign — the cluster boundary already exists.

### Negative

- More files in the source tree (each split adds 8–12 files).
- `git blame` requires `git log --follow` to track method history across the split.
- Re-export lists in the facade need maintenance — a new `pub` symbol requires editing the facade list (this is a feature, not a bug, for semver discipline).
- Architectural overhead of `WindowCore` (Practice 1) adds one level of field indirection. Compile-time cost is negligible; cognitive cost is non-zero for first-time readers (mitigated by Practice 10 rustdoc anchor).

### Risks

- **Sibling reach via `pub(crate)`** — without Practice 1 (`Core` struct), submodules can reach into truly private fields by upgrading visibility. This is enforced by review, not by the language.
- **Re-export drift** — explicit per-symbol lists may forget new `pub` items. Mitigation: `cargo-semver-checks` (R2 track) catches this in CI.
- **Cfg-gate parity miss** — see Practice 5; a CI matrix with the relevant feature combinations (A5 track) prevents the bug.

## Alternatives considered

### Alt-1: sub-crate extraction (`flui-window`, `flui-geometry`)

**Rejected.** Phase I (platform extraction) is frozen until Phase III; cyclic deps between `Window` / `App` / `Element` / `FramePhase` make multi-crate splitting premature. `flui-platform` skeleton exists (S02a) but `S02b–S06` are deferred. Re-open the option when a concrete Phase III driver (iOS / Android / Web) forces a real platform-abstraction boundary.

### Alt-2: `X/mod.rs` instead of `X.rs` + `X/`

**Rejected.** Would require restructuring crate-root re-exports (`lib.rs:317 pub use window::*;` becomes `pub use window::mod::*;` — Rust doesn't allow that). The project already standardised on `X.rs` + `X/` facade pattern for 5 modules; introducing two patterns side-by-side would confuse contributors. K91 contract (preserve `Key`/`ValueKey`/`GlobalKey` crate-root visibility) also pushes for facade-style continuity.

### Alt-3: Leave XL-files as-is, rely on rustdoc grouping and `#[doc(hidden)]`

**Rejected.** Doesn't address reviewer context, IDE jump-to-definition, or `cargo doc` rebuild cost. Documents the symptom (large file) without removing it.

### Alt-4: Sealed extension traits (`pub(crate) trait FocusOps for Window`)

**Rejected for primary use, kept as escape hatch.** Each cluster declaring `pub(crate) trait XOps {}` impl'd for the top-level type works but degrades IDE Ctrl+Click discoverability (jumps to trait def, not impl block) and rustdoc readability (impls scatter across multiple trait pages). Acceptable for narrow internal helpers (e.g. `pub(super) trait FrameTransitions`), not for top-level public API.

## Precedents in `flui-core`

| Module | Pattern | Re-export style | Notes |
|---|---|---|---|
| `app.rs` + `app/` | facade | mixed (glob for context, explicit for cell) | Foundation candidate: `App` will need `AppCore` once K07 lands. |
| `element.rs` + `element/` | facade | explicit (`pub(crate) use identity::ElementIdStack; pub use identity::{ElementId, ...}`) | K91 contract pinned. |
| `platform.rs` + `platform/` | facade | explicit (semver-aware) | S01a.3 prerequisite. |
| `keymap.rs` + `keymap/` | facade | glob | Small internal API. |
| `text_system.rs` + `text_system/` | facade | glob | Already split; deeper subsplit deferred to A11. |
| `gesture/` (no facade file) | mod.rs internal | explicit | One of two non-facade large modules; uses `gesture/mod.rs` with strict re-export discipline. |
| `animation/` (no facade file) | mod.rs internal | curated `mod.rs` | The other non-facade large module; uses `animation/mod.rs` with explicit symbol lists. |

The two `mod.rs`-based modules (`gesture/`, `animation/`) predate this ADR. They are not in violation — both maintain strict re-export discipline — but new splits SHALL use the `X.rs` + `X/` facade form for consistency.

## Open questions

1. **When does a facade become a candidate for sub-crate extraction?** Suggested heuristic: when ≥ 80% of `pub(crate)` calls within `X/` cluster fan in/out only within the cluster (no cross-cluster sharing), the cluster can extract. Validate via `cargo udeps` and `cargo modules` after A10 lands.
2. **Should `WindowCore`-style `Core` struct become a workspace-level convention?** Wait for A10a PR 1.0 to land; revisit in ADR amendment with one or two more applications (`AppCore`, `ElementCore`?) seen in practice.
3. **`cargo-semver-checks` adoption** (R2) — once available in CI, replaces the manual `pub use` graph cross-check listed in `docs/superpowers/specs/2026-05-08-K99-msrv-bump-1.95-design.md`.

## References

- A10 design spec: `docs/superpowers/specs/2026-05-13-A10-xl-file-split-design.md`
- bevy/world: <https://docs.rs/bevy_ecs/latest/bevy_ecs/world/struct.World.html> — Core-struct field indirection precedent.
- wgpu/Device: <https://docs.rs/wgpu/latest/wgpu/struct.Device.html> — same pattern with `inner: ManuallyDrop<...>`.
- K99 (MSRV 1.95) spec: enables AFIT/RPITIT — useful for `pub(super) trait` helpers if Alt-4 is partially adopted.
- K91 (29 globs → explicit) — A10 closes 3 of those 29 as A2 synergy.

## Status flow

- **Proposed** (2026-05-13) — spec drafted, this ADR drafted, ROADMAP updated with A10 entry.
- **Accepted** — after A10a PR 1.0 (`WindowCore` foundation) merges and demonstrates the pattern compiles, tests pass, and reviewers don't flag it.
- **Superseded** — if a future ADR amends the threshold or required practices. Status field updates in-place.
