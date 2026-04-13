---
spec_id: S01a.1
title: lock-inventory-and-hygiene
phase: I
depends_on: []
blocks: [S01a.2, S01a.3, S01a.4, S01b, S01c]
status: draft
date: 2026-04-13
---

# S01a.1 — lock-inventory-and-hygiene

## Context

First atomic sub-step of the lock phase (see
[roadmap](2026-04-13-flui-core-roadmap.md)). S01a was originally drafted as
a single spec bundling nine unrelated hygiene tasks; adversarial review from
four expert agents caught factual errors and blast-radius concerns, and the
user approved splitting it into four smaller specs. S01a.1 is the smallest
and least risky of those: pure tooling and CI infrastructure, no runtime
code changes, no API changes, no rendering changes.

It sets up the ground-truth machinery that S01a.2 (delete dead
screen-capture code), S01a.3 (explicit re-export list), and S01a.4 (repair
debug Windows build) will all consume.

## Goals

1. Make stub-site drift detectable by CI via a new
   `cargo xtask check-stubs` subcommand that lives alongside the existing
   `package_conformity` task.
2. Produce an authoritative `use crate::*;` / `use flui_core::*;` import
   survey inside `platform/**` via a new `cargo xtask check-platform-imports`
   subcommand, emitting a markdown report.
3. Add a `.gitattributes` rule that forces LF line endings on
   `docs/fixtures/*.h` so that the `scene.h` snapshot committed in a later
   spec (S01b) cannot be silently corrupted by Windows contributors.
4. Measure the runtime impact of enabling `--features test-support` in
   `cargo test --workspace` on Linux and macOS, and decide whether to flip
   the CI default. Record the measurement in the spec's test log.

## Non-goals

- No changes to `crates/flui-core/src/**` runtime code.
- No changes to `crates/flui-core/Cargo.toml` features. The `test-support`
  flip is a CI-only change if it happens at all.
- No new pub items in flui-core.
- No new CI matrix entries. Only modifications to existing jobs.
- No golden tests, no rendering refactor, no Windows build repair.
- No changes to `flui-platform` (doesn't exist yet).
- Does NOT commit `docs/fixtures/scene.h` itself — only the
  `.gitattributes` rule that protects it. The actual fixture lands in S01b.

## Current state

- `tooling/xtask/` is a workspace member with existing subcommands: `clippy`,
  `licenses`, `package_conformity`, `publish-gpui`, `web-examples`,
  `workflows`, `check-workflows`. Pattern is one file per task under
  `tooling/xtask/src/tasks/`, one arg struct + one `run_*` function, one
  `CliCommand` variant at [`main.rs:16-25`](../../tooling/xtask/src/main.rs#L16-L25).
- `.gitattributes` currently contains a single rule
  (`*.json linguist-language=JSON-with-Comments`). No line-ending rules.
- `.github/workflows/ci.yml` matrix is `ubuntu-latest + macos-latest`, and
  the `test` job at line 118 runs `cargo test --workspace` without
  `--features test-support`. The consequence is that any test gated on
  `test-support` is silently skipped in CI today.
- No existing tooling or CI job enumerates platform stub sites or the
  `use crate::*;` globs.

### Real stub counts (verified via grep today)

- **`unimplemented!()` in `crates/flui-core/src/platform/**` — 22 sites
  across 5 files** (earlier drafts of the roadmap cited 24; one file was
  miscounted):
  - `platform/test/platform.rs` — 12 sites (lines 263, 279, 283, 287, 352,
    359, 380, 386, 401, 405, 457, 461).
  - `platform/test/window.rs` — 6 sites (lines 45, 53, 234, 238, 242, 313,
    317, 321). **NOTE**: earlier analysis claimed 8; `rg` returns 8 lines
    but two of them are inside comments/doc-strings, not live calls. S01a.1
    records whichever number the xtask subcommand actually measures as
    ground truth.
  - `platform/windows/platform.rs` — 2 sites (lines 474, 479) — dock-menu
    methods (Windows has no dock).
  - `platform/windows/window.rs` — 1 site (line 538) — dock-menu.
  - `platform/mac/metal_atlas.rs` — 1 site (line 246) — rare texture
    format.
- **`unreachable!()` in `crates/flui-core/src/platform/**` — 18 sites
  across 9 files** (earlier draft cited 12):
  - `linux/wayland/client.rs` (4), `linux.rs` (1),
    `mac/metal_renderer.rs` (1 — `SubpixelSprites` guard on mac),
    `mac/metal_atlas.rs` (5 — subpixel-kind guards),
    `mac/platform.rs` (1 — `CursorStyle::None` trap),
    `windows/directx_devices.rs` (2), `windows/direct_write.rs` (1),
    `windows/platform.rs` (1), `app_menu.rs` (2).
- Comment-level `// TODO` markers: ~26 across platform/ tree. Not tracked
  by the xtask check.
- `crates/flui-core/src/platform/**/shaders.{wgsl,hlsl,metal}` may contain
  their own TODO markers. S01a.1 explicitly scopes the check to Rust source
  (`*.rs`) only; shader TODOs are addressed in S01b and S01a.4 as part of
  their respective domain.
- `key_dispatch.rs` (4 sites) and `action.rs` (3 sites) have
  `unimplemented!()` too but are **outside platform/**. S01a.1 scopes the
  stub check to `platform/**` only and names the scope in the subcommand
  documentation; widening to the whole crate is deferred to a later spec.

### `use crate::*;` / `use flui_core::*;` survey (live grep)

Inside `crates/flui-core/src/platform/**`, 12 glob-imports across 6 files.
Every one of the six Windows files carries **both** a `use crate::*;` and a
`use flui_core::*;` on adjacent lines:

- `windows/direct_write.rs:26-27`
- `windows/directx_renderer.rs:23-24`
- `windows/window.rs:29-30`
- `windows/util.rs:17-18`
- `windows/platform.rs:30-31`
- `windows/events.rs:20-21`

No glob-imports in mac/ or linux/.

### test-support feature shape

From `Cargo.toml:15-23`:

```toml
test-support = [
    "leak-detection",
    "backtrace",
    "collections/test-support",
    "util/test-support",
    "http_client/test-support",
    "wayland",
    "x11",
]
leak-detection = ["backtrace"]
```

On Linux, `wayland` and `x11` are already in the default feature set
(line 14), so the flip is a no-op for those. On macOS, `wayland` and `x11`
resolve to optional deps declared under
`[target.'cfg(any(target_os = "linux", target_os = "freebsd"))'.dependencies]`
and are silently no-op'd by Cargo (not a compile error).

The real question is runtime impact of `leak-detection` + `backtrace` — they
enable backtrace capture on allocation paths and can slow test suites
substantially. S01a.1 measures the delta, records it, and decides.

## Design

### 1. `cargo xtask check-stubs`

New file: [`tooling/xtask/src/tasks/check_stubs.rs`](../../tooling/xtask/src/tasks/check_stubs.rs).
Follows the `package_conformity` pattern: args struct, `run_*` function,
returns `Result<()>`, prints findings to stderr, exits non-zero on drift.

**Command shape:**

```
cargo xtask check-stubs         # verify against committed fixture
cargo xtask check-stubs --bless # rewrite the fixture to match the tree
```

**Implementation:**

1. Walk `crates/flui-core/src/platform/**/*.rs` (not
   shaders — that's S01b's problem; not top-level core — that's a later
   spec).
2. Skip any file whose path contains `target/`, `.serena/`, `node_modules/`,
   or lives outside the workspace (defensive — also ensures that a
   developer's local caches don't contaminate the count).
3. For each file, count `unimplemented!(`, `unreachable!(`, and `todo!(`
   patterns via simple byte-level search (no Rust parser — same approach as
   `package_conformity`). Match opening-paren to distinguish macro calls
   from type references.
4. Build a `BTreeMap<PathBuf, StubCounts>` where `StubCounts` is
   `{ unimplemented: usize, unreachable: usize, todo: usize }`. BTreeMap
   for stable iteration.
5. Load the fixture from
   `docs/fixtures/platform-expected-stubs.toml`. Compare the live map to
   the fixture.
6. On drift, print each changed file with old → new counts and exit 1.
7. With `--bless`, overwrite the fixture file and exit 0.

**Fixture format** — `docs/fixtures/platform-expected-stubs.toml`:

```toml
# Auto-generated by `cargo xtask check-stubs --bless`.
# Edit intent:
#   When you intentionally add or remove a stub, run
#   `cargo xtask check-stubs --bless` and commit the new fixture in
#   the same commit as the code change. CI fails on any drift.
#
# Scope: crates/flui-core/src/platform/**/*.rs only. Does not track
# shaders, top-level flui-core modules, build.rs, or sibling crates.

[[stub]]
path = "crates/flui-core/src/platform/test/platform.rs"
unimplemented = 12
unreachable = 0
todo = 0

[[stub]]
path = "crates/flui-core/src/platform/test/window.rs"
unimplemented = 6  # or 8 — live count wins; the check writes what it sees
unreachable = 0
todo = 0

# ... etc for every file with at least one match
```

**Known blind spots** (explicitly acknowledged in the doc comment of the
subcommand):
- Intra-file stub moves (renaming the function holding the `unimplemented!()`
  doesn't change the count).
- Macro swap (`unimplemented!()` → `panic!("not implemented")` or
  `std::process::abort()`): not caught.
- Counterbalanced add + delete in the same PR: not caught.
- Shader files: explicitly out of scope.

These are documented, not fixed. The purpose of the check is a **drift
detector**, not a regression prover — a distinction S01a.1 makes explicit
per review feedback.

### 2. `cargo xtask check-platform-imports`

New file: [`tooling/xtask/src/tasks/check_platform_imports.rs`](../../tooling/xtask/src/tasks/check_platform_imports.rs).

**Command shape:**

```
cargo xtask check-platform-imports         # produce report to stdout
cargo xtask check-platform-imports --emit  # also write docs/reports/platform-imports.md
```

**Implementation:**

1. Walk `crates/flui-core/src/platform/**/*.rs`.
2. For each file, look for lines matching
   `^use (crate|flui_core)(::[a-z_]+)?::\*;` (regex-light — byte-level
   scan for `use crate::` or `use flui_core::` followed by `*;`).
3. For each match, record `(file, line, import-expression)`.
4. Emit a markdown table to stdout. With `--emit`, also write
   `docs/reports/platform-imports.md` atomically.

Purpose: the committed report is **informational only**. It answers "which
files currently rely on glob imports" so that S01a.4 and S02 reviewers can
see the scope. S01a.1 does NOT promise to keep the report up to date with
every PR; it's a snapshot the later specs consume.

### 3. `.gitattributes` rule

Append to `.gitattributes`:

```
# Generated shader ABI snapshots must stay LF to avoid Windows CRLF noise.
docs/fixtures/*.h text eol=lf

# Stub inventory is generated; diff noise minimized by pinning line endings.
docs/fixtures/*.toml text eol=lf
```

This is a three-line change. The `docs/fixtures/` directory does not exist
yet; it will be created when S01a.1 commits the stub fixture.

### 4. `test-support` runtime benchmark + CI decision

S01a.1 does NOT unconditionally flip the CI job to
`cargo test --workspace --features test-support`. Instead it:

1. **Measures locally on Linux + macOS** (developer runs the benchmark, not
   CI):
   ```
   hyperfine --warmup 1 --min-runs 3 \
     'cargo test --workspace' \
     'cargo test --workspace --features test-support'
   ```
2. **Records the result in this spec's "Test log" section** (see below, to
   be filled during implementation).
3. **Decides based on the delta**:
   - If `--features test-support` is within 20% runtime overhead → flip the
     CI default to `--features test-support` in all three `check`, `clippy`,
     `test` jobs. Cost is acceptable; benefit is that `test-support`-gated
     tests actually run.
   - If overhead exceeds 20% → keep the default as-is, add a separate CI
     job `test-with-test-support` that runs only on Linux (cheapest runner)
     to exercise the gated tests without slowing the main pipeline. Document
     the measurement as the reason.
4. **Commits the CI change (or the decision to defer)** in the same commit
   as the xtask subcommands, along with a note in the spec's test log.

The decision is recorded in this spec and closes the open question from the
roadmap.

### 5. CI wiring

Single addition to `.github/workflows/ci.yml`: a new step in the `test` job
(and only `test`) that runs the new xtask check. Added after the existing
`cargo test` step:

```yaml
- name: Check stub inventory drift
  if: runner.os == 'Linux'  # one OS is enough for file-level drift
  run: cargo xtask check-stubs
```

No change to `check` or `clippy` jobs — they don't need to run the xtask
check.

The `test-support` feature flip (if the benchmark supports it) is a
separate one-line change in the same commit to the `test` job's `cargo test`
invocation.

## API surface

**Zero** new public items in `flui-core`. All new code lives in
`tooling/xtask/`, which is a binary crate with no public API.

New files:
- `tooling/xtask/src/tasks/check_stubs.rs`
- `tooling/xtask/src/tasks/check_platform_imports.rs`
- `docs/fixtures/platform-expected-stubs.toml` (auto-generated)
- `docs/reports/platform-imports.md` (auto-generated, informational)

Modified files:
- `tooling/xtask/src/main.rs` — two new `CliCommand` variants, two new
  match arms.
- `tooling/xtask/src/tasks.rs` — `pub mod check_stubs; pub mod check_platform_imports;`
- `.gitattributes` — three lines added.
- `.github/workflows/ci.yml` — one new step (maybe two if the benchmark
  allows the test-support flip); modifications to existing `cargo test`
  invocation if the flip happens.

## Migration / Compatibility

Nothing in `flui-core` or sibling crates changes. No breaking change.
Anything currently importing `flui_core::*` keeps working identically.

The xtask binary gains two subcommands that did not exist before. No
existing subcommand's arguments or behavior change.

The `.gitattributes` change applies only to file paths that don't exist
yet (`docs/fixtures/*.h`, `docs/fixtures/*.toml`). Existing committed files
are unaffected.

## Testing strategy

1. **`cargo xtask check-stubs` unit test**: construct a small fake tree
   in-memory (or in `target/tmp`), seed it with `unimplemented!()`
   occurrences, verify the command counts correctly. At least one test per
   stub kind.
2. **`cargo xtask check-stubs` against real tree**: run it with `--bless`
   locally; commit the output; verify `cargo xtask check-stubs` (without
   `--bless`) exits 0 on the committed tree.
3. **`cargo xtask check-platform-imports` smoke test**: run it with
   `--emit`; verify the output file exists and contains the 12 known glob
   sites.
4. **CI exercise**: the new `Check stub inventory drift` step in the `test`
   job must pass green on first run.
5. **test-support benchmark log**: recorded in this spec's test log
   section, with timing deltas for Linux and macOS.

## Open questions

- **Exact count of `test/window.rs` stubs** — 6 or 8? The live grep will
  resolve this at implementation time. Not a blocker; the first `--bless`
  run records ground truth.
- **Does the `test-support` flip change `-Dwarnings` behavior?** CI uses
  `RUSTFLAGS: -Dwarnings` globally. Enabling `test-support` may surface new
  warnings in previously-uncompiled code paths, which then fail the build.
  Mitigation: run `cargo clippy --workspace --features test-support` as
  part of the benchmark, capture any new warnings, and either fix them in
  S01a.1 (if trivial) or file a follow-up in this spec's open questions.
- **Should `check-stubs` also cover `crates/flui-core/src/**/*.rs` outside
  `platform/`?** Currently scoped to platform/ only. `key_dispatch.rs` +
  `action.rs` have 7 more sites. Deferring to a later spec keeps S01a.1
  small; widening the scope is a one-line change to the walker and can
  happen once S01a.1 has proven itself.
- **`cargo test --workspace --features test-support` on macOS linker** —
  per API review, this is *probably* no-op-safe because wayland/x11 are
  target-gated. But the benchmark is also the functional verification —
  if it doesn't link on mac, we learn that here rather than in S01b.

## Done criteria

- [ ] `cargo xtask check-stubs` exists and exits 0 against the current
      tree.
- [ ] `cargo xtask check-stubs --bless` has been run; fixture committed at
      `docs/fixtures/platform-expected-stubs.toml`.
- [ ] `cargo xtask check-platform-imports --emit` has been run; report
      committed at `docs/reports/platform-imports.md` with the 12 glob
      sites documented.
- [ ] `.gitattributes` has the three new lines.
- [ ] `test-support` benchmark has been run on Linux + macOS; timing delta
      recorded in the Test Log section of this spec.
- [ ] CI `test` job has the new `Check stub inventory drift` step (Linux
      only) and it's green.
- [ ] CI `cargo test` invocation is either unchanged (with rationale) or
      flipped to `--features test-support` (with rationale).
- [ ] `cargo xtask check-stubs` returns non-zero on a fake drift (add a
      stub, re-run, confirm failure, revert).
- [ ] `cargo check -p flui-core` still passes on Linux + macOS.
- [ ] Commit is a single atomic PR touching only the files listed in the
      API surface section.
- [ ] No new pub items in `flui-core`.

## Test log

To be filled during implementation.

### test-support benchmark

| Target | `cargo test --workspace` | `cargo test --workspace --features test-support` | Delta |
|---|---|---|---|
| Linux (local) | TBD | TBD | TBD |
| macOS (local or CI) | TBD | TBD | TBD |

**Decision:** TBD

### Stub inventory first run

```
$ cargo xtask check-stubs --bless
# output captured here
```

### Grep-guard negative test

```
$ # Add temporary unimplemented!() somewhere in platform/
$ cargo xtask check-stubs
# Expected: exit 1 with diff
```

## Follow-ups after S01a.1 lands

Unblocked specs (by dependency graph):
- **S01a.2** — delete dead screen-capture code (uses the inventory to
  prove no sites are silently lost).
- **S01a.3** — explicit re-export list (uses the import survey to see what
  the Windows files actually consume).
- **S01a.4** — repair debug Windows build (uses the import survey as the
  starting point for expanding `use crate::*;` into explicit lists).
- **S01b** — wgpu headless + goldens (uses `docs/fixtures/` directory and
  `.gitattributes` for the `scene.h` fixture later).
- **S01c** — behavior pinning (no direct dep, but benefits from
  test-support being reliably enabled in CI).
