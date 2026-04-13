---
spec_id: S02a
title: flui-platform-crate-skeleton
phase: I
depends_on: [S01a.1, S01a.2, S01a.3, S01a.4, S01b, S01c, S01d]
blocks: [S02b]
status: draft
date: 2026-04-13
---

# S02a — flui-platform-crate-skeleton

## Context

First atomic step of the flui-platform extraction (see
[roadmap](2026-04-13-flui-core-roadmap.md)). The roadmap originally
scoped S02 as "create the crate + move `platform/test/**` and
`platform/visual_test.rs`". Adversarial review from three specialized
agents (flui-arch-reviewer, rust-api-migration-auditor,
migration-risk-adversary) established that this scope is **structurally
impossible** as a single reviewable step. The reasoning:

1. **Trait-return cycle.** `PlatformWindow::as_test` at
   [`platform.rs:723`](../../../crates/flui-core/src/platform.rs#L723)
   returns `Option<&mut TestWindow>`, and `PlatformDispatcher::as_test`
   returns `Option<&TestDispatcher>`. Both trait definitions live in
   `flui-core`. Moving `TestWindow` / `TestDispatcher` into
   `flui-platform` while the traits stay in `flui-core` makes
   `flui-core` reference types from `flui-platform`, forcing
   `flui-core → flui-platform`. But `TestPlatform` implements `Platform`
   which lives in `flui-core`, forcing `flui-platform → flui-core`. A
   Cargo dependency cycle.

2. **`VisualTestPlatform` closes a second cycle.**
   [`platform/visual_test.rs:31`](../../../crates/flui-core/src/platform/visual_test.rs#L31)
   holds `Rc<dyn Platform>` and `impl Platform for VisualTestPlatform`.
   Moving it forces the same cycle regardless of which direction is
   chosen for the other test types.

3. **`TestPlatform` references non-platform `flui-core` types directly.**
   [`platform/test/platform.rs:37`](../../../crates/flui-core/src/platform/test/platform.rs#L37)
   holds `Mutex<crate::Brightness>`. `Brightness` is defined in
   [`brightness.rs`](../../../crates/flui-core/src/brightness.rs). The
   type must also cross the boundary — it is not isolated to the
   platform subtree.

4. **Trait method signatures pull in supporting types.** The `Platform`
   trait methods reference `ClipboardItem`, `WindowParams`,
   `AnyWindowHandle`, `CursorStyle`, `Menu`, `Keymap`, and more. All of
   them live in `flui-core` outside the platform subtree. A "traits-only"
   move is not possible; any trait move drags a web of supporting types
   that would make the PR ~5000 LoC and violate the S01a-split principle
   that each migration step is a single reviewable commit.

The conclusion is that the original S02 must be split. **S02a (this
spec) creates the empty `flui-platform` crate and nothing else.** The
actual content flip — `Platform` trait family + all supporting types +
test backends + `TestAppContext` / `VisualTestContext` — is deferred to
a separate spec **S02b (`flui-platform-trait-and-test-flip`)**, to be
written after this one lands. S02b is the one large coordinated PR that
performs the whole flip in a single atomic step.

S02a is the smallest possible unblocking step. It adds a crate to the
workspace with zero source-level dependencies, zero feature flags, and
zero moved files, so that S02b can proceed without also having to
create workspace scaffolding in the same commit.

## Goals

1. Add `crates/flui-platform/` as a new workspace member of the flui
   workspace, registered in the root `Cargo.toml`.
2. Populate it with the minimum content needed for the crate to compile:
   a `Cargo.toml` inheriting workspace metadata and a `src/lib.rs` whose
   only content is a module-level doc comment describing the crate's
   role and a single `#![warn(missing_docs)]` attribute to match the
   linting discipline used across the workspace.
3. Amend the roadmap document to reflect the S02 → {S02a, S02b} split,
   rewriting the dependency edges for S02–S06 and the Phase I
   dependency graph accordingly.
4. Add a smoke CI job that runs `cargo check -p flui-platform` and
   `cargo build -p flui-platform` on Ubuntu, macOS, and Windows so that
   the crate is covered from day one.
5. Ensure every existing CI check, lock-check, and workspace build
   remains green after the change — nothing inside `flui-core` is
   touched.

## Non-goals

1. **No source file moves.** No `.rs` file from
   `crates/flui-core/src/platform/` or anywhere else in `flui-core` is
   moved, copied, or deleted. `platform/test/**` and
   `platform/visual_test.rs` stay exactly where they are today.
2. **No public re-export changes.** The explicit re-export list at
   [`lib.rs:122-211`](../../../crates/flui-core/src/lib.rs#L122-L211),
   which was just locked in by S01a.3, is not touched. No new
   `pub use flui_platform::…` lines are added.
3. **No other crate gains a `flui-platform` dependency.** `flui-core`,
   `flui-widgets`, `flui-navigator`, `flui-a11y`, `flui-theme`,
   `flui-material`, `flui-animate`, `flui-macros`, and the examples are
   all untouched.
4. **No trait extraction.** `Platform`, `PlatformWindow`,
   `PlatformDisplay`, `PlatformDispatcher`, `PlatformTextSystem`,
   `PlatformKeyboardLayout`, `PlatformKeyboardMapper`, and
   `PlatformHeadlessRenderer` stay defined in `flui-core` under this
   spec. They move in S02b.
5. **No feature flag plumbing.** `flui-platform` has no `[features]`
   block. It does not declare `test-support`, `wayland`, `x11`, or any
   other gate. Feature forwarding from `flui-core/test-support` to
   `flui-platform/test-support` is a S02b problem.
6. **No `flui-macros` changes.** The hard-coded
   `flui_core::TestAppContext`, `flui_core::BackgroundExecutor`,
   `flui_core::ForegroundExecutor`, `flui_core::run_test` token-stream
   literals at
   [`flui-macros/src/test.rs:148,164,190,197,235,255,283`](../../../crates/flui-macros/src/test.rs)
   remain valid because nothing moved out of `flui-core` yet.
7. **No build.rs / cbindgen / FXC / naga changes.** Those are
   per-backend concerns owned by S03–S05.
8. **No `Cargo.lock` strategy games.** The lockfile update produced by
   adding the workspace member is committed along with the rest of the
   change, like any other workspace edit.

## Current state

### Workspace layout

The current root `Cargo.toml` declares the following members
(verbatim, from the workspace root):

```toml
[workspace]
members = [
    "crates/flui-core",
    "crates/flui-macros",
    "crates/flui-widgets",
    "crates/flui-animate",
    "crates/flui-navigator",
    "crates/flui-a11y",
    "crates/flui-theme",
    "crates/flui-material",
    "examples/nav_demo",
    "examples/material_demo",
    "examples/animation_demo",
    "tooling/lock-checks",
]
resolver = "3"
```

There is no `[workspace.dependencies]` table. Each crate declares its
own dependency versions. This is a pre-existing pattern; S02a does not
try to change it.

Workspace inheritable metadata exists under `[workspace.package]`
(`edition = "2024"`, `rust-version = "1.85"`, `authors`, `license`,
`repository`), and is consumed by every current crate via
`edition.workspace = true` and friends.

### flui-core platform subtree

The current `crates/flui-core/src/platform/` directory (no changes
expected in S02a):

```
platform.rs            # module root: traits, re-exports, cfg-gated submod decls
platform/app_menu.rs
platform/keyboard.rs
platform/keystroke.rs
platform/layer_shell.rs
platform/linux.rs      # pub(crate) mod root for Linux backends
platform/linux/**
platform/mac.rs        # pub(crate) mod root for macOS backends
platform/mac/**
platform/test.rs       # pub(crate) test module root
platform/test/dispatcher.rs  # 162 LoC
platform/test/display.rs     #  33 LoC
platform/test/platform.rs    # 482 LoC
platform/test/window.rs      # 394 LoC
platform/visual_test.rs      # 254 LoC, macOS-only
platform/web.rs        # pub(crate) mod root for Web backend
platform/web/**
platform/wgpu.rs       # pub(crate) mod root for wgpu backend
platform/wgpu/**
platform/windows.rs    # pub(crate) mod root for Windows backends
platform/windows/**
```

None of these files are touched by S02a.

### Explicit re-export list (S01a.3 ground truth)

[`crates/flui-core/src/lib.rs:122-211`](../../../crates/flui-core/src/lib.rs#L122-L211)
contains an explicitly enumerated `pub use platform::{…}` list, split
across `#[cfg]` blocks for target OS, `test-support` feature, and the
macOS+test-support dual gate used by `VisualTestPlatform`. S02a adds no
entries and removes no entries. The header comment at
[`lib.rs:126`](../../../crates/flui-core/src/lib.rs#L126) explicitly
calls this block "the prerequisite for S02 extracting the platform
subtree into the sibling `flui-platform` crate" — that promise is
preserved; S02a simply creates the sibling without populating it yet.

### Lock-checks baseline

[`tooling/lock-checks`](../../../tooling/lock-checks) currently
implements two subcommands relevant to the migration:

- `check-stubs` — scans for `unimplemented!()` / `todo!()` /
  `unreachable!()` sites and compares against a committed inventory.
- `check-platform-imports` — walks `crates/flui-core/src/platform/**`
  for `use crate::*;` / `use flui_core::*;` globs and produces
  `docs/reports/platform-imports.md`.

Neither of these scans `crates/flui-platform/**` today. S02a does not
need to change that, because S02a adds no files inside
`crates/flui-platform/` that would contain stubs or glob imports. See
[Open questions](#open-questions) for whether S02b should extend the
scans to the new crate.

### Roadmap document

[`docs/superpowers/specs/2026-04-13-flui-core-roadmap.md`](2026-04-13-flui-core-roadmap.md)
currently numbers the migration as S02 → S03 → S04 → S05 → S06 with
the semantic "S02 = skeleton + move test/ + visual_test". S02a rewrites
that part of the roadmap to:

- **S02a** — skeleton only (this spec).
- **S02b** — Platform trait family + supporting types + test backends
  flip (one coordinated PR).
- **S03** — wgpu + linux (dependency changed from `S02` to `S02b`).
- **S04** — macOS (dependency changed from `S03` to `S03`, unchanged
  transitively).
- **S05** — Windows.
- **S06** — Web + top-level files + deletion of
  `flui-core/src/platform/`.

The Phase I ASCII dependency diagram at
[`roadmap:428-437`](2026-04-13-flui-core-roadmap.md#L428-L437) is
updated to insert S02b between S02a and S03.

## Design

### Crate layout

```
crates/flui-platform/
├── Cargo.toml
└── src/
    └── lib.rs
```

That's the entire crate. No sub-modules, no build.rs, no tests
directory, no examples. Two files.

### `crates/flui-platform/Cargo.toml`

```toml
[package]
name = "flui-platform"
version = "0.1.0"
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "Platform abstraction layer for flui (skeleton crate — populated incrementally by migration specs S02b through S06)"
keywords = ["desktop", "gui", "platform"]
categories = ["gui"]

[lib]
name = "flui_platform"
path = "src/lib.rs"
doctest = false

[lints]
workspace = true
```

Rationale for each field:

- `name = "flui-platform"` matches the hyphenated convention used by
  every other crate in the workspace; the library name
  `flui_platform` (underscored) is used in source code and re-export
  paths.
- Workspace inheritance for `edition`, `authors`, `license`,
  `repository`, `rust-version` matches every other flui crate and means
  a workspace-wide bump (e.g. edition 2024 → 2027) propagates
  automatically.
- `description`, `keywords`, `categories` mirror `flui-core`'s style so
  the crate is publishable without further edits if we ever choose to
  ship it.
- `doctest = false` matches `flui-core`
  ([`Cargo.toml:58`](../../../crates/flui-core/Cargo.toml#L58)).
- `[lints] workspace = true` picks up the workspace-wide clippy
  discipline (`dbg_macro = "deny"`, `redundant_clone = "deny"`,
  `disallowed_methods = "deny"`, …) without any per-crate
  repetition.
- No `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`, or
  target-specific dependency tables. No `[features]` block. The crate
  is a leaf in the dependency graph with nothing to depend on yet.

### `crates/flui-platform/src/lib.rs`

```rust
//! # flui-platform
//!
//! Platform abstraction layer for the flui UI framework.
//!
//! This crate is **intentionally empty** in spec S02a. It is a
//! reserved slot in the workspace dependency graph that will be
//! populated incrementally by subsequent migration specs:
//!
//! - **S02b** — the `Platform` trait family (`Platform`,
//!   `PlatformWindow`, `PlatformDisplay`, `PlatformDispatcher`,
//!   `PlatformTextSystem`, `PlatformKeyboardLayout`,
//!   `PlatformKeyboardMapper`, `PlatformHeadlessRenderer`), supporting
//!   value types referenced by trait signatures
//!   (`ClipboardItem`, `WindowParams`, `AnyWindowHandle`, `CursorStyle`,
//!   `Menu`, `Keymap`, `Brightness`, …), and the test backends
//!   (`TestPlatform`, `TestDispatcher`, `TestDisplay`, `TestWindow`,
//!   `VisualTestPlatform`). S02b is the "trait flip" — the point at
//!   which `flui-core` first gains a dependency on `flui-platform`.
//! - **S03** — wgpu backend and Linux (`x11`, `wayland`, `headless`)
//!   backends.
//! - **S04** — macOS backend (Metal, cbindgen-generated shader ABI).
//! - **S05** — Windows backend (DirectX, FXC shader compilation).
//! - **S06** — web backend, the `keystroke` / `keyboard` / `app_menu` /
//!   `layer_shell` / `scap_screen_capture` top-level modules, and
//!   deletion of `flui-core/src/platform/`. After S06, `flui-core`
//!   re-exports the Platform API from this crate.
//!
//! See `docs/superpowers/specs/2026-04-13-flui-core-roadmap.md` for
//! the full migration plan.

#![warn(missing_docs)]
```

Rationale:

- Crate-level doc comment so `cargo doc -p flui-platform` produces a
  non-empty page explaining what the crate is for during the migration
  window.
- `#![warn(missing_docs)]` matches `flui-core`'s
  [`lib.rs:2`](../../../crates/flui-core/src/lib.rs#L2) discipline. The
  empty crate trivially satisfies it; any item added in S02b must carry
  its own docs.
- No `pub` items, no `mod` declarations, no `use` statements. Every
  line in the file is a comment or an attribute.

### Root `Cargo.toml` edit

Add `"crates/flui-platform",` to the `[workspace] members` list,
keeping alphabetical-ish grouping so the ordering stays close to
existing convention (current ordering puts `flui-core` first, then
other crates by rough dependency order). Concretely, the new list:

```toml
[workspace]
members = [
    "crates/flui-core",
    "crates/flui-platform",
    "crates/flui-macros",
    "crates/flui-widgets",
    "crates/flui-animate",
    "crates/flui-navigator",
    "crates/flui-a11y",
    "crates/flui-theme",
    "crates/flui-material",
    "examples/nav_demo",
    "examples/material_demo",
    "examples/animation_demo",
    "tooling/lock-checks",
]
resolver = "3"
```

`flui-platform` is inserted immediately after `flui-core`, before any
higher-level crate, because in the final state (post-S06) `flui-core`
depends on it and it sits one level below `flui-core` in the graph.

### Roadmap document edit

In `docs/superpowers/specs/2026-04-13-flui-core-roadmap.md`:

1. **Ordered spec list table** (around line 390-403): replace the
   single S02 row with two rows:

   | Spec | Title | Depends on | Summary |
   |---|---|---|---|
   | **S02a** | `flui-platform-crate-skeleton` | S01a.1, S01a.2, S01a.3, S01a.4, S01b, S01c, S01d | Create empty `crates/flui-platform` workspace member with minimal `Cargo.toml` and placeholder `lib.rs`. No file moves, no re-exports, no feature flags. |
   | **S02b** | `flui-platform-trait-and-test-flip` | S02a | Move `Platform` trait family, supporting value types (`ClipboardItem`, `WindowParams`, `AnyWindowHandle`, `CursorStyle`, `Menu`, `Keymap`, `Brightness`, …), `platform/test/**`, `platform/visual_test.rs`, and the `app/{test_context,test_app,visual_test_context,headless_app_context}.rs` test scaffolding into `flui-platform` in one coordinated commit. Update `flui-macros` test-macro code generation to emit `flui_platform::` paths. Re-export from `flui-core` to preserve the public surface. First introduction of the `flui-core → flui-platform` dependency edge. |

2. **Dependency edges** on rows S03, S04, S05, S06 change from `S02`,
   `S03`, `S04`, `S05` to reference `S02b` as the root migration
   trigger. Specifically S03's `Depends on` column becomes `S02b`.

3. **Dependency diagram** (around lines 428-437) updates to:

   ```
            ┌─ S01a.2 ─┐
            ├─ S01a.3 ─┼─ S01d ─┐
   S01a.1 ──┼─ S01a.4 ─┤         │
            ├─ S01b ───┤         │
            └─ S01c ───┴─────────┴─ S02a ─ S02b ─ S03 ─ S04 ─ S05 ─ S06 ─┬─ S07..S15 (parallelizable)
                                                                          │
                                                                          └─ S16 ─ (S17, S18, S19 parallel) ─ S20
   ```

4. **Narrative edit** in the "Rejected strategies" section or a new
   sibling paragraph explaining why the S02 → {S02a, S02b} split was
   necessary (the three cycle proofs summarised in this spec's
   [Context](#context)). Cite this spec's filename. Two short
   paragraphs.

No other lines in the roadmap change.

### CI coverage

The current CI workflow lives under `.github/workflows/`. S02a adds
`flui-platform` to the matrix of per-crate smoke checks so the new
crate never rots in a "not-built-on-Windows" corner. Concretely, the
following invocations must pass on Ubuntu, macOS, and Windows
runners:

```sh
cargo check -p flui-platform
cargo build -p flui-platform
cargo doc  -p flui-platform --no-deps
```

If the existing workflow already has a wildcard `cargo check
--workspace` job per OS, S02a does not add new jobs — the new crate is
picked up automatically by the wildcard. If the workflow pins
per-crate invocations (`cargo check -p flui-core`, `-p flui-widgets`,
…), a new line is added for `-p flui-platform` on each OS matrix leg.
The implementer inspects the current workflow and picks whichever is
less invasive. See [Open questions](#open-questions) for this
determination.

Lock-checks (`cargo run -p lock-checks -- check-stubs` and
`… check-platform-imports`) are unchanged: the first scans stubs in
files that haven't been added, and the second scans `flui-core`
platform imports that haven't changed.

### What S02a explicitly does not change

- `crates/flui-core/Cargo.toml` — untouched.
- `crates/flui-core/src/lib.rs` re-export list — untouched.
- `crates/flui-core/src/platform.rs` module header — untouched.
- `crates/flui-core/src/platform/**` files — untouched.
- `crates/flui-core/src/app/{test_context,test_app,visual_test_context,headless_app_context}.rs`
  — untouched.
- `crates/flui-macros/**` — untouched.
- `docs/reports/platform-imports.md` — unchanged (generator reproduces
  the same output, no diff).
- Committed stub inventory (under `tooling/lock-checks`) — unchanged.
- `Cargo.lock` — receives the new workspace member automatically; this
  is the only change in the lockfile. No version bumps of any existing
  dep.

## API surface

S02a adds exactly zero `pub` items to the workspace public API.

- No new symbols are introduced in `flui-platform` (the crate's `lib.rs`
  has no `pub` declarations).
- No re-exports change in `flui-core`.
- No downstream crate gains a new dependency.

The S01a.3 lock promise — "the explicit re-export list is the only
public surface of the platform subtree" — is preserved.

## Migration / Compatibility

- Zero call-site churn across the entire workspace. No `use` statement
  anywhere changes.
- Zero downstream impact: the new crate exists but is not referenced
  by any other crate. External consumers of `flui-core` see no
  difference.
- Reverting the spec is a single `git revert` of the single commit.
  `Cargo.lock` reverts along with it.
- `rustup` / `cargo` version requirements are unchanged —
  `rust-version = "1.85"` is inherited from workspace and matches
  every other crate.

## Testing strategy

1. **`cargo build --workspace` on each supported platform.** This is
   the primary signal. The new crate must compile on Ubuntu, macOS,
   and Windows runners. No cross-compilation is exercised (S02a does
   not touch any `target.*` dependency tables).

2. **`cargo check --workspace --all-targets`.** Ensures examples,
   tests, benches, and doctests in every crate still parse. Because
   S02a touches nothing but adds an empty crate, this must produce
   the exact same diagnostics as `main` (plus zero new).

3. **`cargo test --workspace`.** Regression check. All existing tests
   must continue to pass unchanged. flui-platform contributes no
   tests.

4. **`cargo doc -p flui-platform --no-deps`.** The crate has a
   doc-comment header; this confirms it renders without broken
   intra-doc links and without `missing_docs` warnings (the crate has
   no undocumented items because it has no items).

5. **`cargo clippy --workspace --all-targets -- -D warnings`.** The
   new crate inherits workspace lints; this confirms the lint
   inheritance works and the empty crate passes.

6. **Lock-checks.** Run `cargo run -p lock-checks -- check-stubs` and
   `cargo run -p lock-checks -- check-platform-imports`. Both must
   produce an unchanged output (no diff against the committed
   baselines).

7. **Workspace metadata sanity.** `cargo metadata --format-version 1
   --no-deps | jq '.workspace_members'` must include
   `"flui-platform 0.1.0 (path+…)"`. This is not a shipping test but
   a smoke assertion during local verification.

8. **Sibling crate smoke.** `cargo check -p flui-widgets`,
   `cargo check -p flui-navigator`, `cargo check -p flui-a11y`,
   `cargo check -p flui-theme`, `cargo check -p flui-material`,
   `cargo check -p flui-animate`, `cargo check -p flui-macros` — all
   must still pass. None depends on `flui-platform` but the
   workspace resolution changes, so the smoke check is cheap insurance
   against any accidental feature-unification surprise.

9. **Documentation test of the edited roadmap.** Run a markdown link
   checker (or eyeball) against the edited
   `2026-04-13-flui-core-roadmap.md` to ensure the S02a and S02b
   cross-references resolve to the right files.

## Open questions

1. **Per-crate vs wildcard CI invocations.** The existing GitHub
   Actions workflow under `.github/workflows/` may use
   `cargo check --workspace` (wildcard, picks up the new crate
   automatically) or per-crate invocations (must be extended). The
   implementer inspects the workflow before touching anything and
   picks the less invasive path. Resolution: during implementation,
   not during spec review.

2. **Should `flui-platform` have its own `[package.metadata.docs.rs]`
   block?** `flui-core` has none. Consistency suggests not. Not
   shipping-relevant for S02a. Proposal: no metadata block in S02a,
   reconsider in S02b if the crate will ever be published.

3. **Does the crate need `#![no_std]`?** No. `flui-core` is `std`-only
   and `flui-platform` will inherit that. An explicit decision is
   deferred until the first actual code move in S02b.

4. **Extending `check-platform-imports` to scan
   `crates/flui-platform/**`?** Not needed in S02a — the crate has
   zero `.rs` files beyond an empty `lib.rs`. Must be reconsidered in
   S02b as a hard requirement.

5. **Naming: `flui-platform` vs `flui-platform-core` vs
   `flui-runtime`?** The roadmap already commits to `flui-platform`
   throughout. No alternative proposed. Resolution: use
   `flui-platform` verbatim.

6. **Does this spec need to pre-declare the `[workspace.dependencies]`
   table to make S02b's dep wiring cleaner?** No. The current
   workspace does not use `[workspace.dependencies]` at all; adding it
   now would be an unrelated hygiene change. S02b can add it if
   needed.

## Done criteria

This spec is complete when **all** of the following are true on a
clean checkout of the landing commit:

1. `crates/flui-platform/Cargo.toml` exists with the exact contents
   specified in [Design](#design).
2. `crates/flui-platform/src/lib.rs` exists with the exact contents
   specified in [Design](#design).
3. Root `Cargo.toml` lists `"crates/flui-platform"` as a workspace
   member.
4. `Cargo.lock` has been updated to register the new workspace
   member; no other lockfile changes.
5. `cargo build --workspace` exits 0 on Ubuntu, macOS, and Windows.
6. `cargo check --workspace --all-targets` exits 0 on all three.
7. `cargo test --workspace` exits 0 on all three. No new tests, no
   test regressions.
8. `cargo clippy --workspace --all-targets -- -D warnings` exits 0 on
   all three.
9. `cargo doc -p flui-platform --no-deps` exits 0 and produces a
   non-empty crate docs page.
10. `cargo run -p lock-checks -- check-stubs` and
    `cargo run -p lock-checks -- check-platform-imports` both produce
    a zero-diff result against the committed baselines.
11. `docs/superpowers/specs/2026-04-13-flui-core-roadmap.md` has been
    updated with the S02 → {S02a, S02b} split: the ordered spec table
    contains both rows, the dependency edges from S03/S04/S05/S06 have
    been rewritten, the ASCII dependency diagram has been amended,
    and a short narrative paragraph has been added explaining why the
    split was necessary.
12. The landing PR is a single commit touching only:
    - `Cargo.toml` (root)
    - `Cargo.lock`
    - `crates/flui-platform/Cargo.toml` (new)
    - `crates/flui-platform/src/lib.rs` (new)
    - `docs/superpowers/specs/2026-04-13-flui-core-roadmap.md`
    - possibly `.github/workflows/*.yml` (iff per-crate invocations
      are in use there; skip otherwise)
    - possibly `docs/superpowers/specs/2026-04-13-S02a-flui-platform-crate-skeleton-design.md`
      (this file) if not already landed in a separate commit.
13. `git revert` of the landing commit returns the workspace to a
    state identical to the parent commit (byte-for-byte in
    source files; `Cargo.lock` reverts along).

## Test log

To be filled in during implementation. Expected entries:

- Local `cargo build --workspace` runtime delta (Linux / macOS /
  Windows), before vs. after the new crate is added. Expectation: well
  under 1 second — no code is being compiled.
- CI wall-clock delta per OS leg.
- `cargo doc --workspace --no-deps` timing, before vs. after.
- Lock-check outputs (must be unchanged).

## Follow-ups after S02a lands

1. **Draft S02b (`flui-platform-trait-and-test-flip`).** The
   coordinated large PR. Must address all the blockers raised by the
   S02 adversarial review:
   - Trait-return cycle on `as_test`.
   - `VisualTestPlatform` implementing `Platform` across the boundary.
   - `TestPlatform` referencing `crate::Brightness`.
   - Supporting types (`ClipboardItem`, `WindowParams`, …) crossing the
     boundary with the trait definitions.
   - `flui-macros` test-macro code generation updating to emit
     `flui_platform::` paths for `TestAppContext`, `BackgroundExecutor`,
     `ForegroundExecutor`, `run_test`.
   - Auto-trait assertions (`static_assertions`) on types whose
     visibility promotes from `pub(crate)` to `pub` (notably
     `TestPlatform`).
   - `test-support` feature forwarding: `flui-core/test-support` must
     activate `flui-platform/test-support` via `dep:flui-platform`.
   - `wayland` and `x11` must NOT be pulled into
     `flui-platform/test-support`; they remain backend features in
     `flui-core` until S03.
   - `check-platform-imports` extended to scan `flui-platform/**`
     with the same glob rules.
   - `check-stubs` extended likewise.
2. **Audit `flui-navigator/Cargo.toml:28`'s `test-support` dev-dep**
   activation and confirm it still resolves correctly after S02b
   (documented in S02b, not here).
3. **Decide on `[workspace.dependencies]`** before S02b — if adopted,
   the path-dependency entries for `flui-core` and `flui-platform`
   live in the workspace table and each crate's `[dependencies]`
   block uses `crate.workspace = true`. Optional, not load-bearing
   for S02b correctness.
