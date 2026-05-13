# PLAN: A10a PR 1.0 — WindowCore foundation + explicit `pub use window::*`

**Type:** structural refactor (API-neutral)
**Branch:** `confident-agnesi-b010fe`
**Created:** 2026-05-13
**Spec:** `docs/superpowers/specs/2026-05-13-A10-xl-file-split-design.md`
**Policy:** `docs/research/adr/ADR-021-xl-file-split-discipline.md`
**Roadmap entry:** `.ai-factory/ROADMAP.md` → Architecture & API hygiene → A10

## Settings

- **Testing:** yes — focused invariant test for `Rc::ptr_eq` preservation; no broader test additions in this PR (Decision D10).
- **Logging:** verbose during implementation; no committed runtime logs added (this is a pure structural move).
- **Docs:** no — spec/ADR/ROADMAP entries already committed (`80baa9f15a`). `/aif-implement` should emit `WARN [docs] Docs policy is no; skipping documentation checkpoint`.

## Roadmap Linkage

- **Milestone:** A10a (sub-track of A10 XL-file decomposition).
- **Rationale:** PR 1.0 lands the `WindowCore` foundation pattern + closes 1 of ~29 globs from A2 audit synergy (`pub use window::*`). All subsequent A10a PRs (1.1 → 1.11) depend on this foundation.

## Hard checklist (from spec § Migration Plan / PR 1.0)

PR 1.0 is **hard-blocked** until ALL of:

- [a] Full pub-surface inventory generated via `cargo public-api -p flui-core > /tmp/before.txt` or manually documented (Appendix A of spec is the authoritative reference list).
- [b] `cargo public-api diff main..HEAD` empty (`/tmp/pr10-pub-api-diff.txt` archived).
- [c] `Rc::ptr_eq` semantics for `active`, `needs_present`, `input_rate_tracker` preserved (no heap-relocation via `Box`/`Arc` around `WindowCore`).
- [d] `DrawPhase` stays `pub(crate)` — NOT promoted to `pub` (Decision D11).
- [e] `cargo build -p flui-core --no-default-features` + `cargo build -p flui-core` both green.
- [f] `flui-framework` + workspace examples compile unchanged (K91 contract).

## Tasks

### Phase 1 — Inventory and scaffolding (no behaviour change)

- [x] **Task #7** — Generate baseline API inventory snapshot ✅
  - Used `cargo public-api -p flui-core --simplified` (v0.51.0).
  - Filtered 113,019 `pub` lines into `.ai-factory/plans/artifacts/pr10-baseline-pub-api-filtered.txt`.
  - Cross-checked all 31 Appendix A symbols (window-direct + prompts chain) — all present at `flui_core::*`. No divergence.
- [x] **Task #8** — Scaffold `crates/flui-core/src/window/` directory with empty `core.rs` ✅
  - Created `window/core.rs` with module-level rustdoc (Contract from ADR-021 Practice 1 spelled out: no Deref, no Box/Arc, plain field, `pub(super)`).
  - Added `mod core;` in `window.rs` line 64 after `mod prompts;`.
  - Discovered + fixed name-shadow: `mod core` shadows stdlib `core` crate. Changed `core::panic::Location` → `::core::panic::Location` in `window.rs:3527` (only occurrence). Defensive note added in `window/core.rs` for future contributors.
  - `cargo check -p flui-core` green.

### Phase 2 — Extract WindowCore foundation

- [ ] **Task #9** — Extract `WindowCore` struct in `window/core.rs`
  - Move all ~140 fields from `pub struct Window` into `pub(super) struct WindowCore`.
  - **Plain field**: `pub struct Window { pub(super) core: WindowCore }` — NO `Deref<Target = WindowCore>`.
  - **Embed by value**: NO `Box<WindowCore>` / `Arc<WindowCore>`.
  - Update `Window::new` constructor to build `WindowCore` first, then wrap.
  - `cargo check -p flui-core` **will fail** after this — that's expected; Task 10 fixes call sites.
- [ ] **Task #10** — Migrate every `impl Window` field access to `self.core.<field>`
  - Mechanical rewrite driven by `cargo check` errors.
  - Add focused `#[cfg(test)] mod tests` in `core.rs` proving `Rc::ptr_eq` invariant for `active`/`needs_present`/`input_rate_tracker`.
  - `cargo build -p flui-core` + `cargo build -p flui-core --no-default-features` both green.

### Phase 3 — Public API surface

- [ ] **Task #11** — Rewrite `pub use window::*` in `lib.rs:317` to explicit per-symbol
  - Source of truth: `.ai-factory/plans/artifacts/pr10-baseline-pub-api.txt` (Task 7 artifact).
  - **`DrawPhase` is NOT in the explicit list** (stays `pub(crate)`).
  - Explicitly re-export the `prompts::*` chain (`PromptResponse`, `Prompt`, `PromptHandle`, `RenderablePromptHandle`, `FallbackPromptRenderer`, `fallback_prompt_renderer`).
  - Inline comment references spec Appendix A and ADR-021.

### Phase 4 — Verification

- [ ] **Task #12** — Verify cargo build/test matrix across feature/target configs
  - `cargo fmt --check`, `cargo build` (default + `--no-default-features`), `cargo test --workspace`, `cargo build -p flui-framework`, `cargo build --examples`, `cargo doc -p flui-core --no-deps`, `cargo clippy`.
  - Paste each result into PR description.
- [ ] **Task #13** — `cargo public-api diff main..HEAD` (zero breaking change)
  - Install `cargo-public-api` if needed.
  - Diff must be empty. If not, iterate Tasks 11 → 12 → 13.
  - Archive diff (or "empty" confirmation) to `.ai-factory/plans/artifacts/pr10-pub-api-diff.txt`.

### Phase 5 — Review gate

- [ ] **Task #14** — Pre-PR triple-launch reviewer trio (in one message, parallel)
  - `flui-arch-reviewer` — arch consistency, WindowCore boundary, K06 supersession.
  - `migration-risk-adversary` — silent regressions from field-location moves.
  - `rust-api-migration-auditor` — public API diff, auto-trait stability, feature flag combinatorics.
  - Fold CRIT findings; document IMP/MINOR in PR description.
  - **Do NOT merge** until all three return green or all blockers addressed.

## Commit Plan (revised — every commit must build clean)

| Commit | Tasks | Buildable? | Suggested message |
|---|---|---|---|
| 1 | #7, #8 | ✅ green | `refactor(window): scaffold window/core.rs module (A10a PR 1.0 phase 1)` |
| 2 | #9, #10 | ✅ green | `refactor(window): extract WindowCore + migrate impl blocks (A10a PR 1.0 phase 2)` |
| 3 | #11 | ✅ green | `refactor(core): explicit per-symbol re-export for pub use window::* (A10a PR 1.0 + A2 synergy)` |

Verification tasks (#12, #13, #14) produce artifacts and PR description content but no source commits. Reviewer fixes (if any) get squashed into the appropriate commit via `git commit --amend` or rebase.

**Rationale for combining #9+#10**: Task #9 alone (struct extraction) leaves `cargo check` failing because all existing `impl Window` methods reference fields via `self.<field>` which no longer exists at that level. Task #10 fixes those by rewriting to `self.core.<field>`. Splitting into two commits would leave a non-buildable intermediate state — bad git hygiene + bisect-unfriendly.

**Type marker**: this PR is API-neutral; do NOT use `!` breaking marker. `cargo public-api diff` (Task #13) is the final arbiter.

## Risks (carried over from spec § Risks)

- **CRIT #1 — K06 conflict**: K06's `BuildOwner/PipelineOwner/SemanticsOwner` model supersedes `WindowCore`. PR 1.0 commits explicitly say "transient scaffold; K06 will redesign". K06 ROADMAP entry already updated with `Blocked-on: A10a`.
- **CRIT #16 — DrawPhase visibility**: Must remain `pub(crate)`. Task 11 acceptance criterion includes explicit verification.
- **CRIT #17 — No Deref**: Forbidden by ADR-021 Practice 1. Task 9 acceptance criterion enforces.
- **IMP #5 — Rc::ptr_eq**: Task 10 includes the invariant test.
- **IMP #6 — DivInspectorState**: Out of scope (PR 3.5). Mentioned only as future context.

## Out of scope (NOT this PR)

- Submodule moves (focus, hitbox, paint, layout, draw, etc.) — those are PR 1.1 through PR 1.11.
- `test_fixtures.rs` shared helpers — PR 1.2.
- `pub use elements::*` rewrite — D13 says it stays glob; out of scope for A10.
- `pub use geometry::*` rewrite — stays glob (Practice 4); out of scope.
- `cargo-public-api` / `cargo-semver-checks` CI integration — R-track follow-up.

## Next steps after PR 1.0 merges

1. ADR-021 status flips `Proposed → Accepted` (pattern validated in practice).
2. Open PR 1.1 (`window/handle.rs` — WindowHandle/AnyWindowHandle move).
3. K06 redesign spec — define `BuildOwner/PipelineOwner/SemanticsOwner` decomposition that replaces `WindowCore`.
