# K99 — MSRV bump to Rust 1.95+

**Branch:** `feature/K99-msrv-bump-1.95`
**Created:** 2026-05-08
**Phase:** 0-K (Kernel Cleanup) — first spec in the critical chain
**Type:** mechanical workspace-wide config change (no source-code logic changes)

## Settings

| Setting | Value | Rationale |
|---|---|---|
| Testing | yes | `cargo test --workspace` is the validation gate; no NEW tests authored — existing suite must pass on new MSRV |
| Logging | minimal | Config-only change. No app-level logging to add or modify |
| Docs | yes (mandatory checkpoint) | MSRV bumps update DESCRIPTION.md, AGENTS.md, rules/base.md, README.md, design spec — `/aif-docs` checkpoint enforced at completion |
| Roadmap linkage | linked | K99 in Phase 0-K critical chain |

## Roadmap Linkage

**Milestone:** K99 — MSRV bump to Rust 1.95+ (Phase 0-K Kernel Cleanup, critical chain)

**Rationale:** Per `.ai-factory/ROADMAP.md` Phase 0-K — K99 is the first spec in the critical chain because it unlocks Rust 1.95+ idioms (AFIT, RPITIT, edition-2024 lifetime captures, async closures, let-chains, lazy_cell, unsafe extern, `#[diagnostic::on_unimplemented]`) that subsequent K-specs will use. AFIT + RPITIT specifically enables `Widget::build(&self) -> impl Widget` without `Box<dyn>`, critical for the "no allocation on rebuild hot path" invariant in `.ai-factory/ARCHITECTURE.md`. Mechanical, single-PR change with no downstream consumer constraints (hard fork posture).

## Research Context

From `.ai-factory/RESEARCH.md` (Active Summary):

- **Hard fork posture** — flui-v2 is a hard fork of `gpui-ce`; no upstream-sync commitment, no semver compatibility with `gpui`. MSRV bump is unilateral.
- **Phase 0-K rationale** — 24+ structural issues in `flui-core` block a healthy Framework tier. Critical chain `K99 → K15 → K07 → K05 → K01 → K02 → K03 → K04` repays the debt sequentially.
- **K99 specifics** — single-PR mechanical bump in `Cargo.toml` + `rust-toolchain.toml`. No downstream consumers. CI matrix to gain explicit MSRV gate.
- **Unlocked idioms** — AFIT + RPITIT + edition-2024 lifetime captures (1.79–2024 ed, edition-2024 lifetime captures specifically), async closures (1.85), let-chains (1.88, 2024 ed), lazy_cell (1.80), unsafe extern (1.82), `#[diagnostic::on_unimplemented]` (1.78).
- **Open question** — pin exact version (reproducible) vs `stable` (auto-upgrades). Plan recommends pin exact.

## Current state (pre-K99)

| Aspect | State | Note |
|---|---|---|
| Workspace MSRV | `Cargo.toml:20` says `rust-version = "1.85"` | declarative |
| Per-crate MSRV inheritance | Only `flui-platform` declares `rust-version.workspace = true` | **other 11 workspace members omit it** — implicit MSRV per member |
| `rust-toolchain.toml` | absent | no developer-side pin |
| CI MSRV gate | absent — 5 jobs use `dtolnay/rust-toolchain@stable` | declarative MSRV not enforced |
| Clippy `msrv` field | absent in `clippy.toml` | MSRV-aware lints not active |
| `tooling/perf` workspace status | `Cargo.toml` declares `edition.workspace = true` but NOT in `[workspace] members` | classify in audit (Task 2) |
| `flake.nix` toolchain | uses `fenix.packages.${system}.latest` | auto-bumps; may diverge from MSRV |
| Doc references | `AGENTS.md:19, 118`, `DESCRIPTION.md:22, 68`, `rules/base.md` (TBD line) | scattered |
| README MSRV | unknown — task #10 audits |
| Workspace members count | **12 total**: 8 lib crates (`crates/*`), 3 examples (`examples/*`), 1 tooling (`tooling/lock-checks`) | one of them (`flui-platform`) already has rust-version.workspace; **11 still need it** |

## Tasks

### Phase 1 — Pre-flight

- [x] **Task 1.** Determine target Rust version and verify availability — `rustup update stable && rustc --version`. Target Rust 1.95+. If 1.95 not yet stable, pick latest available stable and document the gap. Capture which features become available at the chosen version. → **Done.** Active toolchain `stable-x86_64-pc-windows-msvc = rustc 1.95.0 (59807616e 2026-04-14)`. Target = 1.95.0.
- [x] **Task 2.** Audit workspace for blockers preventing the new MSRV — `cargo build --workspace --all-features` on the new toolchain BEFORE making any changes. Establish green baseline OR classify issues as fix-here vs split-spec. **Also classify `tooling/perf`** — declares `edition.workspace = true` but is NOT in root workspace members list; determine if oversight, intentional standalone, or broken inheritance. → **Done.** Workspace builds clean on Rust 1.95.0 (43.58s). 12 declared members compile. `tooling/perf` is orphan (not in `[workspace] members`, Cargo.toml references workspace deps but is not built by `--workspace`) — out of K99 scope; documented for K93 dead-code triage.

### Phase 2 — Workspace MSRV declaration

- [x] **Task 3.** Bump root `Cargo.toml` `rust-version` field from `"1.85"` to chosen target. → **Done.** Set to `"1.95"`.
- [x] **Task 4.** Create `rust-toolchain.toml` at repo root. Pin exact version + components (rustfmt, clippy) + profile (default). **Also document `flake.nix` interaction** — fenix `latest` vs MSRV-pinned divergence; recommend keeping divergence (Nix users get forward-compat radar) and adding a one-line comment to `flake.nix`. → **Done.** `rust-toolchain.toml` created with `channel = "1.95"`, header documents the divergence policy.
- [x] **Task 5.** Add `rust-version.workspace = true` to **all 11 inheriting workspace members**: 7 lib crates (`flui-a11y`, `flui-core`, `flui-macros`, `flui-material`, `flui-navigator`, `flui-theme`, `flui-widgets`) + 3 examples (`animation_demo`, `material_demo`, `nav_demo`) + 1 tooling (`tooling/lock-checks`). Currently only `flui-platform` declares it. → **Done.** Verified: 12/12 workspace members now declare `rust-version.workspace = true`.
- [x] **Task 17.** Add `msrv = "1.95"` (or chosen target) to `clippy.toml` — verified absent. Lint-level enforcement that complements declarative `rust-version`. Drift between `Cargo.toml` rust-version, `rust-toolchain.toml` channel, and `clippy.toml` msrv is the bug class this prevents. → **Done.** `msrv = "1.95"` added with cross-reference comment.
- [x] **Task 18.** Decide and document Cargo.lock policy after MSRV bump. Recommendation: FREEZE for K99 scope (no `cargo update`); Cargo.lock changes only if forced by `cargo build`. Document the chosen policy in design spec (Task 15) under Migration / Compatibility section. → **Done.** FREEZE policy adopted; `cargo metadata` confirmed no Cargo.lock changes required by the declarative bump alone.

### Phase 3 — CI gate

- [ ] **Task 6.** Update `.github/workflows/ci.yml` to introduce MSRV gate — add a dedicated `msrv-check` job pinned to the chosen version; keep existing 5 `@stable` jobs for forward-compat detection. Don't expand to full feature-powerset matrix here (R5 covers that separately).

### Phase 4 — Documentation sweep

- [ ] **Task 7.** Update `AGENTS.md` MSRV references (line 19 tech stack, line 118 agent rule). Add positive guidance for newly allowed idioms.
- [ ] **Task 8.** Update `.ai-factory/DESCRIPTION.md` MSRV references (line 22 tech stack, line 68 NFR). Reword the "target idioms of Rust 1.95+ when stable" line — once 1.95 is the MSRV that wording is stale.
- [ ] **Task 9.** Update `.ai-factory/rules/base.md` MSRV-related rules. Add positive idiom-prefer rules (AFIT/RPITIT for trait-object APIs, let-chains, lazy_cell over once_cell).
- [ ] **Task 10.** Update `README.md` MSRV references if present. Skip if no MSRV mention; do not fabricate.

### Phase 5 — Validation gates

- [ ] **Task 11.** `cargo build --workspace --all-features` — must compile clean.
- [ ] **Task 12.** `cargo test --workspace` — full suite green.
- [ ] **Task 13.** `cargo clippy --workspace --all-targets -- -D warnings` — zero warnings. New lints from newer Rust handled deliberately, no blanket `#[allow]`.
- [ ] **Task 14.** `cargo fmt --all -- --check` — format gate. If new rustfmt drift, decide: fix here vs split into hygiene PR.

### Phase 6 — Spec + ROADMAP closure

- [ ] **Task 15.** Author design spec at `docs/superpowers/specs/2026-05-08-K99-msrv-bump-1.95-design.md` following project convention (reference `2026-04-13-S01a3-explicit-re-export-list-design.md`).
- [ ] **Task 16.** Mark K99 done in `.ai-factory/ROADMAP.md` — flip checkbox + add completion-date row to `## Completed` table.

## Task Dependencies

```
   1. Determine target version
        │
        ▼
   2. Audit blockers + tooling/perf classification
        │
        ▼
   3. Bump root Cargo.toml ─┬──► 4. rust-toolchain.toml + flake.nix note
                            │
                            ├──► 5. Per-crate inheritance (×11) ──► 6. CI gate
                            │
                            ├──► 17. clippy.toml msrv field
                            │
                            └──► 18. Cargo.lock policy decision
                                       │
                                       ▼
                                 11. cargo build [needs 5, 17, 18]
                                       │
                                       ├──► 12. cargo test
                                       │
                                       ├──► 13. cargo clippy [also needs 17]
                                       │
                                       └──► 14. cargo fmt
                                                  │
                                                  ▼
                                            15. Design spec
                                                  │
                                                  ▼
                                            16. ROADMAP closure

   Parallel/independent (after task 3):
     7. AGENTS.md, 8. DESCRIPTION.md, 9. rules/base.md, 10. README.md
```

## Commit Plan

K99 has **18 tasks** (16 original + 2 added in /aif-improve refinement); per the skill convention 5+ tasks need commit checkpoints. Suggested commits:

| Commit | Tasks | Message (conventional commits) |
|---|---|---|
| 1 | 1, 2 | `chore(msrv): pre-flight audit for Rust 1.95+ bump` (no code, just toolchain prep — skip if no commits needed for audit) |
| 2 | 3, 4, 5, 17, 18 | `chore(msrv)!: bump workspace rust-version to 1.95, pin toolchain, propagate to all 11 members` (BREAKING — MSRV bump; includes clippy.toml msrv + Cargo.lock freeze policy) |
| 3 | 6 | `ci(msrv): add explicit MSRV gate job to ci.yml` |
| 4 | 7, 8, 9, 10 | `docs(msrv): update MSRV references across AGENTS, DESCRIPTION, rules, README` |
| 5 | 11, 12, 13, 14, 15 | `docs(spec): add K99 design spec for MSRV bump to Rust 1.95+` (validation gates passed in this commit's CI run; spec captures rationale incl. Cargo.lock policy + flake.nix divergence) |
| 6 | 16 | `docs(roadmap): mark K99 MSRV bump complete` (one-line ROADMAP flip) |

If task 1's audit reveals zero work to do (already on a compatible toolchain), commits 1 and 5 can be merged with their neighbors.

## Done criteria

K99 is done when:

1. ✅ Workspace MSRV declared at chosen target in `Cargo.toml`
2. ✅ `rust-toolchain.toml` exists, pins the chosen channel
3. ✅ **All 12 workspace members** inherit MSRV (`rust-version.workspace = true` in Cargo.toml — already in `flui-platform`, added in 11 others)
4. ✅ `clippy.toml` has `msrv` field matching `Cargo.toml` rust-version
5. ✅ Cargo.lock policy decided + documented in design spec
6. ✅ `flake.nix` divergence (fenix.latest vs MSRV) decided + documented
7. ✅ `tooling/perf` workspace status classified (in members or out)
8. ✅ CI has explicit MSRV gate job that runs on every PR
9. ✅ All MSRV references in docs updated to chosen target
10. ✅ `cargo build --workspace --all-features` green on new MSRV
11. ✅ `cargo test --workspace` green on new MSRV
12. ✅ `cargo clippy --workspace --all-targets -- -D warnings` clean
13. ✅ `cargo fmt --all -- --check` clean
14. ✅ Design spec exists in `docs/superpowers/specs/`
15. ✅ ROADMAP K99 entry checked off, completion date recorded

## Open questions (decided in spec, not blocker for plan)

- **Pin exact version vs `stable` in `rust-toolchain.toml`?** — recommendation in the spec: pin exact for reproducibility; document a quarterly bump cadence.
- **Should we wait for Rust 1.95 if it's not stable yet at implementation time?** — no. Hard fork posture means we move on the latest available stable; the K99 spec records the chosen version. K-track is sequential and we shouldn't block on Rust release schedule.
- **CI matrix expansion (full feature-powerset)?** — out of scope. R5 covers that separately.
- **Migration guide for downstream consumers?** — N/A. flui-v2 has no downstream consumers (hard fork, pre-1.0).
- **`flake.nix` should pin to MSRV or stay on `fenix.latest`?** — recommendation: stay on `fenix.latest` as forward-compat radar. Document the intentional divergence.
- **Cargo.lock: freeze, refresh, or selective?** — recommendation: FREEZE for K99. Updates only if forced by build. Cargo.lock changes are a separate hygiene concern.
- **`tooling/perf` workspace membership?** — decided in Task 2 audit. Three possible outcomes: (a) add to `[workspace] members` and inherit MSRV, (b) confirm intentional standalone (own workspace root, separate MSRV), (c) classify as broken inheritance and split fix into hygiene PR.

## Risk assessment

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Newer Rust introduces breaking lint that blocks CI | Medium | Low | Tasks 13 has explicit handling — fix legitimately, narrow `#[allow]` only with rationale |
| Newer rustfmt reformats large swaths | Low-Medium | Low | Task 14 can split format-only changes to hygiene PR (slot in K90-K98) if non-trivial |
| Dependency version bumps required | Low | Low | Workspace deps pinned via `[workspace.dependencies]` (not yet — that's A6); manual case-by-case if surfaces |
| 1.95 not yet stable at implementation time | Possible | Low | Task 1 falls back to latest available stable, documents version gap, K99 still proceeds |

## Next steps

After K99 lands and is merged:

```
/aif-plan full K15-reentrancy-contract
```

K15 is the next critical-chain spec — documents and enforces re-entrancy semantics for `update_window` / `update_entity` / `setState` (queue-not-panic). This is the architectural spec that affects every callback in the system.
