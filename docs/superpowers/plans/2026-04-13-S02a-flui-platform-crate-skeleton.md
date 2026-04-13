# S02a — flui-platform Crate Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create an empty `crates/flui-platform` workspace member (Cargo.toml + doc-only `lib.rs`), register it in the root workspace, and update the migration roadmap to reflect the S02 → {S02a, S02b} split. No source files are moved; no public API changes.

**Architecture:** Pure scaffolding — two new files inside `crates/flui-platform/`, one edit to root `Cargo.toml`, one edit to the roadmap spec. The new crate is a leaf with no dependencies, no feature flags, and no `pub` items. The whole change lands in a single commit covering ~5 files.

**Tech Stack:** Rust 2024 edition (workspace inherit), Cargo workspaces, flui-v2 CI (`cargo check/clippy/test --workspace`).

**Spec:** [`docs/superpowers/specs/2026-04-13-S02a-flui-platform-crate-skeleton-design.md`](../specs/2026-04-13-S02a-flui-platform-crate-skeleton-design.md)

---

## Preconditions (before Task 1)

- [ ] **P1: Verify main-branch baseline is green**

Run from workspace root:

```bash
cargo build --workspace 2>&1 | tail -20
```

Expected: exit 0, warnings only (existing state). If this fails on `main`, STOP. The plan assumes a green baseline; debugging pre-existing failures is out of scope for S02a.

- [ ] **P2: Confirm working tree is clean**

Run:

```bash
git status --porcelain
```

Expected: empty output (or only `.serena/` untracked — ignore that one). If other changes exist, stash or commit them before starting.

- [ ] **P3: Confirm you are on the correct branch**

Run:

```bash
git branch --show-current
```

Expected: `main` (or a feature branch branched from `main`). The plan assumes the branch will hold a single S02a commit.

---

### Task 1: Create the Cargo manifest for `flui-platform`

**Files:**
- Create: `crates/flui-platform/Cargo.toml`

- [ ] **Step 1: Create the directory and Cargo.toml**

Create the file `crates/flui-platform/Cargo.toml` with the following exact contents:

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

Rationale for non-obvious choices:
- `name = "flui-platform"` (hyphenated) matches the convention used by every other crate in `crates/`. The library name `flui_platform` (underscored) is set via `[lib] name` to match how Rust resolves crate names in `use` statements.
- Workspace inheritance (`edition.workspace = true` etc.) picks up `edition = "2024"` and `rust-version = "1.85"` from the root `Cargo.toml` without per-crate duplication.
- `doctest = false` matches `flui-core` — there is nothing in the library to doctest at this point.
- `[lints] workspace = true` picks up the workspace-wide clippy discipline (`dbg_macro = "deny"`, `redundant_clone = "deny"`, `disallowed_methods = "deny"`, etc.) defined at the root.
- No `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`, target-specific dependency tables, or `[features]` block. The crate is a leaf with nothing to depend on yet. These tables will be populated in S02b.

- [ ] **Step 2: Stage the file and inspect it**

```bash
git add crates/flui-platform/Cargo.toml
git diff --cached crates/flui-platform/Cargo.toml
```

Expected: the full contents shown above. If anything differs (line endings, indentation), fix it before proceeding.

---

### Task 2: Create the library root

**Files:**
- Create: `crates/flui-platform/src/lib.rs`

- [ ] **Step 1: Create `src/lib.rs` with only a module-level doc comment**

Create the file `crates/flui-platform/src/lib.rs` with the following exact contents:

```rust
//! # flui-platform
//!
//! Platform abstraction layer for the flui UI framework.
//!
//! This crate is **intentionally empty** in spec S02a. It is a reserved
//! slot in the workspace dependency graph that will be populated
//! incrementally by subsequent migration specs:
//!
//! - **S02b** — the `Platform` trait family (`Platform`, `PlatformWindow`,
//!   `PlatformDisplay`, `PlatformDispatcher`, `PlatformTextSystem`,
//!   `PlatformKeyboardLayout`, `PlatformKeyboardMapper`,
//!   `PlatformHeadlessRenderer`), supporting value types referenced by
//!   trait signatures (`ClipboardItem`, `WindowParams`, `AnyWindowHandle`,
//!   `CursorStyle`, `Menu`, `Keymap`, `Brightness`, …), and the test
//!   backends (`TestPlatform`, `TestDispatcher`, `TestDisplay`,
//!   `TestWindow`, `VisualTestPlatform`). S02b is the "trait flip" — the
//!   point at which `flui-core` first gains a dependency on
//!   `flui-platform`.
//! - **S03** — wgpu backend and Linux (`x11`, `wayland`, `headless`)
//!   backends.
//! - **S04** — macOS backend (Metal, cbindgen-generated shader ABI).
//! - **S05** — Windows backend (DirectX, FXC shader compilation).
//! - **S06** — web backend, the `keystroke` / `keyboard` / `app_menu` /
//!   `layer_shell` / `scap_screen_capture` top-level modules, and
//!   deletion of `flui-core/src/platform/`. After S06, `flui-core`
//!   re-exports the Platform API from this crate.
//!
//! See `docs/superpowers/specs/2026-04-13-flui-core-roadmap.md` for the
//! full migration plan.

#![warn(missing_docs)]
```

Non-obvious choices:
- No `pub` declarations, no `mod` declarations, no `use` statements. Every non-blank line is a doc comment or an inner attribute. This is deliberate: the empty crate is the contract of S02a.
- `#![warn(missing_docs)]` matches `flui-core`'s top-of-file attribute and keeps linting discipline consistent. An empty crate trivially satisfies it.

- [ ] **Step 2: Stage the file and inspect it**

```bash
git add crates/flui-platform/src/lib.rs
git diff --cached crates/flui-platform/src/lib.rs
```

Expected: the full contents shown above, exactly.

---

### Task 3: Register the crate in the workspace

**Files:**
- Modify: `Cargo.toml` (workspace root, lines 1-16)

- [ ] **Step 1: Edit the workspace `members` array**

The current root `Cargo.toml` starts with:

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

Insert `"crates/flui-platform",` immediately after `"crates/flui-core",`. The result must be:

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

`flui-platform` is inserted immediately after `flui-core` because in the eventual post-S06 graph it sits one level below `flui-core`. No other lines in `Cargo.toml` change.

- [ ] **Step 2: Stage the file and inspect the diff**

```bash
git add Cargo.toml
git diff --cached Cargo.toml
```

Expected: exactly one `+` line (`    "crates/flui-platform",`), zero `-` lines.

---

### Task 4: First verification — workspace compiles

**Files:**
- No source edits; this task runs the Rust toolchain to confirm Tasks 1–3 are structurally sound.

- [ ] **Step 1: Run `cargo check --workspace`**

Run from workspace root:

```bash
cargo check --workspace 2>&1 | tail -30
```

Expected: exit 0. The output's last non-blank line should be something like:

```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in Xs
```

The invocation should also list `flui-platform` among the compiled crates on the first run (look for `Checking flui-platform v0.1.0 …` earlier in the output).

If this fails:
- `error: failed to parse manifest` — inspect the `Cargo.toml` from Task 1 for syntax errors.
- `error: can't find library, rename file to src/lib.rs` — the `src/lib.rs` from Task 2 is missing or mis-named.
- Any error inside another crate — rerun `cargo check --workspace` on `main` to confirm it is pre-existing. If it is, STOP and report; a pre-existing break is not an S02a concern.

- [ ] **Step 2: Run `cargo build --workspace`**

```bash
cargo build --workspace 2>&1 | tail -30
```

Expected: exit 0, similar output to Step 1. This also updates `Cargo.lock` to register the new workspace member. Do not commit `Cargo.lock` yet — it will be committed together with everything else at the end.

- [ ] **Step 3: Confirm `Cargo.lock` shows the new crate**

```bash
git diff Cargo.lock | head -30
```

Expected: the diff contains a new `[[package]] name = "flui-platform"` block. If the diff is empty, Cargo did not pick up the workspace member — rerun Step 2 after verifying Task 3 was saved.

---

### Task 5: Run the full local verification suite

**Files:**
- None. This task runs the tools the CI will run.

- [ ] **Step 1: Run clippy with workspace lints**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -30
```

Expected: exit 0. The empty `flui-platform` crate has no items and inherits workspace lints via `[lints] workspace = true`, so it trivially passes.

If a pre-existing clippy warning in another crate has been reclassified as an error by `-D warnings`, verify it reproduces on `main`. Pre-existing warnings are not an S02a concern — if they reproduce, STOP and report.

- [ ] **Step 2: Run the full test suite**

```bash
cargo test --workspace 2>&1 | tail -30
```

Expected: exit 0, same test counts as `main`. S02a adds zero tests and touches zero existing tests, so the numbers must match the baseline exactly.

- [ ] **Step 3: Build the flui-platform docs**

```bash
cargo doc -p flui-platform --no-deps 2>&1 | tail -10
```

Expected: exit 0, no warnings. Confirm the output path exists:

```bash
ls target/doc/flui_platform/index.html
```

Expected: the file exists (the crate produced a doc page from the module-level comment). If `missing_docs` warnings fire, you added a `pub` item by accident — revisit Task 2.

- [ ] **Step 4: Run `cargo check --workspace --all-targets --all-features`**

```bash
cargo check --workspace --all-targets --all-features 2>&1 | tail -30
```

Expected: exit 0. This catches any feature-gating surprises even though S02a touches no feature flags.

---

### Task 6: Confirm lock-checks are unchanged

**Files:**
- None. This task runs the workspace's lock-check tooling to confirm S02a introduces zero drift in the committed inventories.

- [ ] **Step 1: Run `check-stubs`**

```bash
cargo run -p lock-checks -- check-stubs 2>&1 | tail -20
```

Expected: exit 0 with a "no drift" / "inventory matches" style message. The exact wording depends on S01a.1's implementation — whichever message means "baseline matches committed inventory" is the one to look for.

If the command prints a diff, S02a has inadvertently added a stub site. The empty `flui-platform/src/lib.rs` contains no `unimplemented!()`, `todo!()`, or `unreachable!()` calls, so this should not happen — if it does, inspect the output and correct Task 2.

- [ ] **Step 2: Run `check-platform-imports`**

```bash
cargo run -p lock-checks -- check-platform-imports 2>&1 | tail -20
```

Expected: exit 0, no diff. The check scans `crates/flui-core/src/platform/**` for `use crate::*;` / `use flui_core::*;` globs — S02a does not touch those files, so the output must match the committed baseline byte-for-byte.

If a diff appears, you have edited a file in `crates/flui-core/src/platform/` by mistake. Revert that edit.

---

### Task 7: Update the roadmap document for the S02 → {S02a, S02b} split

**Files:**
- Modify: `docs/superpowers/specs/2026-04-13-flui-core-roadmap.md` — three separate regions (ordered spec table, dependency diagram, narrative).

- [ ] **Step 1: Replace the S02 row in the ordered spec table**

Open `docs/superpowers/specs/2026-04-13-flui-core-roadmap.md` and locate the row at line 398:

```markdown
| **S02** | `flui-platform-crate-skeleton` | S01a.1, S01a.2, S01a.3, S01a.4, S01b, S01c, S01d | Create `crates/flui-platform`, move `test/` + `visual_test.rs`, set up re-exports for backwards compatibility. |
```

Replace it with these **two** rows (in the same place in the table):

```markdown
| **S02a** | `flui-platform-crate-skeleton` | S01a.1, S01a.2, S01a.3, S01a.4, S01b, S01c, S01d | Create empty `crates/flui-platform` workspace member (minimal `Cargo.toml` + doc-only `lib.rs`). No file moves, no re-exports, no feature flags. |
| **S02b** | `flui-platform-trait-and-test-flip` | S02a | Move `Platform` trait family, supporting value types (`ClipboardItem`, `WindowParams`, `AnyWindowHandle`, `CursorStyle`, `Menu`, `Keymap`, `Brightness`, …), `platform/test/**`, `platform/visual_test.rs`, and `app/{test_context,test_app,visual_test_context,headless_app_context}.rs` in one coordinated commit. Update `flui-macros` test-macro code generation to emit `flui_platform::` paths. Re-export from `flui-core` to preserve the public surface. First introduction of the `flui-core → flui-platform` dependency edge. |
```

- [ ] **Step 2: Update the `Depends on` column on the S03 row**

Locate the S03 row (was line 399):

```markdown
| **S03** | `platform-migration-wgpu-linux` | S02 | Move `wgpu/` + `linux/{x11,wayland,headless}` + Linux target-deps + naga build.rs. |
```

Replace `S02` with `S02b` in the `Depends on` column:

```markdown
| **S03** | `platform-migration-wgpu-linux` | S02b | Move `wgpu/` + `linux/{x11,wayland,headless}` + Linux target-deps + naga build.rs. |
```

S04, S05, S06 do not need edits in this column — they reference their immediate predecessors (S03, S04, S05) which are unchanged.

- [ ] **Step 3: Update the ASCII dependency diagram**

Locate the diagram around lines 428–437:

```
         ┌─ S01a.2 ─┐
         ├─ S01a.3 ─┼─ S01d ─┐
S01a.1 ──┼─ S01a.4 ─┤         │
         ├─ S01b ───┤         │
         └─ S01c ───┴─────────┴─ S02 ─ S03 ─ S04 ─ S05 ─ S06 ─┬─ S07..S15 (parallelizable)
                                                               │
                                                               └─ S16 ─ (S17, S18, S19 parallel) ─ S20
```

Replace the `S02` node with `S02a ─ S02b`:

```
         ┌─ S01a.2 ─┐
         ├─ S01a.3 ─┼─ S01d ─┐
S01a.1 ──┼─ S01a.4 ─┤         │
         ├─ S01b ───┤         │
         └─ S01c ───┴─────────┴─ S02a ─ S02b ─ S03 ─ S04 ─ S05 ─ S06 ─┬─ S07..S15 (parallelizable)
                                                                       │
                                                                       └─ S16 ─ (S17, S18, S19 parallel) ─ S20
```

Watch the trailing `│` / `└─` lines — they were aligned to the column of `S16` in the original; re-align them so the new diagram is still visually connected. If alignment drifts by one or two spaces it is cosmetic and acceptable.

- [ ] **Step 4: Rewrite the "Step 1 — flui-platform skeleton + test platform" bullet**

Locate the bullet around lines 355–357:

```markdown
- **Step 1 — flui-platform skeleton + test platform.** (spec S02) Create the
  crate, move `test/` and `visual_test.rs` only. Smallest and safest because
  it has no native deps.
```

Replace it with two bullets that reflect the split:

```markdown
- **Step 1a — flui-platform skeleton.** (spec S02a) Create an empty
  `crates/flui-platform` workspace member with a minimal `Cargo.toml` and
  a doc-only `lib.rs`. No source moves, no re-exports, no dependency edge
  from `flui-core` yet. The smallest possible unblocking commit.
- **Step 1b — Platform trait + test backend flip.** (spec S02b) Move the
  `Platform` trait family, all supporting value types referenced by trait
  method signatures, `platform/test/**`, `platform/visual_test.rs`, and
  the `app/{test_context,test_app,visual_test_context,headless_app_context}.rs`
  test scaffolding into `flui-platform` in one coordinated commit. First
  introduction of the `flui-core → flui-platform` dependency edge. Update
  `flui-macros` to emit `flui_platform::` paths in generated test-harness
  code. Re-export from `flui-core` to preserve the public surface.
```

- [ ] **Step 5: Add a paragraph in "Rejected strategies" explaining the split**

Locate the "Rejected strategies" section around lines 372–379:

```markdown
### Rejected strategies

- **Big bang.** Highest chance of silent functionality loss; rollback is
  impractical.
- **Parallel dual-track.** Creates duplicated trait paths, confuses IDEs and
  clippy, and still has the same rollback problem at cut-over.
- **Asymmetric (desktop stays in core).** Permanent technical debt; new
  contributors won't know where a platform lives.
```

Add one more bullet at the end of that list:

```markdown
- **Original S02 scope (create crate + move `platform/test/**` + `visual_test.rs` in one step).**
  Ruled out by adversarial review (see spec S02a `Context` section). Three
  independent Cargo dependency cycles proved the move structurally
  impossible in a single commit: `PlatformWindow::as_test` and
  `PlatformDispatcher::as_test` return concrete test types by reference;
  `VisualTestPlatform` implements `Platform` while holding `Rc<dyn Platform>`;
  and `TestPlatform` references non-platform `flui-core` types like
  `Brightness`. Resolving any of these requires moving the whole trait
  family plus its supporting value types, which exceeds the S01a-split
  single-reviewable-commit budget. Hence the S02 → {S02a, S02b} split.
```

- [ ] **Step 6: Stage and inspect the roadmap diff**

```bash
git add docs/superpowers/specs/2026-04-13-flui-core-roadmap.md
git diff --cached docs/superpowers/specs/2026-04-13-flui-core-roadmap.md
```

Expected: four distinct regions of change — the ordered spec table (one row deleted, two added), the S03 row (one word changed), the ASCII diagram (one node expanded), the "Step 1" bullet (rewritten), and the "Rejected strategies" list (one bullet added). No other lines touched.

---

### Task 8: Final verification — everything green with the roadmap edit staged

**Files:**
- None.

- [ ] **Step 1: Re-run the full suite now that all changes are staged**

```bash
cargo check --workspace 2>&1 | tail -10 \
 && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10 \
 && cargo test --workspace 2>&1 | tail -10 \
 && cargo doc -p flui-platform --no-deps 2>&1 | tail -5
```

Expected: all four commands exit 0. If any step fails, fix inline — do not proceed to commit.

- [ ] **Step 2: Re-run lock-checks**

```bash
cargo run -p lock-checks -- check-stubs 2>&1 | tail -10 \
 && cargo run -p lock-checks -- check-platform-imports 2>&1 | tail -10
```

Expected: both exit 0, no drift.

- [ ] **Step 3: Confirm workspace metadata sees `flui-platform`**

```bash
cargo metadata --format-version 1 --no-deps 2>/dev/null | grep '"name":' | grep flui
```

Expected: the output includes `"name": "flui-platform"` alongside `flui-core`, `flui-macros`, `flui-widgets`, `flui-animate`, `flui-navigator`, `flui-a11y`, `flui-theme`, `flui-material`, and `lock-checks`.

- [ ] **Step 4: Confirm the staged change set is exactly what S02a promises**

```bash
git status --short
```

Expected exactly (order may vary, ignore `.serena/`):

```
M  Cargo.toml
M  Cargo.lock
A  crates/flui-platform/Cargo.toml
A  crates/flui-platform/src/lib.rs
M  docs/superpowers/specs/2026-04-13-flui-core-roadmap.md
```

If `Cargo.lock` is not staged, stage it now:

```bash
git add Cargo.lock
```

If any file outside that set is modified (especially anything under `crates/flui-core/src/`), STOP and revert the stray change. S02a must not touch flui-core.

---

### Task 9: Commit

**Files:**
- None; this task creates the single landing commit.

- [ ] **Step 1: Create the commit**

```bash
git commit -m "$(cat <<'EOF'
feat(workspace): add empty flui-platform crate skeleton (S02a)

Create crates/flui-platform as a new workspace member with a minimal
Cargo.toml (workspace-inherited metadata, no dependencies, no feature
flags) and a doc-only src/lib.rs. Register the crate in the workspace
members list immediately after flui-core.

Also split the roadmap's original S02 entry into S02a (this commit)
and S02b (the coordinated trait-and-test flip). Adversarial review from
three specialized agents established that the original S02 scope was
structurally impossible as a single reviewable commit because of three
independent Cargo dependency cycles between flui-core and a naively
populated flui-platform: trait-return methods (PlatformWindow::as_test,
PlatformDispatcher::as_test), VisualTestPlatform impling Platform, and
TestPlatform referencing crate::Brightness. Resolving the cycles
requires moving the whole Platform trait family plus supporting value
types in one atomic step; that is now S02b.

This commit touches no flui-core file and adds no public API. It is a
pure scaffolding step that unblocks S02b without risking any existing
behavior. Reverting is a single git revert with no downstream fallout.

Spec: docs/superpowers/specs/2026-04-13-S02a-flui-platform-crate-skeleton-design.md
Plan: docs/superpowers/plans/2026-04-13-S02a-flui-platform-crate-skeleton.md

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected: commit created. Output contains something like `[main <sha>] feat(workspace): add empty flui-platform crate skeleton (S02a)` followed by `5 files changed, …`.

- [ ] **Step 2: Verify post-commit state**

```bash
git log -1 --stat
```

Expected: the commit lists exactly these files:

```
 Cargo.lock                                                                         | ...
 Cargo.toml                                                                         | 1 +
 crates/flui-platform/Cargo.toml                                                    | 19 +++
 crates/flui-platform/src/lib.rs                                                    | 35 +++++
 docs/superpowers/specs/2026-04-13-flui-core-roadmap.md                             | ...
```

Line counts will vary; the important invariant is that no file outside this set appears. If any extra file is listed, `git reset --soft HEAD^`, unstage the stray file, and recommit.

- [ ] **Step 3: Final smoke**

```bash
cargo build --workspace 2>&1 | tail -5
```

Expected: exit 0. This is a paranoid last check that the committed state actually builds — catches the case where a dirty local `target/` hid a problem in an earlier step.

---

## Done criteria

S02a is complete when:

- [ ] All of Task 1–9 are checked off.
- [ ] `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo doc -p flui-platform --no-deps`, `cargo check --workspace --all-targets --all-features` all exit 0 locally on the committed state.
- [ ] `cargo run -p lock-checks -- check-stubs` and `cargo run -p lock-checks -- check-platform-imports` both exit 0 with no drift.
- [ ] `cargo metadata --no-deps` includes `flui-platform` as a workspace member.
- [ ] The commit touches exactly five files: `Cargo.toml` (root), `Cargo.lock`, `crates/flui-platform/Cargo.toml` (new), `crates/flui-platform/src/lib.rs` (new), `docs/superpowers/specs/2026-04-13-flui-core-roadmap.md` (modified).
- [ ] `git revert HEAD` would return the tree to the exact pre-S02a state (this is implied by the single-commit invariant, not re-run).
- [ ] Nothing under `crates/flui-core/` has been modified.

## Rollback

If CI on the landing branch surfaces a problem that can't be fixed forward in minutes:

```bash
git revert <s02a-commit-sha>
```

No further cleanup is required — the reverted state is byte-for-byte identical to pre-S02a.

## Follow-up

After S02a lands and CI is green on all three OSes, the next spec is **S02b (`flui-platform-trait-and-test-flip`)**. Do not start S02b in the same PR or the same session — the whole point of splitting S02 was to make S02a independently reviewable. S02b gets its own brainstorming → spec → plan → implementation cycle.
