# K07 — AppCell removal (token-based borrow model)

**Branch:** `feature/K07-appcell-removal-token-borrow`
**Created:** 2026-05-09
**Refined (aif-improve):** 2026-05-09 — codebase-grounded fact pass; recount precision (`~248` → `100-220, exact in Task 2`); added open questions Q8 (`AppContext::as_mut` widening), Q9 (Drop-order semantics), Q10 (`Application: Clone` status); reentrancy.rs rustdoc cleanup added to Task 9; executor.rs test-fixture clarified; explicit parallel labels on Tasks 15-19; examples scan widened in Task 24; explicit dep on Task 25 from {20,21,22,23,24}.
**Refined (aif-improve round 2):** 2026-05-09 — project-convention pass; +3 doc-completeness tasks (32a CHANGELOG entry, 32b migration guide at `docs/superpowers/migrations/K07-appcell-removal.md`, 32c AGENTS.md update); Q11 (CHANGELOG backfill policy) + Q12 (PR title convention) added; Done criteria 21d/21e/21f added.
**Refined (post-spike risk audit):** 2026-05-09 — Phase 1 spike + 3-agent research (UI frameworks, Rust borrow primitives, local candidate analysis) **lock in Candidate B** (UnsafeCell + flag → ReentryError). 4 risk-driven updates: Task 14a (R5 — panic-leak fields RAII fix, K15 Limitation #6 closure), Task 8 expansion (R2/R3/R4 — auto-trait + `#[track_caller]` compile-time tests), Q6 re-eval to PR-blocking Miri, "Future considerations" section in design spec for R9/R10/R12. Q1-Q12 all resolved with rationale. Recount: `this.upgrade()` = **5 distinct sites** (not ~30); K15 has **6 Known Limitations** (not 4). 48 → **49 checkbox tasks** (+14a; Task 8 expansion / Future considerations are non-checkbox scope changes).
**Refined (rev 5 — Task 6 adversarial review absorbed):** 2026-05-09 — three reviewers (`flui-arch-reviewer`, `migration-risk-adversary`, `rust-api-migration-auditor`) returned **8 BLOCKERs + 12 MAJORs + 10 MINORs**. Comprehensive triage: spec rev 2 published with all BLOCKERs patched. Major plan changes: Task 8 (Drop NO `#[track_caller]` — no-op; `#[derive(Debug)]` on BorrowState; temporary manual-UnwindSafe proposal later superseded by rev 6; `pub use cell::{...}` not `pub(crate) use`; NO standalone `AppCell::new` — Rc::new_cyclic integration); Task 9 (MANDATORY deletion of `borrow_mut_error_converts_to_app_borrowed` test at reentrancy.rs:253-259); Task 14a (temporarily re-scoped to `catch_unwind(AssertUnwindSafe(...))`, later superseded by rev 6 raw-pointer guards); Task 14b (NEW — paint/dispatch hot-path audit for Key Principle #8); Task 15 (5 `as_mut` panic sites, not 1; AsyncApp::app() cascade policy explicit per public method). 49 → **50 checkbox tasks** (+1 hot-path audit). Cross-confirmed BLOCKERs: WindowUpdateGuard/EntityUpdateGuard fabricated (don't exist), UnwindSafe inverted, EntityScope won't compile, BorrowMutError test orphan. False-positive: static_assertions Cargo.lock contradiction (already in lockfile transitively).
**Refined (rev 6 — second adversarial review of rev 2 absorbed):** 2026-05-09 — 4 agents reviewed rev 2 (3 adversarial + 1 quality general-purpose). **Rev 2 introduced new BLOCKERs while fixing rev 1's.** rust-api-migration-auditor verified std source: `unsafe impl UnwindSafe` is `error[E0199]` (UnwindSafe is not unsafe trait); UnwindSafe regression narrative is FALSE (`UnsafeCell<T>: UnwindSafe` automatic, no negative impl). Migration-risk found: `catch_unwind` pattern leaks `pending_updates` via `resume_unwind` through `App::update::finish_update` — silent permanent regression. Arch + migration cross-confirmed: catch_unwind also clears entity guard while leaving slot leaked (worse than K15). Major rev 6 plan changes: Task 8 (DROP `unsafe impl UnwindSafe`/`RefUnwindSafe`; auto-trait + `assert_impl_all!(UnwindSafe)` + `assert_not_impl_any!(RefUnwindSafe)` regression guards; `debug_assert!` in Drop cfg-gated to avoid abort-on-double-panic); Task 9 (atomic with `async_context.rs:96` line update — fix compile-error window); Task 14a (re-re-scoped — `catch_unwind` LEAKS pending_updates; replaced with raw-pointer field-projection guard pattern: `Guard { ptr: *mut Field, prev }`, Drop runs `*self.ptr = self.prev` without crossing `App::update` frame); Task 14b (unconditionally PR-blocking; output committed not gitignored); Task 15 (Q4 cascade uses `panic_any(ReentryError::AppGoneAway)` for typed payload, not `panic!("{}", e)`); 5 `as_mut` Display rephrased context-agnostic. New tasks: 7a (audit whether `try_borrow` shared-cell avenues are needed — possibly drop `BorrowState::Shared` entirely), 8a (Cargo.toml `[dev-dependencies]` += static_assertions), 26b (AppCell acquire/release bench example, no Criterion dependency), 27a (CI job for `cargo +nightly miri test -p flui-core cell`). 50 → **54 checkbox tasks**.
**Refined (rev 7 — aif-improve drift cleanup):** 2026-05-10 — scrubbed actionable spec contradictions left after rev 6; added Task 7b for the spec-scrub gate; synchronized dependency graph, commit plan, Done criteria, task count, K15-test accounting, bench/Miri CI prerequisites, and callsite counts (`103` narrow AppCell hits; `5` `this.upgrade()` sites). 54 → **55 checkbox tasks**.
**Phase:** 0-K (Kernel Cleanup) — third spec in the critical chain (gates K05 → K01 → K02 → K03 → K04 → Phase II-F)
**Type:** structural refactor of the App ownership primitive (replaces `RefCell<App>` with a compile-time-checked borrow model). **HIGH-RISK** per ROADMAP — but Phase 1 spike + 3-agent research (UI framework comparison, Rust borrow primitive comparison, local candidate analysis) **LOCKED IN Candidate B** (hand-rolled `UnsafeCell<App>` + `BorrowState` flag returning `ReentryError`). Migration is signature-compatible (~200 LoC primitive + 0 callsite-LoC; 103 narrow-pattern AppCell-derived callsites compile unchanged after the cell rewrite).
**Tasks:** 55 checkbox tasks (45 base tasks + 3 doc-completeness tasks from rev 3 + 1 panic-safety task from rev 4 + 1 hot-path audit from rev 5 + 4 rev 6 tasks + 1 rev 7 spec-scrub task).
**Decision lock:** Candidate B — see "Design choice — three candidates" §"Recommended candidate (LOCKED — revision 4)" below.

> **Design-first spec.** Unlike K15, the K07 design is NOT pre-decided in this plan. The token-based borrow primitive itself is the central design choice (Section "Design choice — three candidates" lists the alternatives). Phase 1 of the plan authors the design spec which freezes the primitive; subsequent phases implement it.

## Settings

| Setting | Value | Rationale |
|---|---|---|
| Testing | yes | HIGH-RISK refactor of every callback path; property tests for borrow soundness + runtime safety are CORE deliverables. Without them the new primitive is unverified. |
| Logging | verbose | New runtime path (token construction, borrow attempts under each candidate). Project uses `log` crate (NOT `tracing` — A4 is roadmap, out of K07 scope). `log::trace!` per token enter/exit, `log::warn!` on borrow contention (when applicable to chosen primitive), `log::debug!` on contract-mode toggle. |
| Docs | yes (mandatory checkpoint) | New public types (token / cell), new module-level docs, **design spec** at `docs/superpowers/specs/2026-05-09-K07-appcell-removal-design.md` is itself a deliverable, ROADMAP flip, RESEARCH addendum, K07 ports the K15 contract onto the new primitive (rustdoc cross-references must be updated). |
| Roadmap linkage | linked | K07 in Phase 0-K critical chain — third spec, gates K05–K04 and SF04/SF05. |

## Roadmap Linkage

**Milestone:** K07 — AppCell removal — token-based borrow model (Phase 0-K Kernel Cleanup, critical chain — third spec after K99 and K15).

**Rationale:** Per `.ai-factory/ROADMAP.md` Phase 0-K — "replaces `AppCell = RefCell<App>` (marked 'remove after stabilization') with token-based mutual-exclusion. Closes E3 from `docs/promt.md`. **HIGH-RISK** — every callback signature changes."

K07 must land before K05 (Element ctx-object), K01 (Provider rewrite), and SF04/SF05 (Framework State + setState):
- **K05** introduces `&mut PaintCx<'_>` / `&mut LayoutCx<'_>` — those context objects need to know what mutability proof they carry; if AppCell still exists, K05 either piggybacks on `RefCell<App>` (cementing the debt) or invents a parallel proof (forking the surface). K07 first, K05 second.
- **K01** rewrites the Provider to per-Window `InheritedRegistry` with subscription-driven invalidation. The subscriber-set + dirty-list machinery cross-borrows App fields; doing this on top of AppCell means writing K01 callsites against the legacy primitive, then rewriting them in K07. K07 first.
- **SF04** (Framework State<W> + StateMap) and **SF05** (setState + dirty-list) are explicit consumers of K07 per the ROADMAP cross-track dependency table.

K07 also discharges the deferred items from K15:
- `ReentryMode::PanicLikeUpstream` (K15 Decision log §"Removed PanicLikeUpstream" — deferred to K07): under the new primitive, "upstream-like" panics may need to be re-introduced as a one-release migration aid.
- The 10+ unstructured `app.borrow_mut()` sites in `app/async_context.rs` lines 39, 45, 55, 65, 126, 135, 152, 168, 182 (K15 Known Limitation #1: "K07 redesigns this surface").
- `AsyncApp::as_mut` panic at `app/async_context.rs:73` (K15 Known Limitation #2: "different panic class — out of K15 scope").

## Research Context

From `.ai-factory/RESEARCH.md` (Active Summary), the K07 reconnaissance pass, and the K15 hand-off notes:

- **Hard fork posture** — flui-v2 has no upstream-sync commitment. K07 is unilateral; we name the primitive, document it, enforce it. Upstream's `Rc<RefCell<App>>` shape is not a constraint to preserve.
- **Phase 0-K rationale** — 24+ structural issues block a healthy Framework tier; AppCell is one of them.
- **Audit context for E3** ([docs/promt.md](docs/promt.md) §E3, line 857-859):
  > "AppCell = RefCell<App> with TODO 'remove after stabilization' (HIGH-RISK)"
  > "Where: crates/flui-core/src/app.rs:73-75."
  > "Fix: token-based borrow model; runtime-borrow-check elimination. Spec: S36."
  (Note: legacy "S36" reference is the pre-Phase-0-K numbering; the active number is K07.)
- **Current state — AppCell wrapping** ([app.rs:75-108](crates/flui-core/src/app.rs#L75)):
  - `AppCell { app: RefCell<App> }`. `pub fn borrow() -> AppRef<'_>`, `pub fn borrow_mut() -> AppRefMut<'_>`, `pub fn try_borrow_mut() -> Result<AppRefMut<'_>, BorrowMutError>`.
  - `AppRef` and `AppRefMut` derive `Deref`/`DerefMut`, drop-trace via `option_env!("TRACK_THREAD_BORROWS")`.
  - `pub struct Application(Rc<AppCell>)` is the public top-level handle.
  - `App::this: Weak<AppCell>` is the internal back-pointer (line 585).
- **Consumers of `Rc<AppCell>` / `Weak<AppCell>`** (12 known sites):
  - `Application(Rc<AppCell>)` ([app.rs:139](crates/flui-core/src/app.rs#L139))
  - `App.this: Weak<AppCell>` ([app.rs:585](crates/flui-core/src/app.rs#L585)) — back-pointer used by **5 distinct** `this.upgrade()` sites (`app.rs:215`, `app/context.rs:74,110,166`, `app/test_context.rs:660`)
  - `App::new_app(...) -> Rc<AppCell>` ([app.rs:684](crates/flui-core/src/app.rs#L684))
  - `AsyncApp.app: Weak<AppCell>` ([app/async_context.rs:23](crates/flui-core/src/app/async_context.rs#L23))
  - `AsyncApp::app() -> Rc<AppCell>` ([app/async_context.rs:29](crates/flui-core/src/app/async_context.rs#L29)) — internal helper used by every other method
  - `TestAppContext.app: Rc<AppCell>` ([app/test_context.rs:32](crates/flui-core/src/app/test_context.rs#L32))
  - `TestAppContext::to_async() — app: Rc::downgrade(&self.app)` ([app/test_context.rs:425](crates/flui-core/src/app/test_context.rs#L425))
  - `TestApp.app: Rc<AppCell>` ([app/test_app.rs:41](crates/flui-core/src/app/test_app.rs#L41))
  - `TestApp::to_async — app: Rc::downgrade(&self.app)` ([app/test_app.rs:226](crates/flui-core/src/app/test_app.rs#L226))
  - `TestApp.app: Rc<AppCell>` (second occurrence — different impl block) ([app/test_app.rs:322](crates/flui-core/src/app/test_app.rs#L322))
  - `HeadlessAppContext.app: Rc<AppCell>` ([app/headless_app_context.rs:40](crates/flui-core/src/app/headless_app_context.rs#L40))
  - `VisualTestContext.app: Rc<AppCell>` ([app/visual_test_context.rs:23](crates/flui-core/src/app/visual_test_context.rs#L23))
- **`borrow()` / `borrow_mut()` callsites — magnitude (refined recount):**
  - 99 occurrences of `app.borrow_mut()` / `app.try_borrow_mut()` / `app.borrow()` patterns in `crates/flui-core/src/` — these are predominantly AppCell-derived and the most reliable proxy for the migration target.
  - 217 occurrences of broader patterns including `.0.borrow_mut()`, `self.app.borrow_mut()`, `this.borrow_mut()`, `lock = app.borrow_mut()` — this set MIXES AppCell sites with non-AppCell (`RefCell<Window>`, `RefCell<Keymap>`, `RefCell<Arena>`) sites.
  - **True AppCell-specific count: between 99 and 217**, with the higher end being the upper bound. Exact count produced by Task 2 audit using the documented one-liner.
  - Files using `AppCell` symbol directly (verified via `git grep AppCell`): `app.rs`, `async_context.rs`, `test_context.rs`, `test_app.rs`, `headless_app_context.rs`, `visual_test_context.rs`, `executor.rs` (test fixture only — see Note 1), `reentrancy.rs` (rustdoc only — see Task 9), platform `app_menu.rs` / `mac/platform.rs` / `windows/platform.rs` (K15 deferral comments only).
  - **Note 1 (executor.rs):** [executor.rs:556](crates/flui-core/src/executor.rs#L556) declares `fn create_test_app() -> (TestDispatcher, BackgroundExecutor, Rc<crate::AppCell>)` inside a `#[cfg(test)]` module. Line 573 calls `app.borrow().foreground_executor.clone()`. NO production-code AppCell access in `executor.rs`.
- **K15 contract is the load-bearing precondition** — re-entrant `update_window` returns `Err(ReentryError::NestedWindowUpdate)`; same-entity re-entry panics with `ReentryError::NestedEntityUpdate` Display; multi-entity cycles produce the same Display via the unified `EntityMap::double_lease_panic`. K07 keeps these contracts intact under the new primitive — that means the new primitive MUST surface re-entry detection as cleanly as `try_borrow_mut()` did, OR the primitive itself must make re-entry a structurally impossible state.
- **K15 deferred surface to K07:**
  1. `ReentryMode::PanicLikeUpstream` was removed from K15. K07 may re-introduce it as a one-release migration mode (see Design choice §3) or document it as obsolete.
  2. 10+ unstructured `app.borrow_mut()` sites in `app/async_context.rs` (lines 39, 45, 55, 65, 126, 135, 152, 168, 182). K07 redesigns the surface so these become structured.
  3. `AsyncApp::as_mut` panic at `app/async_context.rs:73`: `panic!("Cannot as_mut with an async context. Try calling update() first")`. K07 either (a) removes the `as_mut` capability entirely, (b) keeps the panic but changes its Display, or (c) returns `Result`.
  4. K15 noted: "if revealed by K07, file follow-up" — re: web platform dispatcher re-entry exposure (web event loop is single-threaded). K07's primitive interaction with web must be explicitly verified.
- **Re-entrancy contract (post-K15) MUST be preserved by K07** — every behavioral test in `crates/flui-core/src/reentrancy.rs` must continue to pass on the new primitive. K07 ports the tests forward; if any test FAILS under the new primitive, that's a contract regression — escalate.
- **MSRV 1.95 (K99 done) unlocks idioms K07 may use:** `OnceLock` / `LazyLock` (stable), AFIT and RPITIT (for token-protected accessor traits), edition-2024 lifetime captures (for borrow-token return types), `unsafe extern` (only if Candidate C below is chosen). Async closures stable (relevant for `AsyncApp` redesign).
- **Constraints carried over:**
  - 60 FPS structural property — no per-frame allocation, no per-borrow heap traffic.
  - "No `Rc<RefCell<…>>` on dispatch / tick / paint hot paths" (ARCHITECTURE Key Principle #8). K07 MUST shrink not grow the `Rc<RefCell<…>>` surface.
  - Single-threaded `App` invariant — `App: !Send + !Sync` (current model). K07 preserves this; the new primitive must encode it (PhantomData or `*const ()` field).
  - `cargo-semver-checks` is roadmap R2 — K07 is breaking by design, but the surface should be small enough to review.
- **Adversarial review precedent:** K15 went through `flui-arch-reviewer` + `migration-risk-adversary` + `rust-api-migration-auditor` review and absorbed 22 findings (9 BLOCKER, 8 MAJOR, 5 MINOR). K07 carries higher migration risk than K15; planning factors all three reviewers in plus `wgpu-gpu-reviewer` if the chosen primitive interacts with the GPU dispatch path (it shouldn't — but adversary spot-checks it).

## Current state (pre-K07)

| Aspect | State | Note |
|---|---|---|
| Public top-level handle | `Application(Rc<AppCell>)` ([app.rs:139](crates/flui-core/src/app.rs#L139)) | `AppCell = RefCell<App>` ([app.rs:75-78](crates/flui-core/src/app.rs#L75)) |
| Internal back-pointer | `App::this: Weak<AppCell>` ([app.rs:585](crates/flui-core/src/app.rs#L585)) | used by 5 distinct `this.upgrade()` sites per `.k07-recon.txt` |
| Async context | `AsyncApp { app: Weak<AppCell>, … }` ([async_context.rs:23](crates/flui-core/src/app/async_context.rs#L23)) | Holds `Weak`; calls `.upgrade()` per operation |
| Test contexts | `TestAppContext`, `TestApp`, `VisualTestContext`, `HeadlessAppContext` each own `Rc<AppCell>` | mirrors `Application` shape |
| Mutable-borrow API | `AppCell::borrow_mut() -> AppRefMut<'_>`, `try_borrow_mut() -> Result<…, BorrowMutError>` | runtime borrow check |
| Re-entry detection | K15 contract via inline `window_update_stack` / `currently_updating_entity` checks plus `double_lease_panic` unified Display | structured AT CALLBACK BOUNDARIES, but the primitive itself still uses `RefCell` |
| AppCell-derived borrow callsites | 103 narrow-pattern hits | migration target from `.k07-recon.txt`; wide-pattern count includes non-AppCell `RefCell` sites |
| `AsyncApp` `borrow_mut()` sites without K15 structure | 10+ at lines 39, 45, 55, 65, 126, 135, 152, 168, 182 | K15 Known Limitation #1 |
| `AsyncApp::as_mut` panic | `app/async_context.rs:73` raw `panic!(…)` | K15 Known Limitation #2 |
| `AppCell` `#[doc(hidden)]` | yes ([app.rs:74](crates/flui-core/src/app.rs#L74)) | type is hidden but `borrow*` methods are `pub` |
| `Application` semver shape | `Application(Rc<AppCell>)` is `pub struct` with non-`pub` field — public API is the methods | K07 keeps the `Application` name and constructor; internals replaced |
| `AppRef` / `AppRefMut` | `pub struct AppRef<'a>(Ref<'a, App>)` derives `Deref<Target=App>`/`DerefMut` ([app.rs:111-135](crates/flui-core/src/app.rs#L111)) | callers rely on `Deref<Target=App>` ergonomics; the new primitive must offer the same |
| `borrow_mut` debug instrumentation | `option_env!("TRACK_THREAD_BORROWS")` printlns | K07 either preserves equivalent diagnostic or replaces it with `log::trace!` (project style) |

## Design choice — three candidates

K07's central design decision is the borrow primitive. Phase 1 of the plan picks ONE candidate; the remaining phases implement it. The three candidates below are the design space; the design spec (Task 5) commits to one with a documented rationale.

### Candidate A — Pass-through `&mut App` (no cell)

**Shape:** Replace `Rc<AppCell>` with `Rc<RefCell<RuntimeShim>>` where `RuntimeShim` holds executor + platform handles only; `App` itself is owned by the run-loop top-level closure and passed by `&mut` through every call chain. Callbacks that need `App` access acquire it via the run-loop dispatching `&mut App` into the closure. Asynchronous callers go through `AsyncApp` which **enqueues** a closure; the run-loop drains the queue and dispatches `&mut App` into each.

**Pros:**
- Zero runtime borrow checks. No `RefCell` on the App-level path.
- `App: !Send + !Sync` is structural (no PhantomData wizardry needed).
- Best fit for ARCHITECTURE Key Principle #8.

**Cons:**
- ALL async-context methods change shape: `AsyncApp::update<F>(F)` becomes a queue-and-poll. Existing code that does `let mut app = handle.borrow_mut(); app.x(); app.y()` collapses into a single closure → multi-line refactor of every callsite, not a 1:1 rename.
- Lifetime gymnastics: `&mut App` outlives every callback and becomes the universal proof. May cascade into Window/Element borrows.
- HIGHEST migration cost; incompatible with incremental landing across multiple PRs.

### Candidate B — Single-borrow guard (token = guard object)

**Shape:** Replace `RefCell<App>` with a custom `AppCell` that holds `UnsafeCell<App>` and a single `bool`/`AtomicBool`-equivalent active flag. `borrow_mut()` returns `AppGuard<'_>` (RAII drop clears the flag); recursive `borrow_mut()` returns `Result<AppGuard, BorrowMutError-equivalent>`. Same callsite ergonomics as today — but the cell type is owned by `flui-core`, not `std`, so we can:
- Replace `BorrowMutError` with `ReentryError::AppBorrowed` natively (no `From` conversion needed).
- Add structured `log::warn!` on contention.
- Add a `track-borrows` debug feature that records call locations.
- Make the type `!Send + !Sync` explicitly (matching `App`).

**Pros:**
- Minimal callsite churn — `AppCell::borrow_mut()` shape preserved; mass-replace `borrow_mut()` with `lock_mut()` (or keep the name) and the change is largely mechanical.
- Async surface (`AsyncApp::update`) keeps current ergonomics.
- Re-entry detection localized in the new primitive instead of K15's overlay guards.
- `unsafe` for the cell primitive itself is confined to `app/cell.rs`; Task 14a adds separate raw-pointer field-guard writes in `app.rs`. `wgpu-gpu-reviewer` does NOT need to engage (cell is independent of GPU path).

**Cons:**
- Still has runtime check (just better-instrumented). The "token-based" promise from ROADMAP is partially fulfilled.
- `unsafe { &mut *cell.get() }` is a hand-written `RefCell` — needs careful Stacked-Borrows / Tree-Borrows audit (Miri).

### Candidate C — Compile-time token (qcell / GhostCell variant)

**Shape:** Use a `GhostCell`-style approach — `AppCell` stores `App` behind `UnsafeCell`; access requires `&mut AppToken<'id>` where `'id` is a unique branded lifetime per run-loop invocation. The token is constructed once at App startup and threaded through every callback signature. `Application::run` produces the token and passes it.

**Pros:**
- ZERO runtime check; borrow soundness proven by Rust's type system.
- Best aligns with the "token-based borrow model" wording in the ROADMAP and `docs/promt.md` §E3.

**Cons:**
- Every callback signature changes: `FnMut(&mut App)` → `FnMut(&mut AppToken<'id>)` with `'id` brand.
- Branded lifetimes are notoriously ergonomically painful — `for<'id>` HRTB scattered throughout, and adoption requires user-facing API changes.
- Crate dependency choice: `qcell` (mature, but adds a dep) vs hand-rolled GhostCell (more `unsafe`, but no dep).
- Widget authors writing custom `Element` impls now see `AppToken<'id>` in trait method signatures — affects every downstream crate.
- HIGHEST design risk; ergonomic regression may force a partial rollback.

### Recommended candidate — LOCKED (revision 4, post-spike)

**Candidate B (single-borrow guard).** Locked after Phase 1 spike (Task 3 + 3-agent research dispatch from Tasks 4 + Agent 1 UI-framework comparison + Agent 2 Rust borrow-primitive comparison).

**Spike findings supporting the lock:**

1. **Agent 2 (Rust primitives):** explicitly recommended hand-rolled `UnsafeCell<T>` + `bool` flag wrapper that returns `ReentryError` for "single-threaded UI App with ~100 callbacks at arbitrary depth, with re-entrancy detection already structurally enforced". Rejected `ghost-cell` for production UI: HRTB pollutes trait objects (poisoning `Box<dyn FnMut>` callback typedefs at `app.rs:243-250`).
2. **Agent 1 (UI frameworks):** Zed/GPUI upstream has NOT removed AppCell — flui-v2 leads. Druid/Xilem/Iced/Vizia structurally use top-down `&mut T` (not applicable to GPUI's open-ended callback model). Bevy ECS uses compile-time access analysis (incompatible with arbitrary callback closures). Slint/Dioxus/Floem/Servo SHARD the cells — but full sharding is K06 territory (Window decomposition), not K07.
3. **Agent 3 (local spike):** Candidate A migration cost = ~1500-2500 LoC (intellectual, multi-week); Candidate C = >5000 LoC (HRTB pollution, `AsyncApp` doesn't compose with branded `'id`). **Candidate B = ~200 LoC primitive + 0 callsite-LoC**, fully signature-compatible.
4. **K15 contract preservation:** Candidate B's cell flag = `ReentryError::AppBorrowed`; K15's inline `window_update_stack` / `currently_updating_entity` same-target checks are ORTHOGONAL — different concerns, both stay. No K15 contract regression.
5. **`App.this: Weak<AppCell>`** (5 distinct upgrade sites — recount during spike, NOT ~30 as plan rev 1-3 claimed) all compile unchanged under Candidate B.

**Rejected alternatives (with documented reasons):**
- **Candidate A** — DEFER to follow-up. Async-surface rewrite is itself a multi-week project; mixing it with K07 violates K07 PR scope discipline. K15 Known Limitation #1 (10+ unstructured `app.borrow_mut()` in `async_context.rs`) is partially addressed by Candidate B's `try_borrow_mut() -> Result<_, ReentryError>` shape.
- **Candidate C** — REJECT. HRTB through every callback typedef. `AsyncApp` cannot exist with branded `'id` per `async_context.rs:15-20` ("static lifetime so it can be held across await points"). RustBelt-proven sound but ergonomically incompatible with retained-mode UI.
- **Adding `qcell` / `ghost-cell` dependency** — REJECT. Project minimizes deps in Phase 0-K (see K92 dep-update spec). `qcell::TLCell` would work but adds global-marker constraint and dep weight for marginal gain over hand-rolled cell.
- **Sharding (Slint/Dioxus pattern)** — OUT OF SCOPE for K07. K06 (Window decomposition) and future per-domain owners (BuildOwner/PipelineOwner/SemanticsOwner) handle sharding. Candidate B does NOT preclude future sharding.

**Plan is now dimensioned strictly for Candidate B.** Tasks 11-25 assume signature-compatible migration. If a Phase-2 implementation blocker re-opens the candidate question, that's a revision-5 trigger (not a routine spec edit).

## What K07 explicitly does NOT do

- Does NOT remove `RefCell<Keymap>`, `RefCell<Arena>`, `RefCell<Window>`, or any of the field-level `RefCell` instances inside `App` ([app.rs:598](crates/flui-core/src/app.rs#L598), [app.rs:619](crates/flui-core/src/app.rs#L619)). Those are out of scope; K07 owns only the App-level cell.
- Does NOT change `Element` trait method signatures (K05).
- Does NOT introduce `BuildOwner` / `PipelineOwner` (K06).
- Does NOT touch `Render::&mut self` (K03).
- Does NOT refactor the Provider system (K01).
- Does NOT introduce `Key` (K02) or Widget identity.
- Does NOT touch the pending-effects queue or change frame phases (K04).
- Does NOT widen `Element` or `Render` trait signatures (K05).
- Does NOT change `Platform::*` trait surface (out of platform scope per ARCHITECTURE).
- Does NOT add `tracing` (A4 is roadmap, out of K07).
- Does NOT touch gesture re-entry surface (A7-audit-closed).
- Does NOT modify `Cargo.lock` (workspace policy frozen — see CLAUDE.md).
- Does NOT bypass Miri verification of the new `unsafe` (if Candidate B/C). Adversarial review verifies.

## Known Limitations (to document in design spec)

Pre-emptive enumeration; each is a deliberate scope decision:

1. **Element / Render method signatures** retain their AppCell-era shape. K05 reshapes them. K07 keeps `&mut App`-flavored access through whatever the new primitive yields; the trait surface is K05's problem.
2. **Web platform** is single-threaded; the K07 primitive must not regress under wasm/web event-loop integration. Verification via existing test-platform paths; if Phase 7 adversary review reveals a web-specific gap, file as follow-up K-spec rather than block K07.
3. **`option_env!("TRACK_THREAD_BORROWS")` debug instrumentation** ([app.rs:83-86](crates/flui-core/src/app.rs#L83), [93-96](crates/flui-core/src/app.rs#L93), [103-106](crates/flui-core/src/app.rs#L103), [117-119](crates/flui-core/src/app.rs#L117), [129-131](crates/flui-core/src/app.rs#L129)) — replaced by `log::trace!` under target `flui_core::app::cell` (matching project style).
4. **`AsyncApp::as_mut`** panic at [async_context.rs:73](crates/flui-core/src/app/async_context.rs#L73) — replaced by `Result<&mut App, ReentryError::AppBorrowed>` OR removed entirely if no caller (Phase 5 audit).
5. **Inspector / debug feature** (`#[cfg(any(feature = "inspector", debug_assertions))]` fields in App) — preserved as-is; the cell wraps the whole `App`, so inspector surface is untouched.

## Tasks

### Phase 0 — Pre-flight & branch hygiene

- [x] **Task 1.** ✅ Baseline captured at `.k07-baseline.txt` (HEAD `1fe67bf103`, 2026-05-09T11:23:39Z). Results: `cargo build --workspace --all-features` green (21.76s); `cargo test -p flui-core --lib --all-features` = **345 tests, 344 passed + 1 ignored** (K15's "344 passed" matches; the 17 new K15 tests are already inside the 344 — recount note for plan was off); `cargo clippy --workspace --all-targets -- -D warnings` zero warnings; `cargo fmt --all -- --check` clean; `cargo doc --workspace --no-deps` exit 0 with 1 pre-existing `animation_demo` warning (NOT K07's regression — carry-forward).

- [x] **Task 2.** ✅ Recount captured at `.k07-recon.txt` (2026-05-09T11:23:50Z). **Exact numbers:**
  - Step 2.1 (wide pattern): **759 hits** across all `RefCell` types in `crates/flui-core/src/` (includes non-AppCell `RefCell<Window>`, `RefCell<Keymap>`, `RefCell<Arena>`, `RefCell<Scene>`, etc.).
  - Step 2.2 (narrow pattern, AppCell-derived): **103 hits** — variable names `app|this|cx|context_lock|lock` followed by `.borrow_mut()` / `.try_borrow_mut()` / `.borrow()`. **This is the K07 migration target.**
  - Step 2.3 (storage type declarations): **10 sites** of `Rc<AppCell>` / `Weak<AppCell>` (NOT 12 — the 2 extra in original recon were `Rc::downgrade(&self.app)` method calls, not type declarations): `app.rs:139` (`Application`), `app.rs:585` (`App.this`), `app.rs:684` (`new_app` return), `async_context.rs:23` + `:29`, `headless_app_context.rs:40`, `test_app.rs:41` + `:322`, `test_context.rs:32`, `visual_test_context.rs:23`.
  - Step 2.4 (symbol references): **33 across 10 files**: `app.rs:16`, `async_context.rs:3`, `executor.rs:1` (test fixture), `headless_app_context.rs:2`, `test_app.rs:3`, `test_context.rs:2`, `visual_test_context.rs:2`, `mac/platform.rs:1`, `windows/platform.rs:1`, `reentrancy.rs:2` (rustdoc — Task 9 cleanup target).
  - **Plan estimate validated:** "100-220 callsites" lower-bound 100 was correct; actual 103. Plan recount paragraph already accurate post-rev-2.
  - Drift: ZERO — Research Context numbers in plan match recon. No plan edits needed.

  Original Task 2 instructions retained for reference:
  - **Step 2.1 — Wide pattern (upper bound):**
    ```
    grep -rEn '\.borrow_mut\(\)|\.try_borrow_mut\(\)|\.borrow\(\)' crates/flui-core/src/ | wc -l
    ```
    Expect: ~217-248. This INCLUDES non-AppCell `RefCell` (`Window`, `Keymap`, `Arena`).
  - **Step 2.2 — Narrow pattern (lower bound, more accurate):**
    ```
    grep -rEn '(app|this|cx|context_lock|lock)\.(try_)?borrow(_mut)?\(\)' crates/flui-core/src/ | wc -l
    ```
    Expect: ~99-160. Each match should be inspected — false positives are sites where the variable name `app` / `cx` shadows a non-AppCell binding (e.g., `let app = NSApplication::sharedApplication(nil)` in `mac/platform.rs:512` — not an AppCell).
  - **Step 2.3 — Storage sites:**
    ```
    grep -rEn 'Rc<AppCell>|Weak<AppCell>' crates/flui-core/src/
    ```
    Expect: 12 sites listed in "Research Context".
  - **Step 2.4 — Symbol references:**
    ```
    grep -rEn 'AppCell|AppRef|AppRefMut' crates/flui-core/src/
    ```
    Catches imports like `use crate::{… AppCell, …}` plus inline references.
  - Capture all four numbers in `.k07-recon.txt` (gitignored). Update the "Research Context" recount paragraph in this plan if drift > 10%. Task 2 edits the plan file only; no code change.

### Phase 1 — Design spec authoring

- [x] **Task 3.** ✅ **(rev 4 — completed via parallel general-purpose agent dispatch.)** Three candidates spiked against actual file:line code: Agent 3 (local spike, 11 tool-uses, 104s) read `app.rs:70-250, 580-700, 2370-2520`, full `async_context.rs`, `test_context.rs:1-80,410-440`, `reentrancy.rs`, plus `app/context.rs:65-180`. Concrete findings: Candidate A = ~1500-2500 LoC intellectual migration (multi-week); Candidate B = ~200 LoC primitive + 0 callsite-LoC (signature-compatible); Candidate C = >5000 LoC, HRTB pollution, `AsyncApp` cannot exist with branded `'id`. **Verdict: Candidate B LOCKED.** See spec §"Recommended candidate (LOCKED — revision 4 post-spike)".

  Original Task 3 instructions retained for reference:
  - Time-box: 30 min per candidate.
  - For each: sketch `AppCell` type + `borrow_mut` impl + ONE example callback migration (`Application::run` and one `AsyncApp::update`).
  - Capture: lines-of-diff estimate, lifetime/HRTB blockers, `unsafe` count, Miri behavior.
  - Output: short note (`/tmp/k07-spike-A.md` etc.) used as input to Task 4.

- [x] **Task 4.** ✅ **(rev 4 — covered by parallel research agents.)** Agent 2 (Rust borrow-primitive comparison, 6 tool-uses, 83s) compared `qcell` (4 flavors), `ghost-cell` (HRTB branded), `atomic_refcell`, hand-rolled `UnsafeCell + flag`, `thread_local!` patterns, owning + `&mut` threading, message passing, branded-lifetime alternatives. Verdict: hand-rolled `UnsafeCell + bool flag → ReentryError` recommended over `qcell::TLCell` for "single-threaded UI App with ~100 callbacks at arbitrary depth, with re-entrancy detection already structurally enforced". Agent 1 (UI framework comparison, 31 tool-uses, 108s) confirmed: Druid/Iced/Xilem/Vizia structural `&mut` (incompatible), Bevy ECS (incompatible), Slint/Dioxus/Floem/Servo SHARD cells (K06 territory). All findings absorbed into spec §"Design choice — three candidates" and §"Decision log".

  Original Task 4 instructions retained for reference:
  > "Review three candidate replacements for `crates/flui-core/src/app.rs:75-108` (`AppCell = RefCell<App>`). Candidate A: pass-through `&mut App` (no cell). Candidate B: single-borrow custom guard with `UnsafeCell<App>` + active flag. Candidate C: GhostCell-style branded `AppToken<'id>`. For each, evaluate: (1) public-API blast radius, (2) feature-flag matrix, (3) trait object safety, (4) auto-trait regressions (`Send`/`Sync`/`UnwindSafe`), (5) workspace dependency direction. Score against ROADMAP K07 acceptance criteria. Output a recommendation table."
  - **Run in parallel** with Task 3 spikes.
  - Artifact: agent report cited in Task 5 spec.

- [x] **Task 5.** ✅ **(rev 4 — design spec rev 1 authored.)** File: `docs/superpowers/specs/2026-05-09-K07-appcell-removal-design.md` (~750 lines). All required sections present: Context, Goals, Non-goals, Current state, Design choice (three candidates with rejected-alternatives reasoning), Type surface (canonical sketch of `AppCell` + `AppRef` + `AppRefMut` with SAFETY comments, auto-trait tests, full `unsafe` audit), Migration plan, K15 contract preservation table, **Decisions on Q1-Q12 (all resolved)**, `unsafe` audit (3 blocks total), Compile-time auto-trait tests, Testing strategy, Known Limitations (5), **Open questions: EMPTY**, Done criteria, Cross-references, Unblocks, Risks (4-tier), Future considerations (R9/R10/R12), Decision log (revision 1 LOCKED rationale). Awaits Task 6 adversarial review for revision 2.

  Original Task 5 instructions retained for reference:
  - Context, Goals, Non-goals (mirror "What K07 explicitly does NOT do").
  - Current state (audit table from this plan).
  - Design choice — three candidates (verbatim from this plan), recommended candidate with documented rationale incorporating Tasks 3 + 4 outputs.
  - Detailed type surface: every `pub` symbol added/removed, with rustdoc.
  - Migration plan — chunks of callsites and how each maps from `borrow_mut()` → new API.
  - K15 contract preservation table — for each `ReentryError::*` variant, specify how K07 surfaces it on the new primitive.
  - **`AppContext::as_mut` / `GpuiBorrow<'a, T>` decision (Q8):** the trait method at [app.rs:2509-2513](crates/flui-core/src/app.rs#L2509) returns `pub struct GpuiBorrow<'a, T>` ([app.rs:2714](crates/flui-core/src/app.rs#L2714)). `AsyncApp::as_mut` panics ([async_context.rs:73](crates/flui-core/src/app/async_context.rs#L73)). Decision: (a) widen trait return to `Result<GpuiBorrow<'a, T>, ReentryError>` (BREAKING — all 5 `AppContext` implementors update); (b) keep panic but structure Display via `ReentryError::AsyncContextAsMutForbidden` new variant; (c) split trait — add `try_as_mut` returning `Result`, leave `as_mut` as panic-shape. Spec picks one with rationale.
  - **`Application` Drop-order semantics (Q9):** `Application(Rc<AppCell>)` — current Drop order is `Rc::drop` → `AppCell::drop` → `RefCell::drop` → `App::drop` (which honors the `// Drop globals last` comment at [app.rs:622-627](crates/flui-core/src/app.rs#L622)). New `AppCell` MUST preserve this ordering. If new cell holds `App` in `UnsafeCell<App>` directly, Drop semantics are preserved. If cell holds `Option<App>` or `MaybeUninit<App>`, behavior changes — spec forbids unless K12 (drop-order codification) is amended.
  - **`Application: Clone` status (Q10):** No `impl Clone for Application` exists today (verified: `grep 'impl Clone for Application'` returns 0 hits). The shape `Application(Rc<AppCell>)` IS clonable via tuple-field `.0.clone()` — but the public API does NOT expose `Clone`. Spec confirms: K07 keeps this. Adding `Clone` is OUT OF SCOPE.
  - PanicLikeUpstream decision: re-introduce, drop, or document as obsolete (resolves K15 deferral).
  - `AsyncApp::as_mut` decision: keep, remove, or `Result`-ify (resolves K15 deferral; cross-link Q8).
  - 10+ unstructured `app.borrow_mut()` sites in `async_context.rs`: explicit migration table per line (resolves K15 Known Limitation #1).
  - `unsafe` audit (if Candidate B/C): every `unsafe` block annotated with SAFETY comment + Miri test reference.
  - Decision log — record rejected candidates with reason; record any candidate-specific revisions surfaced by Task 4; record Q8/Q9/Q10 decisions explicitly.
  - Migration / compatibility table — the 12 `Rc<AppCell>` storage sites + the verified borrow callsite count from Task 2, grouped by file.
  - Testing strategy (proptest + behavioral; Miri requirement if Candidate B/C).
  - Known Limitations (5 enumerated above; expand as needed).
  - Open questions (must be RESOLVED at spec-merge time, NOT deferred to implementation).
  - Done criteria (verbatim from this plan).
  - Cross-references (K99, K15 specs; ROADMAP K07; RESEARCH Active Summary; `docs/promt.md` §E3; ARCHITECTURE Key Principles 1, 6, 8, 11; K12 (drop-order codification — Q9 cross-link)).
  - Unblocks (K05, K01, K02, K03, K04, SF04, SF05).
  - Risks & rollback strategy.
  - **Future considerations (rev 4 — R9/R10/R12 mitigation):**
    - **R9 — K05 partial borrows:** K05 (Element trait → context object) introduces `&mut PaintCx<'_>` / `&mut LayoutCx<'_>`. Under K07's monolithic AppCell, K05 will need either (a) sub-cell sharding (Slint/Dioxus pattern) or (b) temp `&mut App` field-projection. K07 does NOT preclude (a). Document the deferred decision.
    - **R10 — Phase III multi-threaded UI (iOS UIKit Main + Background Renderer, Android UI thread + GL thread):** K07's `_not_send: PhantomData<*const ()>` permanently blocks `App: Send`. If Phase III ever wants `App: Send`, AppCell would need full redesign (`Mutex<App>` or thread-affinity assertion). Document; not blocker now.
    - **R12 — Drop-on-panic:** new AppCell's `Drop` releases the borrow flag, BUT does NOT roll back partial mutations to App. Same as pre-K07 RefCell semantics. Module rustdoc MUST warn: "App is in best-effort consistent state after a panicking closure; for `catch_unwind`-based recovery, set `ReentryMode::Loose` and accept potential false-positive `NestedEntityUpdate` on the next `update_entity` after a caught panic."
    - **Sharding deferred to K06:** Servo / Slint / Dioxus / Floem all SHARD their cells (per-property / per-entity / per-resource). flui-v2 already shards `entities: EntityMap`. Future per-domain owners (BuildOwner / PipelineOwner / SemanticsOwner from K06) extend sharding. K07 stays monolithic — explicit choice.

- [x] **Task 6.** ✅ **(rev 5 — completed 2026-05-09.)** Three adversarial reviews dispatched in parallel on spec rev 1:
  - `flui-arch-reviewer` (38 tool-uses, 289s) — 7 findings: 2 BLOCKERs (Key Principle #8 hot-path reachability, WindowUpdateGuard/EntityUpdateGuard fabrication), 3 MAJORs (Task 14a EntityScope conflict, Unblocks K05 misleading, UnwindSafe inverted), 2 MINORs (TLCell language, observe_in/subscribe_in row missing). Plus 4 red flags incl. AppCell::new vs Rc::new_cyclic, AsyncContextAsMut dual-purpose.
  - `migration-risk-adversary` (70 tool-uses, 402s) — 22 findings + 5 silent regression vectors + 5 missing specifications: 3 BLOCKERs (test orphan, read_global semantic mismatch, 4 other as_mut sites), 7 MAJORs, 12 minor/silent. False positive: Cargo.lock contradiction (verified by api auditor as transitively present).
  - `rust-api-migration-auditor` (80 tool-uses, 564s) — 13 findings: 3 BLOCKERs (AsyncApp::app() cascade, test orphan, #[track_caller] on Drop no-op), 5 MAJORs (AsyncContextAsMut variant unreachable, pub Rc<AppCell> exposure, UnwindSafe inverted, pub use vs pub(crate) use, From<BorrowMutError> semver MAJOR), 5 MINORs.
  - All findings triaged in Task 7. Spec rev 2 published.

  Original Task 6 instructions retained for reference:
  - **`flui-arch-reviewer`** — verify the chosen primitive aligns with three-tier architecture and Key Principles 1/6/8/11; flag any drift from established conventions.
  - **`migration-risk-adversary`** — paranoid sweep: what functionality is lost / silently regressed when the **103 narrow AppCell-derived callsites** migrate? Specifically: subscription handler signatures, observer callbacks, drop-time runs, async-spawn paths, web event-loop integration.
  - **`rust-api-migration-auditor`** — semver impact, trait object safety, feature flag matrix, MSRV idiom usage, `unsafe` audit.
  - Goal: each agent returns a list of BLOCKER / MAJOR / MINOR findings with file:line citations.

- [x] **Task 7.** ✅ **(rev 5 — completed 2026-05-09.)** Comprehensive triage of 8 BLOCKERs + 12 MAJORs + 10 MINORs. ALL BLOCKERs patched into spec rev 2 + plan rev 5. ALL MAJORs absorbed (most into spec rev 2 directly; semver MAJOR documented in Task 32a CHANGELOG). MINORs: TLCell language softened; "3 → 4" unsafe block count corrected; Tree Borrows vs Stacked Borrows separated. False positive (Cargo.lock contradiction) identified via cross-reviewer triangulation. Spec rev 2 Decision log §"Revision 2 (post-Task 6 adversarial review)" enumerates all changes.

  Original Task 7 instructions retained for reference:

### Phase 1.5 — pre-implementation audits (rev 6 NEW)

- [x] **Task 7a. (NEW — rev 6, quality review)** Audit whether `try_borrow` (shared) has any genuine callers in flui-core. Steps:
  - `grep -rEn 'app.*\.borrow\(\)' crates/flui-core/src/` to find shared-borrow patterns.
  - For each hit, verify whether the call is actually shared (e.g., `app.borrow().platform.clone()` at app.rs:190 — borrow is held briefly for read-only access).
  - Decision criterion: if ZERO genuine shared-borrow callsites remain after migration, drop `BorrowState::Shared(NonZeroU32)` variant entirely. Cell becomes `enum BorrowState { Free, Mut }` — half the state machine, half the unsafe surface, simpler proptest.
  - If ≥1 genuine site remains, KEEP `BorrowState::Shared`. Document the audit result in `.k07-shared-borrow-audit.md` (gitignored).
  - Output: decision recorded in spec rev 4 (if rev 3 → 4 transition needed) OR plan note.
  - **Done criterion:** explicit decision recorded; if dropping Shared, Task 8's `BorrowState` enum simplified accordingly.
  - **Plan note (2026-05-10):** `rg` found genuine shared app borrows in `Application` read-only helpers (`app.rs:190`, `:203`, `:224`, `:229`, `:234`, `:239`), `init_app_menus(platform.as_ref(), &app.borrow())` (`app.rs:771`), async read paths (`app/async_context.rs:81`, `:112`), and executor test setup (`executor.rs:573`). Decision: KEEP `BorrowState::Shared(NonZeroU32)` and the `try_borrow`/`borrow` shared path. Detailed audit written to `.k07-shared-borrow-audit.md` and ignored in `.gitignore`.

- [x] **Task 7b. (NEW — rev 7, aif-improve drift cleanup)** Scrub actionable contradictions from `docs/superpowers/specs/2026-05-09-K07-appcell-removal-design.md` before implementation starts. Steps:
  - Verify actionable-current design sections say K15 uses inline `window_update_stack` / `currently_updating_entity` guards, not nonexistent `WindowUpdateGuard` / `EntityUpdateGuard` types. Historical decision-log references may remain when clearly framed as rejected/superseded history.
  - Verify Q9 says **NO manual `UnwindSafe` / `RefUnwindSafe` impls**; auto-trait behavior is locked by `static_assertions`.
  - Verify the type sketch uses `std::panic::panic_any(e)` for typed `ReentryError` panic payloads where the public API cannot return `Result`.
  - Verify the unsafe audit says 2 projection blocks in `app/cell.rs` plus raw-pointer field-guard writes in `app.rs`, with no unsafe impl blocks.
  - Verify actionable-current sections (semver, type surface, testing strategy, Known Limitations, Done criteria) no longer use formatted-string panic payloads for structured K07 errors, no longer claim the deleted K15 `BorrowMutError` conversion test survives unchanged, and no longer use legacy AppCell / `this.upgrade()` count estimates as current facts.
  - **Done criterion:** targeted grep/review checks find no actionable-current references to manual auto-trait impls, formatted-string panic payloads for K07 structured errors, stale AppCell callsite counts, stale `this.upgrade()` counts, or unchanged-K15-test language. Historical superseded notes may remain in revision records when they are explicitly labeled as superseded.
  - **Plan note (2026-05-10):** fixed the remaining active Candidate B unsafe-count drift in the design spec; Task 14a later refined this to 2 projection blocks in `app/cell.rs` plus 3 raw-pointer field-guard writes in `app.rs`. Follow-up grep for `3-5 blocks`, `Auditable in one file`, stale plan-done counts, and stale AppCell count language returned no actionable-current matches; remaining `WindowUpdateGuard` / `panic!("{}", e)` / `unsafe impl UnwindSafe` hits are historical or explicitly negative/superseded guidance.

- [x] **Task 8a. (NEW — rev 6, api-auditor MAJOR)** Add `static_assertions = "1"` to `crates/flui-core/Cargo.toml` `[dev-dependencies]`. Verify:
  - Crate is already in `Cargo.lock` transitively via `postage v0.5.0` (no lockfile modification expected).
  - Run `cargo check -p flui-core --tests` after adding to verify resolution.
  - **Note:** the workspace policy "Does NOT modify Cargo.lock" applies to runtime deps; dev-deps that are already in the lockfile transitively don't trigger the policy.
  - File: `crates/flui-core/Cargo.toml` `[dev-dependencies]` section.
  - **Plan note (2026-05-10):** added `static_assertions = "1"` to `crates/flui-core/Cargo.toml` dev-dependencies. Verified `Cargo.lock` already contained `static_assertions`; `cargo check -p flui-core --tests` passed.

### Phase 2 — Public type surface (recommended Candidate B; pivot if Task 5 chooses A or C)

- [x] **Task 8.** Create `crates/flui-core/src/app/cell.rs` (NEW module owned by `app/`). Contents:
  - `pub struct AppCell { app: UnsafeCell<App>, borrowed: Cell<BorrowState>, _not_send: PhantomData<*const ()> }`.
  - `enum BorrowState { Free, Mut, Shared(NonZeroU32) }` with documented transitions and saturation behavior at `Shared(u32::MAX)` (return `Err(ReentryError::AppBorrowed)` rather than panic-on-overflow — this is a structural impossibility but pinned as a regression guard).
  - `impl AppCell { pub fn new(app: App) -> Rc<Self>; pub fn borrow(&self) -> AppRef<'_>; pub fn borrow_mut(&self) -> AppRefMut<'_>; pub fn try_borrow_mut(&self) -> Result<AppRefMut<'_>, ReentryError>; pub fn try_borrow(&self) -> Result<AppRef<'_>, ReentryError>; }`. **Note (rev 4):** `try_borrow_mut` returns `Result<_, ReentryError>` directly (NOT via `BorrowMutError` intermediate); plan Task 9's `From<BorrowMutError>` deletion is symmetric with this design.
  - `pub struct AppRef<'a>` and `pub struct AppRefMut<'a>` — both `Deref<Target=App>` (and `DerefMut` for the latter), with `Drop` clearing `BorrowState`.
  - **SAFETY comment per `unsafe` block** — every `unsafe { &*cell.app.get() }` / `unsafe { &mut *cell.app.get() }` derive must annotate (a) the borrow flag has been transitioned to the corresponding state, (b) no aliased reference is live, (c) reference borrow lasts strictly less than the guard's `Drop`. Cite `std::cell::RefCell` source as the canonical model.
  - **Stacked Borrows / Tree Borrows discipline (rev 4 R1 mitigation):** every projection MUST go through ONE root reference (`cell.app.get()`) — never derive `&mut` from one path and `&` from another in overlapping scope. Reference RustBelt `RefCell` proof.
  - `log::trace!(target: "flui_core::app::cell", "borrow_mut acquired")` on each acquire with `#[track_caller]` source; `log::trace!` on drop; `log::warn!` on contention (post-K15, contention is forbidden — but the cell still emits a log for auditability when running outside `cfg(test)`).
  - `option_env!("TRACK_THREAD_BORROWS")` shim removed; replaced by `log::trace!` (Known Limitation #3).
  - **Re-export rule:** `AppCell`, `AppRef`, `AppRefMut` keep `#[doc(hidden)]` (matches current shape). Public API is the methods on `Application` / `App`.
  - **Compile-time auto-trait tests (rev 4 R2/R3/R4 mitigation):** add `#[cfg(test)]` module with:
    ```rust
    static_assertions::assert_not_impl_any!(AppCell: Send, Sync);
    static_assertions::assert_not_impl_any!(AppRef<'static>: Send, Sync);
    static_assertions::assert_not_impl_any!(AppRefMut<'static>: Send, Sync);
    // UnwindSafe behavior — no manual widening:
    static_assertions::assert_not_impl_any!(AppCell: std::panic::UnwindSafe, std::panic::RefUnwindSafe);
    ```
    If `static_assertions` not yet in dev-deps, add it (single-use is fine). All three negative assertions are MANDATORY — `App: !Send + !Sync` invariant must propagate compile-time.
  - **`#[track_caller]` propagation (rev 4 R4 mitigation, rev 5 amendment):** every `borrow*` method on `AppCell` MUST carry `#[track_caller]`. **Drop impls MUST NOT** — `#[track_caller]` on `Drop::drop` is a no-op per Rust semantics (drop glue does not carry caller Location). Logging from `Drop` records the drop-glue's internal location, not the borrow callsite. This is acceptable because acquire-side methods supply the diagnostic info.
  - **`Rc::new_cyclic` integration (rev 5 — adversarial review BLOCKER #8):** **NO standalone `AppCell::new(app)` constructor.** `App.this: Weak<AppCell>` requires cyclic init. The cell is constructed inline in `App::new_app` via `Rc::new_cyclic(|this: &Weak<AppCell>| AppCell { app: UnsafeCell::new(App { this: this.clone(), … }), borrowed: Cell::new(BorrowState::Free), _not_send: PhantomData })`. No public constructor exposed.
  - **(rev 6 — adversarial review BLOCKER A, corrected during Task 8 implementation):** **NO manual `unsafe impl UnwindSafe` or `unsafe impl RefUnwindSafe` blocks, and no `AssertUnwindSafe` storage wrapper.** Facts:
    1. `unsafe impl UnwindSafe` is `error[E0199]` — `UnwindSafe` is NOT an `unsafe trait`. Same for `RefUnwindSafe`.
    2. `UnsafeCell<T>` has no negative `UnwindSafe` impl in std, but `App` itself is not `UnwindSafe` under Rust 1.95 because it contains platform trait objects and interior-mutable fields. Therefore post-K07 `AppCell` is `!UnwindSafe` and `!RefUnwindSafe` by auto-trait inference. K07 locks that compiler-derived behavior instead of widening it.
    Replace rev 5's `unsafe impl` blocks with:
    ```rust
    // Lock auto-trait behavior as regression guard — no manual impl or wrapper.
    static_assertions::assert_not_impl_any!(
        AppCell: std::panic::UnwindSafe,
                 std::panic::RefUnwindSafe,
    );
    ```
  - **`#[derive(Debug)]` on `BorrowState` (rev 5 — adversarial review MINOR):** required because `unreachable!("{:?}", other)` in `AppRef::Drop` formats `BorrowState`. Without Debug, the unreachable! arm is a compile error. Change `#[derive(Clone, Copy)]` to `#[derive(Clone, Copy, Debug)]`.
  - **(rev 6 — `debug_assert!` in Drop = abort vector mitigation):** `debug_assert!(matches!(self.cell.borrowed.get(), BorrowState::Mut))` in `AppRefMut::Drop` was specified in rev 1-5. If this assert fires during stack unwind from a panicking inner closure, double-panic → process abort. To avoid: gate with `#[cfg(debug_assertions)] { if !std::thread::panicking() { debug_assert!(...); } }` so the assertion is skipped during unwinding. Same treatment for `unreachable!("{:?}", other)` in `AppRef::Drop`.
  - File: `crates/flui-core/src/app/cell.rs` (new), `crates/flui-core/src/app.rs` (delete lines 75-135 incl. `AppCell`/`AppRef`/`AppRefMut`; replace with `mod cell; pub use cell::{AppCell, AppRef, AppRefMut};` — **`pub use`, NOT `pub(crate) use`** (rev 5 — adversarial review MAJOR M10): `lib.rs:125 pub use app::*` does not re-export `pub(crate)` items, breaking `HeadlessAppContext.app: Rc<AppCell>` field type accessibility for `test-support` consumers).
  - Module-level rustdoc explains the new contract: "AppCell is a single-mutable-borrow cell; recursive borrow_mut produces ReentryError::AppBorrowed via the K15 contract. Use cx.defer to schedule work that must touch App. Drop-on-panic releases the borrow flag but does NOT undo partial mutations to App — App is in best-effort consistent state after a panicking closure (matches pre-K07 RefCell semantics)."
  - **Plan note (2026-05-10):** implemented `app/cell.rs`, moved `AppCell`/`AppRef`/`AppRefMut` out of `app.rs`, wired `Rc::new_cyclic` to initialize `UnsafeCell<App>` plus `BorrowState::Free`, removed the `TRACK_THREAD_BORROWS` shim from AppCell, and verified `cargo check -p flui-core --tests` passes. During this check, the rev 6 `AppCell: UnwindSafe` assertion proved impossible for current `App`; Task 8 and the design spec now lock `AppCell: !UnwindSafe + !RefUnwindSafe` with `assert_not_impl_any!` and avoid `AssertUnwindSafe`.

- [x] **Task 9.** Update `crates/flui-core/src/reentrancy.rs`:
  - `ReentryError::AppBorrowed` already exists (K15). Confirm its Display still reads correctly under K07 ("App was already mutably borrowed (callback re-entered the runtime; use cx.defer)").
  - DELETE the `impl From<std::cell::BorrowMutError> for ReentryError` block at [reentrancy.rs:161-163](crates/flui-core/src/reentrancy.rs#L161); the new `AppCell` does NOT use `std::cell::BorrowMutError` — it returns `ReentryError::AppBorrowed` directly.
  - DELETE the `use std::cell::BorrowMutError;` import at [reentrancy.rs:76](crates/flui-core/src/reentrancy.rs#L76).
  - UPDATE module-level rustdoc that references `BorrowMutError` (lines 45, 71, 154 — verified by `grep -n BorrowMutError crates/flui-core/src/reentrancy.rs`):
    - Line 45 (contract matrix entry for `AsyncApp::run_update`): rephrase from "via `From<BorrowMutError> for ReentryError`" to "via `AppCell::try_borrow_mut() -> Result<_, ReentryError>` directly."
    - Line 71 (Known Limitation #4 about source location): rephrase from "`std::cell::BorrowMutError::location()` is nightly-only" to "[K07] `AppCell::try_borrow_mut()` returns `ReentryError::AppBorrowed` without source location; use `RUST_LOG=flui_core::app::cell=trace` for callsite context via `#[track_caller]`."
    - Line 154 (per-variant rustdoc on `ReentryError::AppBorrowed`): rephrase from "in stable Rust because [`std::cell::BorrowMutError::location`] is …" to "[K07] `AppCell::try_borrow_mut` returns this variant directly; no conversion from `std::cell::BorrowMutError` is performed."
  - **(rev 5 — adversarial review BLOCKER #4) MANDATORY: DELETE the test `borrow_mut_error_converts_to_app_borrowed`** at [reentrancy.rs:253-259](crates/flui-core/src/reentrancy.rs#L253). The test directly exercises the `From<BorrowMutError> for ReentryError` impl being deleted. Without explicit deletion in commit 5 (Tasks 8-10), the test becomes a compile error and Task 9's commit is broken. Plan rev 1-4 said "Update existing K15 tests if any reference BorrowMutError" with conditional "if any" — wrong, this test is non-optionally affected. The replacement test for the new direct path lives in Task 26 (`prop_borrow_mut_then_borrow_mut_returns_app_borrowed_in_strict`).
  - **(rev 6 — adversarial review BLOCKER C: atomic with `async_context.rs:96` update)** Task 9 MUST also update `crates/flui-core/src/app/async_context.rs:94-96` in the SAME commit. Current code:
    ```rust
    let mut lock = app
        .try_borrow_mut()
        .map_err(crate::reentrancy::ReentryError::from)?;
    ```
    After Task 9 deletes `From<BorrowMutError> for ReentryError`, this `.map_err` call won't compile. Update to:
    ```rust
    let mut lock = app.try_borrow_mut()?;
    ```
    Because `AppCell::try_borrow_mut` now returns `Result<_, ReentryError>` directly. Atomic with Task 9 — NOT deferred to Task 15. Commit 5 (Tasks 8-10) MUST be self-contained-green.
  - VERIFY no remaining caller relies on the `From` conversion: `grep -rn 'BorrowMutError' crates/flui-core/src/` MUST return zero hits after Task 9 lands.
  - DECIDE (in Task 5 spec, applied here): whether to add a new variant `ReentryError::AppGoneAway` for the (rare) case of `Weak::upgrade()` returning `None` when the App is mid-drop. Currently `AsyncApp::app() at async_context.rs:32` does `expect("app was released before async operation completed")` — a raw panic. K07 either (a) keeps the panic shape, (b) widens AsyncApp methods to `Result` carrying `ReentryError::AppGoneAway`, or (c) introduces the variant but only used inside `try_borrow_mut` chains. Spec resolves.
  - VERIFY the surviving K15 reentrancy tests still pass, and ensure Task 26 adds the direct AppCell contention replacement for the deleted `borrow_mut_error_converts_to_app_borrowed` test.
  - Files: `crates/flui-core/src/reentrancy.rs`. No new dependencies.
  - **Plan note (2026-05-10):** removed `From<BorrowMutError> for ReentryError`, deleted the `borrow_mut_error_converts_to_app_borrowed` test, updated reentrancy docs, and changed `app/async_context.rs` to `app.try_borrow_mut()?`. Verified `rg -n "BorrowMutError|map_err\\(crate::reentrancy::ReentryError::from\\)|borrow_mut_error_converts_to_app_borrowed" crates/flui-core/src` returns no hits and `cargo check -p flui-core --tests` passes.

- [x] **Task 10.** Update `crates/flui-core/src/prelude.rs` — no change unless the design spec exposes a new public type. The existing `ReentryError` and `ReentryMode` re-exports from K15 stay.
  - **Plan note (2026-05-10):** no prelude change needed. `prelude.rs` already re-exports `ReentryError` and `ReentryMode`; `AppCell`/`AppRef`/`AppRefMut` remain doc-hidden runtime internals via `app::*`, not prelude items.

### Phase 3 — Internal migration: `App`, `Application`

- [x] **Task 11.** Migrate `App::new_app` ([app.rs:684](crates/flui-core/src/app.rs#L684)) to return the new `Rc<AppCell>` shape (signature unchanged — both shapes are `Rc<AppCell>`; only the internals differ).
  - **Plan note (2026-05-10):** completed with Task 8. `App::new_app` still returns `Rc<AppCell>`, but `Rc::new_cyclic` now constructs `AppCell { app: UnsafeCell::new(App { ... }), borrowed: Cell::new(BorrowState::Free), _not_send: PhantomData }`. `cargo check -p flui-core --tests` passed.

- [x] **Task 12.** Migrate `Application` ([app.rs:139-241](crates/flui-core/src/app.rs#L139)). Each `self.0.borrow()` / `self.0.borrow_mut()` callsite (lines 160, 170, 179, 190, 192, 203, 213-216, 224, 229, 234, 239) becomes a method call on the new `AppCell`. Mass replace; semantics preserved.
  - **Plan note (2026-05-10):** no callsite rewrite was needed beyond the Task 8 type swap because the new `AppCell` preserves the same `borrow()` / `borrow_mut()` method surface. Verified the `Application` callsites still target the new `AppCell` methods and `cargo check -p flui-core --tests` passed.

- [x] **Task 13.** Migrate `App::this: Weak<AppCell>` consumers. `git grep 'this.upgrade()' crates/flui-core/src` to enumerate; expect **5 distinct sites** (`app.rs:215`, `app/context.rs:74,110,166`, `app/test_context.rs:660` per `.k07-recon.txt`). Each `Weak::upgrade()? .borrow_mut()` becomes `Weak::upgrade()? .borrow_mut()` against the new cell — identical at the use site. Verify by `cargo check` after this task.
  - **Plan note (2026-05-10):** verified the 5 `this.upgrade()` sites (`app.rs`, `app/context.rs` x3, `app/test_context.rs`). No source rewrite needed because the upgraded `Weak<AppCell>` still exposes the same `borrow_mut()` method. `cargo check -p flui-core --tests` passed.

- [x] **Task 14.** Audit `App::pending_effects` queue / drain pathway ([app.rs:603 + 1389-1424](crates/flui-core/src/app.rs#L603)) for any reliance on `RefCell::borrow_mut` panic shape. K15 uses inline `window_update_stack` / `currently_updating_entity` checks plus `EntityMap::double_lease_panic` — there are no `WindowUpdateGuard` / `EntityUpdateGuard` types. Confirm the surviving K15 reentrancy tests plus the Task 26 replacement AppCell test pass under the new primitive.
  - **Plan note (2026-05-10):** audited `pending_updates` / `flush_effects` / `pending_effects` paths and found no dependency on the old `RefCell::borrow_mut` panic payload. Added the direct replacement test `app_cell_try_borrow_mut_reports_app_borrowed_directly` in `reentrancy.rs`. Verified `cargo test -p flui-core reentr --tests` (13 passed) and `cargo check -p flui-core --tests`.

- [x] **Task 14a.** **(rev 4 — closes K15 Known Limitation #6.)** Fix the panic-leak class on `currently_updating_entity` and `window_update_stack` fields. Per K15 design spec line 202: *"No panic-safety on `currently_updating_entity` and `window_update_stack` fields on the panic-during-update path — same as the pre-K15 manual push/pop pattern. Acceptable parity; not a regression."* And K15 inline comment at [app.rs:2483-2496](crates/flui-core/src/app.rs#L2483): *"RAII guards were considered and rejected during planning because they conflict with Rust borrow rules — a guard borrowing `&mut App` cannot coexist with `App` flowing through this closure body. **Fixing both panic-leak classes is K07's job (it redesigns the borrow primitive).**"*
  - **(rev 5 — adversarial review BLOCKER #3) Re-scope: NO `EntityScope` RAII.** Two reviewers independently confirmed the `EntityScope { app: &'a mut App }` pattern proposed in rev 4 does NOT compile. The borrow conflict is the same as K15 documented at app.rs:2488-2491: "RAII guards … conflict with Rust borrow rules — a guard borrowing `&mut App` cannot coexist with `App` flowing through this closure body." Candidate B's `AppRefMut<'_>` does NOT resolve this because `cx: &mut App` is still passed into the inner closure body via `self.update(|cx| { … })`.
  - **(rev 6 — adversarial review BLOCKER B) `catch_unwind` was WRONG.** `resume_unwind` propagates through `App::update`'s frame, skipping `finish_update`, leaking `pending_updates`. Silent permanent regression: after one caught panic, `flush_effects` guard `if pending_updates == 1` never fires.
  - **Actual fix path (rev 6 — raw-pointer field-projection guard):**
    ```rust
    fn update_entity<T: 'static, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R {
        let id = handle.entity_id();
        if self.currently_updating_entity == Some(id) {
            // K15 same-entity check (unchanged)
            let err = ReentryError::NestedEntityUpdate(id);
            log_reentry(self.reentry_mode, &err);
            std::panic::panic_any(err);  // typed payload (rev 6)
        }
        self.update(|cx| {
            // rev 6: raw-pointer field-projection guard. Holds *mut, not &mut —
            // no borrow conflict with closure body's `cx`. Drop runs during
            // stack unwind (panic) without crossing App::update frame, preserving
            // pending_updates / finish_update semantics.
            let prev = cx.currently_updating_entity.replace(id);
            struct Guard {
                ptr: *mut Option<EntityId>,
                prev: Option<EntityId>,
            }
            impl Drop for Guard {
                fn drop(&mut self) {
                    // SAFETY: ptr derived from &mut cx.currently_updating_entity
                    // earlier in the closure body; cx is alive for entire enclosing
                    // self.update(|cx| { … }) scope; this Drop runs strictly before
                    // cx exits scope. No aliased reference exists because Guard
                    // holds only the raw pointer, not a borrow. Single-threaded
                    // !Send + !Sync App — no concurrent access.
                    unsafe { *self.ptr = self.prev; }
                }
            }
            let _guard = Guard {
                ptr: &mut cx.currently_updating_entity as *mut _,
                prev,
            };
            // closure body — cx STILL USABLE because guard holds *mut not &mut
            let mut entity = cx.entities.lease(handle);
            let r = update(&mut entity, &mut Context::new_context(cx, handle.downgrade()));
            cx.entities.end_lease(entity);
            r
            // _guard's Drop runs here unconditionally (normal return) OR during
            // stack unwind (panic) — restoring currently_updating_entity to prev.
        })
    }
    ```
  - **`window_update_stack` panic-safety** (rev 6 — same pattern, distinct guard):
    ```rust
    // Inside App::update_window_id closure body, after window taken:
    cx.window_update_stack.push(window.handle.id);
    struct StackGuard { stack: *mut Vec<WindowId> }
    impl Drop for StackGuard {
        fn drop(&mut self) {
            // SAFETY: same justification as Guard above; stack ptr derived from
            // &mut cx.window_update_stack within the closure body.
            unsafe { (*self.stack).pop(); }
        }
    }
    let _stack_guard = StackGuard { stack: &mut cx.window_update_stack as *mut _ };
    let result = update(root_view, &mut window, cx);
    // Note: K15's "pop FIRST before observers" invariant — when `trail()` runs
    // normally, _stack_guard's Drop runs at end of closure scope, AFTER trail()
    // has finished (which already pops at line 1611). Need to verify that
    // double-pop is safe (will pop empty stack — must guard with len check OR
    // make _stack_guard a "panic-only" guard with an early-disarm helper).
    // Leaning toward: disarm helper called by trail() once normal pop runs.
    if let Some(()) = trail(id, window, cx) {
        // Normal path — disarm guard so its Drop is no-op.
        // Implementation detail: convert _stack_guard's Drop to check a flag.
        // OR use std::mem::forget(_stack_guard) here.
        std::mem::forget(_stack_guard);
        Some(result)
    } else { None }
    ```
    Slightly more delicate due to `trail()`'s existing inline pop. Implementation choice (recorded in spec rev 3): use `std::mem::forget` after `trail()` succeeds. On panic before `trail()` completes, guard's Drop pops the stack. Two-pop scenario (guard pops AFTER trail's pop) is prevented by `mem::forget`.

  - **NO `catch_unwind` / `resume_unwind` / `AssertUnwindSafe`** — entirely replaced.

  - **Stale rev 5 `catch_unwind` text deleted (rev 7):** do NOT reintroduce `catch_unwind`, `resume_unwind`, or `AssertUnwindSafe` for the implementation path. The rejected rev 5 sketch remains only in the refinement record, not in this actionable task.
  - **Test addition (Task 26 expansion):** new property tests `prop_currently_updating_entity_restored_after_panic`, `prop_window_update_stack_restored_after_panic`, and `prop_pending_updates_zero_after_panic_through_update_entity`. The tests may use `std::panic::catch_unwind` at the test boundary to observe post-panic state; production code must use raw-pointer field-projection guards, not catch/resume.
  - **NO `scopeguard` dep** — the guard is a local raw-pointer field-projection type with a small `Drop` impl. Phase 0-K dep-minimization preserved.
  - File: `crates/flui-core/src/app.rs:2469-2505` (update_entity), `1559-1641` (update_window_id including trail()), `1080-1115` (open_window). Optional logging: `log::trace!(target: "flui_core::app::reentry", "currently_updating_entity/window_update_stack restored during unwind");` inside guard `Drop` only if it does not allocate or panic.
  - **Done criterion:** surviving K15 tests plus `prop_currently_updating_entity_restored_after_panic`, `prop_window_update_stack_restored_after_panic`, and `prop_pending_updates_zero_after_panic_through_update_entity` are green. K15 spec line 202 stated "Acceptable parity; not a regression"; K07 closes Limitation #6 explicitly via raw-pointer field-projection guards.
  - **Plan note (2026-05-10):** implemented `CurrentEntityGuard`, `WindowUpdateStackGuard`, and `PendingUpdateGuard` in `app.rs`. The third guard was required by the task's `pending_updates_zero_after_panic_through_update_entity` criterion. Added focused panic-recovery tests: `currently_updating_entity_and_pending_updates_restore_after_panic` and `window_update_stack_restores_after_open_window_panic`. Verified `cargo test -p flui-core reentr --tests` (15 passed) and `cargo check -p flui-core --tests`. Updated the design spec unsafe audit to 2 cell projections + 3 app.rs raw-pointer guards.

- [x] **Task 14b. (NEW — rev 5; rev 6 — UNCONDITIONALLY PR-blocking; arch-reviewer MAJOR 3)** Paint/dispatch hot-path reachability audit for `try_borrow_mut`. ARCHITECTURE Key Principle #8 compliance is a CORRECTNESS invariant, not optional.
  - **Scope expanded (rev 6):** grep includes `crates/flui-core/src/elements/` observer dispatch paths AND `crates/flui-core/src/window.rs` (Window::draw / Window::dispatch_event), NOT just direct `Window::*` callsites.
  - Steps:
    1. `grep -rEn 'try_borrow_mut|borrow_mut' crates/flui-core/src/`
    2. For each hit, trace caller upward through `cargo expand` if needed to determine reachability from `Window::draw` / `Window::dispatch_event` / `Element::paint` / observer dispatch.
    3. Build a callgraph table: file:line → caller chain → hot-path reachability (yes/no) → expected frequency.
  - **Output committed to `docs/superpowers/audits/K07-hot-path-audit.md`** (NOT gitignored — durable record for Tasks 42-44 re-review).
  - **If audit finds ANY reachable hits**: spec rev 3+ Known Limitations section escalated; route reachable calls through deferred-effect path OR document the per-frame perf cost.
  - **If audit finds zero hits**: spec's permissive interpretation of Key Principle #8 stands; document audit conclusion explicitly.
  - **Status: PR-blocking unconditionally.** Audit MUST run and produce a definitive answer (yes/no with citations) before K07 PR can merge.
  - **Plan note (2026-05-10):** audit completed in `docs/superpowers/audits/K07-hot-path-audit.md`. Conclusion: no AppCell `try_borrow_mut` / `borrow_mut` path is reachable from `Window::draw`, `Window::dispatch_event`, `Element::paint`, or observer dispatch hot paths. Hot-path `borrow_mut()` hits are local `RefCell` state, not AppCell. `cargo check -p flui-core --tests` passed.

### Phase 4 — Internal migration: `AppContext` implementors

> **Tasks 15-19 are PARALLEL-SAFE.** Each touches a separate file (`app/async_context.rs`, `app/test_context.rs`, `app/test_app.rs`, `app/headless_app_context.rs`, `app/visual_test_context.rs`). They share no symbols beyond the `AppCell` type itself (Task 8). Run as 5 concurrent tasks once Task 14 lands.

- [x] **Task 15.** [PARALLEL with 16-19] Migrate `AsyncApp` ([app/async_context.rs](crates/flui-core/src/app/async_context.rs)). 15+ `borrow_mut()` callsites. Per the K15 deferral, this is also where the 10+ unstructured sites get structured:
  - Lines 39, 45, 55, 65 (`AsyncApp::*` constructors / converters): convert `borrow_mut()` to `try_borrow_mut()?` (or whatever Task 5 decides for non-`Result` returners). May require ascending the return type to `Result`.
  - Line 95: already `try_borrow_mut()` — confirms migration is mechanical.
  - Lines 126, 135, 152, 168, 182: convert per the design-spec migration table.
  - Lines 210, 219, 228, 242, 253: read-only or write paths — confirm semantics.
  - **`AsyncApp::as_mut`** at [async_context.rs:73](crates/flui-core/src/app/async_context.rs#L73): apply Q2 decision (panic with structured Display via `ReentryError::AsyncContextAsMut`).
  - **(rev 5 — adversarial review BLOCKER #7) THREE OTHER `as_mut` panic sites in the AppContext implementor zoo also need the structured Display:**
    - [`async_context.rs:412`](crates/flui-core/src/app/async_context.rs#L412) (AsyncWindowContext) — current message: `"Cannot use as_mut() from an async context, call 'update'"`.
    - [`test_context.rs:68`](crates/flui-core/src/app/test_context.rs#L68) (TestAppContext) — current message: distinct from above.
    - [`headless_app_context.rs:230`](crates/flui-core/src/app/headless_app_context.rs#L230) (HeadlessAppContext) — current message: distinct.
    - [`visual_test_context.rs:430`](crates/flui-core/src/app/visual_test_context.rs#L430) (VisualTestContext) — current message: distinct.
    All four convert to `std::panic::panic_any(ReentryError::AsyncContextAsMut)` (rev 6 — typed payload, NOT `panic!("{}", e)` which produces `String`). `catch_unwind` callers can `downcast_ref::<ReentryError>()`. **Note** (per spec Q2 rev 3): `AsyncContextAsMut` Display rephrased context-agnostic ("AppContext::as_mut is forbidden in async/test/headless context types"); same Display works correctly when raised from any of the 5 sites. Variant is reachable ONLY as panic payload (never returnable from a `Result` because `AppContext::as_mut` returns `GpuiBorrow<'a, T>` directly per Q8). Rustdoc on the variant marks it "Panic-only variant".
  - **(rev 5 — adversarial review BLOCKER #6) `AsyncApp::app()` cascade policy:** `app()` is private, widens to `Result<Rc<AppCell>, ReentryError::AppGoneAway>`. Public AsyncApp methods absorb the cascade per Q4:
    - Methods returning `Result<T>`: propagate via `?` (e.g., `update_window`, `read_window`, `read_entity`).
    - Methods returning `T` (e.g., `new`, `reserve_entity`, `insert_entity`, `update_entity`, `read_global`, `update_global`, `has_global`, `refresh`): wrap with `match self.app() { Ok(rc) => …, Err(e) => std::panic::panic_any(e) }`. **rev 6 — typed payload via `panic_any`, NOT `panic!("{}", e)`** (which would produce `String` and lose `ReentryError` type identity at `catch_unwind.downcast_ref::<ReentryError>()`). Net effect: panic semantics preserved, panic payload typed.
  - **`AsyncApp::app() -> Rc<AppCell>`** at line 29: returns `Option<Rc<AppCell>>` already (returns `Rc` on upgrade success — verify).
  - File: `crates/flui-core/src/app/async_context.rs`. New tests added in Phase 6.
  - **Plan note (2026-05-10):** `AsyncApp::app()` now returns `Result<Rc<AppCell>, ReentryError::AppGoneAway>`, with `app_or_panic()` preserving panic semantics for non-`Result` methods via typed `panic_any`. Result-returning methods (`update_window`, `read_window`, `open_window`) propagate structured errors. `AsyncApp::as_mut` and `AsyncWindowContext::as_mut` now panic with `ReentryError::AsyncContextAsMut`. Added both variants to `ReentryError`. `cargo check -p flui-core --tests` passed.

- [x] **Task 16.** [PARALLEL with 15, 17-19] Migrate `TestAppContext` ([app/test_context.rs](crates/flui-core/src/app/test_context.rs)). 19+ callsites. `app: Rc<AppCell>` field type unchanged. Verify `Rc::downgrade(&self.app)` at [test_context.rs:425](crates/flui-core/src/app/test_context.rs#L425) still produces a valid `Weak<AppCell>` for the `to_async()` path.
  - **Plan note (2026-05-10):** callsites compile unchanged against the new `AppCell` method surface; `as_mut` now uses typed `ReentryError::AsyncContextAsMut`. Verified `Rc::downgrade(&self.app)` remains at the `to_async()` path and `cargo check -p flui-core --tests` passed.

- [x] **Task 17.** [PARALLEL with 15-16, 18-19] Migrate `TestApp` ([app/test_app.rs](crates/flui-core/src/app/test_app.rs)). 8 callsites. Two `app: Rc<AppCell>` fields (lines 41 and 322 in different impl blocks) unchanged. Verify `Rc::downgrade(&self.app)` at [test_app.rs:226](crates/flui-core/src/app/test_app.rs#L226) still produces a valid `Weak<AppCell>`.
  - **Plan note (2026-05-10):** no source rewrite needed; callsites compile against the new `AppCell` surface. Verified `Rc::downgrade(&self.app)` remains in `to_async()` and `cargo check -p flui-core --tests` passed.

- [x] **Task 18.** [PARALLEL with 15-17, 19] Migrate `HeadlessAppContext` ([app/headless_app_context.rs](crates/flui-core/src/app/headless_app_context.rs)). 11 callsites. `app: Rc<AppCell>` field unchanged.
  - **Plan note (2026-05-10):** callsites compile unchanged against the new `AppCell` method surface; `as_mut` now uses typed `ReentryError::AsyncContextAsMut`. `cargo check -p flui-core --tests` passed.

- [x] **Task 19.** [PARALLEL with 15-18] Migrate `VisualTestContext` ([app/visual_test_context.rs](crates/flui-core/src/app/visual_test_context.rs)). 11 callsites. `app: Rc<AppCell>` field unchanged.
  - **Plan note (2026-05-10):** callsites compile unchanged against the new `AppCell` method surface; `as_mut` now uses typed `ReentryError::AsyncContextAsMut`. `cargo check -p flui-core --tests` passed.

### Phase 5 — Internal migration: Element / Window / platform / subscription

- [x] **Task 20.** Migrate `crates/flui-core/src/elements/`. Files: `uniform_list.rs` (6+5=11), `div.rs` (8+11=19), `list.rs` (14+6=20), `text.rs` (3+9=12). Combined: ~62 sites. Mass-replace; verify `cargo check` after each file.
  - **Plan note (2026-05-10):** no AppCell migration needed in `elements/`. The many `borrow_mut()` hits are local element/scroll/text `RefCell` state, not `Rc<AppCell>` or `Weak<AppCell>`. Verified with AppCell-specific grep over `crates/flui-core/src/elements` and `cargo check -p flui-core --tests`.

- [x] **Task 21.** Migrate `crates/flui-core/src/subscription.rs` (3 sites). Cross-check K15 `SubscriberSet::retain` snapshot pattern still holds.
  - **Plan note (2026-05-10):** no AppCell migration needed. `subscription.rs` owns a local `Rc<RefCell<SubscriberSetState<...>>>`; the K15 snapshot pattern in `SubscriberSet::retain` is unchanged. Verified with grep and `cargo check -p flui-core --tests`.

- [x] **Task 22.** Migrate `crates/flui-core/src/executor.rs` test fixture only. The site is [executor.rs:556-573](crates/flui-core/src/executor.rs#L556) — `#[cfg(test)] fn create_test_app() -> (TestDispatcher, BackgroundExecutor, Rc<crate::AppCell>)` constructs an `Rc<AppCell>`; line 573 calls `app.borrow().foreground_executor.clone()`. NO production-code AppCell access. Migration: preserve the `Rc<AppCell>` return type (test-support surface invariant); replace `app.borrow()` with the new cell's shared-borrow API. Note: line 581's `*task_ran.borrow_mut() = true` is a `RefCell<bool>` (NOT AppCell-derived) — leave untouched.
  - **Plan note (2026-05-10):** no source rewrite needed; `app.borrow()` now resolves to the new shared-borrow API and the `RefCell<bool>` fixture borrow is unrelated. `cargo check -p flui-core --tests` passed.

- [x] **Task 23.** Migrate `crates/flui-core/src/platform/` (9 files, ~72 sites in upper-bound count — narrow set is much smaller because most `borrow_mut` here is `RefCell<WindowState>` etc., not AppCell). Sub-tasks:
  - `app_menu.rs` (3+3 wide-pattern sites — verify which are AppCell-derived)
  - `linux/headless/client.rs` (1)
  - `linux/x11/client.rs` (53+8 wide-pattern, mostly NON-AppCell — Wayland-style RefCell on internal state)
  - `linux/wayland/client.rs` (10+2 wide-pattern, mostly NON-AppCell)
  - `mac/platform.rs` — re-confirm K15 deferral comments still apply
  - `windows/platform.rs` — re-confirm K15 deferral comment still applies
  - **Update** all three K15 platform-deferral comments to reference K07 with EXACT wording (replace, don't append):
    - `mac/platform.rs:500-502` (`Platform::quit` close-callback deferral): "Defer the close callback to satisfy the K07 (post-AppCell) re-entrancy contract; calling `Platform::quit` while the new `AppCell` is mutably borrowed produces `ReentryError::AppBorrowed`."
    - `mac/platform.rs:1254` (thermal-state deferral): "Defer the thermal-state observer to satisfy the K07 (post-AppCell) re-entrancy contract; running the observer synchronously while the new `AppCell` is mutably borrowed produces `ReentryError::AppBorrowed`."
    - `windows/platform.rs:452-453` (close-callback deferral): "Defer the close callback to satisfy the K07 (post-AppCell) re-entrancy contract; calling `Platform::quit` while the new `AppCell` is mutably borrowed produces `ReentryError::AppBorrowed`."
  - File-by-file commits to keep diffs reviewable.
  - **Plan note (2026-05-10):** verified the AppCell-derived platform sites are limited to `platform/app_menu.rs` menu callbacks plus the documented macOS/Windows deferral comments. `app_menu.rs` callsites compile unchanged against the new AppCell method surface. Updated the macOS and Windows comments to the exact K07 post-AppCell wording requested. Other platform `borrow_mut()` hits are backend-local `RefCell` state. `cargo check -p flui-core --tests` passed.

- [x] **Task 24.** Examples comprehensive scan & migration. Steps:
  - `Glob examples/**/*.rs` to enumerate every example file in the workspace.
  - `Grep AppCell|app\.borrow_mut\(\)|app\.borrow\(\)` across the result.
  - Known: `crates/flui-core/examples/legacy/window.rs` (the 2 callsites K15 updated for `prompt`).
  - Migrate every match; verify each example still compiles via `cargo build --workspace --examples`.
  - If any example uses `AppCell` directly (rare — usually goes through `Application`), audit the use and update.
  - **Plan note (2026-05-10):** enumerated all example `.rs` files under `examples/` and `crates/flui-core/examples/`. `rg -n "AppCell|app\\.borrow_mut\\(\\)|app\\.borrow\\(\\)" examples crates/flui-core/examples` returned no hits. Verified `cargo build --workspace --examples` passed.

- [x] **Task 25.** Final scan (depends on Tasks 20, 21, 22, 23, 24 — all migrations complete). Steps:
  - Run all four Task-2 grep one-liners (Steps 2.1-2.4) against the post-migration tree.
  - Compare legacy AppCell/`RefCell<App>` count: MUST be zero. Candidate B deliberately preserves `AppCell::borrow()` / `borrow_mut()` / `try_borrow_mut()` spelling for compatibility; remaining AppCell borrow hits must resolve to the new `app::cell::AppCell`, and every non-AppCell `.borrow_mut()` / `.borrow()` hit must be against a non-AppCell `RefCell` (`Window`, `Keymap`, `Arena`, `Cell<bool>` test fixtures, etc.). Audit each remaining hit.
  - `grep -rn 'AppCell|AppRef|AppRefMut' crates/flui-core/src/`: every hit must be (a) the new module declaration, (b) the type definition, (c) `Rc<AppCell>` / `Weak<AppCell>` storage in the 12 known sites, or (d) the new module's tests.
  - `grep -rn 'BorrowMutError' crates/flui-core/src/`: MUST return zero hits (Task 9 deletion).
  - `grep -rn 'TRACK_THREAD_BORROWS' crates/flui-core/src/`: MUST return zero hits (Known Limitation #3 — replaced by `log::trace!`).
  - Document the post-migration counts in `.k07-recon-final.txt` (gitignored).
  - **Plan note (2026-05-10):** final scan captured in `.k07-recon-final.txt`. Counts: wide borrow pattern 756; narrow Candidate-B-compatible borrow pattern 101; `Rc<AppCell>` / `Weak<AppCell>` storage 11; `AppCell|AppRef|AppRefMut` symbol references 59. `rg -n "RefCell<App>|BorrowMutError|TRACK_THREAD_BORROWS" crates/flui-core/src` returns zero hits. Remaining narrow borrow hits are expected uses of the new `AppCell` API (Application/test/async/platform app-menu paths), one non-AppCell subscriber-set `RefCell` hit, and one rustdoc historical limitation line.

### Phase 6 — Test infrastructure & property tests

- [x] **Task 26.** Add `crates/flui-core/src/app/cell/tests.rs` (or `crates/flui-core/tests/app_cell.rs`) using existing `proptest` dev-dep. New tests:
  - `prop_borrow_mut_then_borrow_mut_returns_app_borrowed_in_strict` — random nested-borrow sequences; same-cell nesting returns `Err(ReentryError::AppBorrowed)`.
  - `prop_drop_releases_borrow` — random `borrow_mut → drop` sequences; subsequent borrow succeeds.
  - `prop_panic_during_borrow_releases_borrow` — `std::panic::catch_unwind` wraps a borrow that panics; assert subsequent borrow succeeds (panic-safety guarantee).
  - `prop_borrow_share_count_caps` (if Candidate B uses `Shared(NonZeroU32)`): saturating add at u32::MAX returns `Err`; documented as a structural impossibility but pinned as a regression guard.
  - **Port forward the surviving K15 reentrancy tests** to the new primitive; confirm all green. Task 9 deletes `borrow_mut_error_converts_to_app_borrowed`, so the K15 set is no longer "all 11 unchanged". Add the replacement direct-cell test here: `app_cell_try_borrow_mut_reports_app_borrowed_directly` (or equivalent) to cover the deleted `From<BorrowMutError>` path.
  - **Plan note (2026-05-10):** added `crates/flui-core/src/app/cell/tests.rs`, moved the compile-time auto-trait assertions into it, and added all four requested property tests. The direct replacement `app_cell_try_borrow_mut_reports_app_borrowed_directly` already exists in `reentrancy.rs` from Task 14. Verified `cargo test -p flui-core cell --tests` passes (5 tests) and `cargo test -p flui-core reentr --tests` passes (15 tests).

- [x] **Task 27.** Run Miri test for K07 cell module (`cargo +nightly miri test -p flui-core cell` with default Stacked Borrows). PR-blocking gate per Q6 resolution. Tree Borrows (`MIRIFLAGS=-Zmiri-tree-borrows`) added as separate non-blocking gate via Task 27a CI job. Capture both outputs.
  - **Plan note (2026-05-10):** initial Miri run exposed proptest file-failure persistence calling `GetCurrentDirectoryW` under Miri isolation on Windows, so `crates/flui-core/src/app/cell/tests.rs` now sets `failure_persistence: None`. Verified default Stacked Borrows `cargo +nightly miri test -p flui-core cell` passes (5 tests), and non-blocking Tree Borrows `$env:MIRIFLAGS='-Zmiri-tree-borrows'; cargo +nightly miri test -p flui-core cell` also passes (5 tests). Both runs emit pre-existing nightly `float_literal_f32_fallback` warnings in `crates/flui-core/src/taffy.rs`.

- [x] **Task 27a. (NEW — rev 6, api-auditor MAJOR; rev 7 CI prerequisites)** Add CI job to `.github/workflows/ci.yml` that runs Miri scoped to the cell module. Required because Q6 declares Miri PR-blocking but no CI infrastructure exists. The job must either install the same Linux system packages as the existing `check`/`test` jobs or use a verified `cargo +nightly miri test` feature set that avoids platform backends without weakening the cell coverage. Preferred job entry:
  ```yaml
  miri-cell:
    name: Miri (cell module — Stacked Borrows)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Linux dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libwayland-dev \
            libxkbcommon-dev \
            libxkbcommon-x11-dev \
            libfontconfig-dev \
            libegl-dev \
            libx11-dev \
            libx11-xcb-dev \
            libxcb-shape0-dev \
            libxcb-xfixes0-dev \
            libxcb1-dev \
            libvulkan-dev \
            libssl-dev \
            libsqlite3-dev \
            mesa-vulkan-drivers \
            vulkan-tools
      - uses: dtolnay/rust-toolchain@nightly
        with:
          components: miri
      - name: Miri test
        run: cargo +nightly miri test -p flui-core cell
  miri-cell-tree-borrows:
    name: Miri (cell module — Tree Borrows, non-blocking)
    runs-on: ubuntu-latest
    continue-on-error: true
    steps:
      - uses: actions/checkout@v4
      - name: Install Linux dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libwayland-dev \
            libxkbcommon-dev \
            libxkbcommon-x11-dev \
            libfontconfig-dev \
            libegl-dev \
            libx11-dev \
            libx11-xcb-dev \
            libxcb-shape0-dev \
            libxcb-xfixes0-dev \
            libxcb1-dev \
            libvulkan-dev \
            libssl-dev \
            libsqlite3-dev \
            mesa-vulkan-drivers \
            vulkan-tools
      - uses: dtolnay/rust-toolchain@nightly
        with:
          components: miri
      - name: Miri Tree Borrows
        env:
          MIRIFLAGS: '-Zmiri-tree-borrows'
        run: cargo +nightly miri test -p flui-core cell
  ```
  Stacked Borrows is PR-blocking (Q6); Tree Borrows is non-blocking research signal. Document Tree Borrows divergence in `.github/workflows/ci.yml` comments. If local CI proves the full default feature set is too heavyweight for Miri, record the verified narrower command in the spec before changing the workflow.
  - **Plan note (2026-05-10):** added `miri-cell` and `miri-cell-tree-borrows` jobs to `.github/workflows/ci.yml`. Both install the same Linux package set as check/test, use `dtolnay/rust-toolchain@nightly` with `miri`, and run scoped `cargo +nightly miri test -p flui-core cell`. The Tree Borrows job is `continue-on-error: true`. Both Miri steps override the workflow-global `RUSTFLAGS=-Dwarnings` with an empty value so unrelated nightly future-compat warnings (observed locally in `taffy.rs`) do not mask UB results for the cell module.

- [x] **Task 26b. (NEW — rev 6, quality review; rev 7 dependency correction)** Add an AppCell acquire/release microbenchmark using the repo's existing bench-example convention, not Criterion. `criterion` is not currently in `Cargo.lock`, while K07's non-goals preserve "Does NOT modify Cargo.lock"; adding Criterion belongs in a follow-up benchmarking hygiene PR unless the non-goal is explicitly changed.
  - Preferred file: `crates/flui-core/examples/bench/app_cell.rs`.
  - Add a matching `[[example]]` entry in `crates/flui-core/Cargo.toml`:
    ```toml
    [[example]]
    name = "app_cell_bench"
    path = "examples/bench/app_cell.rs"
    ```
  - Benchmark operation: acquire `AppCell::borrow_mut()`, pass the guard through `std::hint::black_box`, then drop it in a tight loop using `std::time::Instant`.
  - Capture result in `.k07-bench-results.txt` (gitignored). If the result suggests the flag check is not sub-microsecond at realistic iteration counts, escalate before landing K07.
  - Rejected rev 6 sketch (kept here only to avoid accidental reintroduction):
  ```rust
  fn bench_borrow_mut_acquire_release(c: &mut Criterion) {
      let app = make_test_app();
      c.bench_function("AppCell::borrow_mut + drop", |b| {
          b.iter(|| {
              let _g = app.borrow_mut();
              black_box(&_g);
          });
      });
  }
  ```
  Backs the spec's Known Limitation L1 ("flag check is sub-microsecond, not a hot-path concern") with empirical measurement. K07 gates K05 → K01 → SF04/SF05; if flag check turns out to be 50ns × 10k frame events = 500μs hot-path overhead, escalate.
  - **Plan note (2026-05-10):** added `crates/flui-core/examples/bench/app_cell.rs` plus a `[[example]]` entry named `app_cell_bench` with `required-features = ["test-support"]`, because the bench needs a real `Rc<AppCell>` without widening production API. Added `.k07-bench-results.txt` to `.gitignore`. Verified `cargo build -p flui-core --example app_cell_bench --features test-support`, `cargo build --workspace --examples`, and `cargo run -p flui-core --release --features test-support --example app_cell_bench`. Release result: `ns/op=5`, `budget_ns=1000`, `verdict=pass`.

- [x] **Task 28.** Behavioral tests for the AsyncApp redesign (Task 15):
  - `async_app_update_after_app_drop_returns_app_gone_away` (was: silent `unwrap` panic).
  - `async_app_as_mut_after_drop_returns_structured_error` (or panics with structured Display, per Task 5 decision).
  - `async_app_borrow_mut_propagates_reentry_error` — emit re-entry from a spawned task; assert structured error, NOT `BorrowMutError`.
  - **Plan note (2026-05-10):** added four async behavior tests in `crates/flui-core/src/reentrancy.rs`: `async_app_open_window_after_app_drop_returns_app_gone_away`, `async_app_update_after_app_drop_panics_app_gone_away`, `async_app_as_mut_panics_structured_error`, and `async_app_borrow_mut_propagates_reentry_error`. The borrow-propagation test uses a deterministic `cx.to_async().open_window(...)` call while outer `App::update` holds the mutable guard; an attempted foreground-task/blocking shape violates the scheduler's no-parking rule and is not the contract under test. Verified `cargo test -p flui-core async_app --tests` passes (4 tests) and `cargo test -p flui-core reentr --tests` passes (19 tests).

- [x] **Task 29.** Update existing K15 tests in `crates/flui-core/src/reentrancy.rs` if any of them reference `std::cell::BorrowMutError` (Task 9 deletion). Use `grep -n 'BorrowMutError' crates/flui-core/src/reentrancy.rs`.
  - **Plan note (2026-05-10):** no further source changes needed; `rg -n "BorrowMutError" crates/flui-core/src/reentrancy.rs` returns zero hits.

### Phase 7 — Documentation & spec close-out

- [x] **Task 30.** Update `.ai-factory/RESEARCH.md` Active Summary with one-paragraph K07 entry. Mention: AppCell replaced by `flui_core::app::cell::AppCell` (Candidate B — hand-rolled `UnsafeCell<App>` + `BorrowState` returning `ReentryError`); 103 narrow AppCell-derived callsites migrated; K15 Known Limitation #1 (10+ AsyncApp sites), #2 (`as_mut` panic), and #6 (panic-leak fields) discharged; AppCell `#[doc(hidden)]` retained.
  - **Plan note (2026-05-10):** updated `.ai-factory/RESEARCH.md` date, added a top-level K07 research entry, and inserted a K07 status paragraph inside the Active Summary block. The entry records Candidate B, 103 callsites, K15 limitation discharge (#1/#2/#6), retained doc-hidden AppCell surface, Miri/bench validation, and K05 as next critical-chain item.

- [x] **Task 31.** Mark K07 done in `.ai-factory/ROADMAP.md` — checkbox flip at line 58; completion-date row in `## Completed` table.
  - **Plan note (2026-05-10):** flipped K07 to checked, updated the Phase 0-K overview to remove AppCell/re-entrancy from active debt, and added a `## Completed` table row dated 2026-05-10.

- [x] **Task 32.** Run `/aif-docs` to absorb rustdoc / README drift. Confirm `cargo doc --workspace --no-deps` zero new warnings vs Task 1 baseline. Specifically:
  - The `_ownership_and_data_flow.rs` doctest references AppCell — verify it still compiles or is gated. (Note: K98 is the dedicated rewrite spec; for K07 we just keep it green.)
  - `flui-core::app` module docs reflect new primitive.
  - **Plan note (2026-05-10):** used the `aif-docs` workflow as a targeted docs checkpoint rather than restructuring README/docs. Updated `reentrancy.rs` and `app/async_context.rs` rustdoc to reflect K07's completed AsyncApp/AppCell behavior, and converted K07/K15-local intra-doc links in `reentrancy.rs` to code spans so this change does not add rustdoc link warnings. Verified `cargo doc --workspace --no-deps` exits 0. Remaining rustdoc warnings are existing non-K07 documentation debt in `flui-macros`, animation/gesture docs, platform keystroke docs, and `animation_demo`.

- [x] **Task 32a.** Add `CHANGELOG.md` entry for K07 under `## [Unreleased]` section, following the S21 entry style ([CHANGELOG.md](CHANGELOG.md)). Required content:
  - Section heading: `## [Unreleased] — K07 AppCell removal (token-based borrow model)`.
  - One paragraph summarizing: AppCell replaced by `flui_core::app::cell::AppCell` (Candidate B/A/C — fill in choice); breaking changes (AsyncApp surface, `BorrowMutError` removal, K15 `From<BorrowMutError>` deletion); link to plan + design spec + migration guide (Task 32b).
  - "Migration guide:" line referencing Task 32b path.
  - **Decision (Q11):** K99 / K15 backfill — recommended NO (separate hygiene PR). Document this decision in the entry footer.
  - File: [CHANGELOG.md](CHANGELOG.md). Place above the S21 entry.
  - **Plan note (2026-05-10):** added a K07 `[Unreleased]` entry above S21 with Candidate B summary, breaking `BorrowMutError`/`try_borrow_mut` changes, typed panic guidance, validation coverage, plan/spec/migration-guide links, and Q11 no-backfill decision.

- [x] **Task 32b.** Author migration guide at `docs/superpowers/migrations/K07-appcell-removal.md`. Pattern: same as `docs/superpowers/migrations/animation-flutter-parity.md` (referenced from CHANGELOG S21 entry). **Note:** the `migrations/` subdirectory may not exist yet — create it (`mkdir -p docs/superpowers/migrations`). Required sections:
  - Title, summary (1 paragraph).
  - "Before / After" code samples for each user-facing breaking change:
    1. `AsyncApp::*` borrow_mut migration (Q2 + Q8 decisions reflected).
    2. `Window::prompt` callers (already migrated by K15 — confirm post-K07 shape unchanged).
    3. `AsyncWindowContext::prompt` callers (already migrated by K15 — confirm post-K07).
    4. `AppContext::as_mut` users (Q8 decision — if widened, show new signature).
    5. Any direct `AppCell::borrow_mut` callers in user crates (rare — `flui-navigator`, `flui-widgets` skeleton — audit).
    6. `std::cell::BorrowMutError` pattern-matchers (downstream code) — convert to `ReentryError::AppBorrowed`.
  - "What stayed the same" section (mostly the high-level `Application::run`, `App::*` methods, `cx.defer`, `cx.update_window`, etc.).
  - Link back to the design spec.
  - **Plan note (2026-05-10):** created `docs/superpowers/migrations/K07-appcell-removal.md` with summary, design-spec link, before/after samples for AsyncApp borrow errors, `Window::prompt`, `AsyncWindowContext::prompt`, `AppContext::as_mut`, direct AppCell callers, and `BorrowMutError` pattern matchers, plus a "What stayed the same" section.

- [x] **Task 32c.** Update `AGENTS.md` to reflect K07 closure of E3:
  - [AGENTS.md:15](AGENTS.md#L15) currently lists `AppCell` among 24+ debt items: "broken Provider, …, AppCell, action globals, undefined re-entrancy contract, …". Remove `AppCell` from this list (and update the count if it's a literal "24+").
  - Add K07 to "Done" or status list per the file's convention (read AGENTS.md to determine the exact section).
  - Verify no other `AppCell` mention in `AGENTS.md` besides line 15.
  - File: [AGENTS.md](AGENTS.md).
  - **Plan note (2026-05-10):** updated current status to remove K07-closed debt and name K05 as next critical-chain item, added K99/K15/K07 to the done list, added `docs/superpowers/audits/` and `docs/superpowers/migrations/` to the project tree, and added K07 spec/migration-guide rows to Documentation. Verified `rg -n "AppCell" AGENTS.md` returns zero hits.

- [x] **Task 33.** Update CLAUDE.md if any new rule emerges from K07 (e.g., "do not pattern-match on `std::cell::BorrowMutError` from flui-core APIs — use `ReentryError::AppBorrowed`").
  - **Plan note (2026-05-10):** no root `CLAUDE.md` exists in this repository (`Test-Path CLAUDE.md` returned `False`). `.claude/` is present as the installed agents/skills directory, not a project-level rule file. No update made; the user-facing rule is covered by the K07 migration guide and changelog.

### Phase 8 — Validation gates

- [x] **Task 34.** `cargo build --workspace --all-features` green.
  - **Plan note (2026-05-10):** passed in 20.36s on Windows (`x86_64-pc-windows-msvc`).
- [x] **Task 35.** `cargo test --workspace` green. Pre-existing test count increases by ≥ N (where N is the sum of new tests in Tasks 26-29; expected ≥ 7).
  - **Plan note (2026-05-10):** passed. `flui-core` unit-test count moved from Task 1 baseline 345 total (344 passed + 1 ignored) to 356 total after the `/aif-review` `AsyncApp::read_window` regression test, satisfying the expected >= 7 increase.
- [x] **Task 36.** `cargo clippy --workspace --all-targets -- -D warnings` zero new warnings vs Task 1 baseline.
  - **Plan note (2026-05-10):** first run caught a K07-local `redundant_clone` in `examples/bench/app_cell.rs`; fixed by borrowing `cx.app` directly. Final run passed cleanly.
- [x] **Task 37.** `cargo fmt --all -- --check` clean.
  - **Plan note (2026-05-10):** passed after final doc-link cleanup.
- [x] **Task 38.** `cargo doc --workspace --no-deps` zero new warnings vs Task 1 baseline.
  - **Plan note (2026-05-10):** initial rebuild surfaced stale rustdoc link warnings in `flui-macros`, animation docs, gesture docs, `platform/keystroke.rs`, and `window.rs`. Fixed those by converting private/unresolved rustdoc links to code spans or plain URLs. Final `cargo doc --workspace --no-deps` passed with zero warnings.
- [x] **Task 39.** `cargo +nightly miri test -p flui-core cell` (Task 27) — ALL green. If miri not installed, document via `rustup component add miri`.
  - **Plan note (2026-05-10):** passed: 5 tests green (`app::cell::*` plus direct reentrancy replacement). Nightly emitted pre-existing `taffy.rs` future-incompat float-literal warnings during compilation; no Miri failures.
- [x] **Task 40.** Manual smoke: run `cargo run --example nav_demo` ~30 seconds with `RUST_LOG=flui_core::app::cell=trace`. Verify zero `warn!` events under normal navigation.
  - **Plan note (2026-05-10):** ran `target/debug/nav_demo.exe` for a full 30-second smoke window with `RUST_LOG=flui_core::app::cell=trace`; process was stopped after the window and produced no `warn` matches.
- [x] **Task 41.** Web platform smoke (if reachable from test infra): build with `--target wasm32-unknown-unknown` if a recipe exists; otherwise document gap and file follow-up.
  - **Plan note (2026-05-10):** no documented wasm smoke/build recipe was found in `README.md`, `docs/`, `.github/`, `examples/`, or the relevant Cargo manifests; local installed targets only included `x86_64-pc-windows-msvc`. Filed follow-up plan `.ai-factory/plans/followup-K07-web-platform-smoke.md`.

### Phase 9 — Adversarial re-review (post-implementation)

- [ ] **Task 42.** Dispatch **`flui-arch-reviewer`** subagent on the K07 implementation diff. Prompt: "Re-review the K07 implementation against the revision-N design spec. Key invariants: (a) `AppCell` `unsafe` blocks have SAFETY comments and pass Miri; (b) the K15 contract still holds — every `ReentryError::*` variant produces the same Display under K07; (c) `AsyncApp` 10+ sites are now structured; (d) `AsyncApp::as_mut` panic is gone OR Display-structured; (e) the 5 Known Limitations are present in the spec, not silent." → Depends on Task 5 (spec) + Task 32 (docs sweep).

- [ ] **Task 43.** Dispatch **`migration-risk-adversary`** subagent on the K07 diff. Prompt: "Re-review K07 implementation. Validate adversarial-review findings from Task 6 are addressed. Find new regressions: subscription handlers, observer callbacks, release-listener drops, async-spawn paths, web event-loop integration, drop-time runs (entity Drop, App Drop, Window Drop). Specifically: did any callback that previously had silent error-swallowing become silent in a NEW way under K07?"

- [ ] **Task 44.** Dispatch **`rust-api-migration-auditor`** subagent on the K07 diff. Prompt: "Re-review K07 final API. Verify: semver impact matches design-spec promises (no surprise breakage); auto-trait set on `Application` / `App` / `AsyncApp` unchanged unless documented; feature flag matrix (`test-support`, `inspector`, `leak-detection`) all green; trait object safety preserved; MSRV 1.95 idioms used appropriately."

- [ ] **Task 45.** Triage findings from Tasks 42-44. (a) accept and patch, (b) split to follow-up K-spec, or (c) reject with documented reason in design spec.

## Task Dependencies

```
   1. Baseline → 2. Audit cite drift
        │           │
        │           ▼
        ├──► 3. Spike (parallel) ║ 4. rust-api-migration-auditor (parallel)
        │              │
        │              ▼
        ├──► 5. Author design spec (Candidate decision)
        │              │
        │              ▼
        ├──► 6. flui-arch-reviewer ║ migration-risk-adversary ║ rust-api-migration-auditor (parallel)
        │              │
        │              ▼
        ├──► 7. Triage / spec rev-2
        │              │
        │              ▼
        ├──► 7a. shared-borrow audit ║ 7b. spec drift scrub ║ 8a. static_assertions dev-dep
        │              │
        │              ▼
        ├──► 8. AppCell new module + RAII
        │              │
        │              ▼
        ├──► 9. ReentryError surface fix-ups
        │              │
        │              ▼
        ├──► 10. prelude touch-up (if needed)
        │              │
        │              ▼
        ├──► 11. App::new_app
        │              │
        │              ▼
        ├──► 12. Application
        │              │
        │              ▼
        ├──► 13. App::this consumers (5 sites)
        │              │
        │              ▼
        ├──► 14. pending_effects audit
        │              │
        │              ▼
        ├──► 14a. panic-leak field guards ║ 14b. hot-path reachability audit
        │              │
        │              ▼
        ├─►  Migrations (parallel where files don't overlap):
        │       15. AsyncApp ║ 16. TestAppContext ║ 17. TestApp ║ 18. HeadlessAppContext ║ 19. VisualTestContext
        │              │
        │              ▼
        ├─►  More migrations (parallel):
        │       20. elements ║ 21. subscription ║ 22. executor ║ 23. platform (split per file) ║ 24. examples
        │              │
        │              ▼
        ├──► 25. Final scan (no AppCell hits remaining)
        │              │
        │              ▼
        ├──► Tests:  26. proptest + K15-port ║ 26b. app_cell bench example ║ 27. Miri ║ 27a. Miri CI ║ 28. AsyncApp behavioral ║ 29. K15 fixup
        │              │
        │              ▼
        ├──► Validation: 34 ║ 35 ║ 36 ║ 37 ║ 38 ║ 39 ║ 40 ║ 41 (parallel)
        │              │
        │              ▼
        ├──► Docs: 30. RESEARCH ║ 31. ROADMAP ║ 32. /aif-docs ║ 33. CLAUDE.md
        │              │
        │              ▼
        ├──► 42. flui-arch-reviewer (re-review) ║ 43. migration-risk-adversary (re-review) ║ 44. rust-api-migration-auditor (re-review)
        │              │
        │              ▼
        └──► 45. Final adversary triage
                       │
                       ▼
                  (final commit + PR)

   Critical path: 1 → 2 → 5 → 7 → 7b → 8 → 11 → 12 → 13 → 14a → 15 → 25 → 26 → 34 → 42 → 45
   Total: ~16 sequential nodes; ~7-9 parallel slots in the audit/migration/test phases.
```

## Commit Plan

K07 has **55 checkbox tasks**. Per skill convention, commit checkpoints every 3-5 tasks. Each commit MUST be green at HEAD — `cargo build` + `cargo test` MUST pass. Several commits are documentation-only and easy to land standalone.

| # | Tasks | Conventional commit message |
|---|---|---|
| 1 | 1, 2 | `chore(k07): pre-flight baseline + cite-drift audit` |
| 2 | 3, 4 | `docs(k07): candidate spike notes + auditor input` (notes-only — may be dropped if spike artifacts kept untracked) |
| 3 | 5 | `docs(spec): K07 design spec — primitive choice, migration plan, Decision log rev-1` |
| 4 | 6, 7 | `docs(spec)!: K07 design spec rev-2 — adversarial-review absorbed` |
| 5 | 7a, 7b, 8a | `docs+chore(k07): pre-implementation audit cleanup` (shared-borrow decision, spec scrub, static_assertions dev-dep) |
| 6 | 8, 9, 10 | `feat(app)!: introduce flui_core::app::cell::AppCell + remove BorrowMutError From` (BREAKING — `From<BorrowMutError>` impl removed; new public-but-doc-hidden `AppCell` module) |
| 7 | 11, 12, 13, 14, 14a, 14b | `refactor(app): migrate App + Application + Weak<AppCell>; add panic guards and hot-path audit` |
| 8 | 15 | `refactor(async)!: AsyncApp surface — structure 10+ unstructured borrow_mut sites; resolve as_mut panic` (BREAKING — panic payload / return surface changes per Task 5 decision) |
| 9 | 16, 17, 18, 19 | `refactor(test): migrate Test/Visual/Headless contexts to new cell` |
| 10 | 20, 21, 22 | `refactor(elements): migrate elements + subscription + executor to new cell` |
| 11 | 23 | `refactor(platform)!: migrate flui-core::platform/* to new cell; update K15 deferral comments` |
| 12 | 24, 25 | `refactor(examples): migrate legacy example + final scan` |
| 13 | 26, 26b, 27, 27a, 28, 29 | `test(app::cell): proptest + Miri/CI + bench example + AsyncApp behavioral + K15-port` |
| 14 | 30, 31 | `docs(research+roadmap): close K07; cross-references updated` |
| 15 | 32, 32a, 32b, 32c, 33 | `docs(rustdoc+changelog+migration+agents+claude): K07 sweep` |
| 16 | 34-41 | `chore(k07): validation pass — build/test/clippy/fmt/doc/miri/smoke/web` (likely empty — fold into commit 14/15 if no fixups) |
| 17 | 42, 43, 44, 45 | `docs(k07): adversary re-review triage` |

If commits 2 or 15 are empty, drop them. **Rollback note:** Commits 5-onwards have forward type-dependencies; rollback of K07 = revert all-as-unit (migration-risk finding pre-recorded).

## Done criteria

K07 is done when:

1. ✅ `crates/flui-core/src/app/cell.rs` (or equivalent path per Task 5) module exists with the chosen primitive; `unsafe` blocks all carry SAFETY comments; module-level rustdoc IS the new contract document.
2. ✅ `AppCell`, `AppRef`, `AppRefMut` keep `#[doc(hidden)]` (matches pre-K07 shape).
3. ✅ `Application(Rc<AppCell>)` shape preserved at the public surface; `App::this: Weak<AppCell>` shape preserved.
4. ✅ `AsyncApp`, `TestAppContext`, `TestApp`, `HeadlessAppContext`, `VisualTestContext` all migrated to the new primitive with their `Rc<AppCell>` / `Weak<AppCell>` field types unchanged in spelling.
5. ✅ All **103 narrow AppCell-derived** `borrow_mut()` / `try_borrow_mut()` / `borrow()` callsites from `.k07-recon.txt` migrated onto the new Candidate-B `app::cell::AppCell`; final grep of `crates/flui-core/src/` returns zero legacy `RefCell<App>` / `BorrowMutError` / `TRACK_THREAD_BORROWS` hits. Remaining AppCell borrow hits are expected compatibility API calls, not legacy `RefCell<App>` uses.
6. ✅ The 10+ unstructured `app.borrow_mut()` sites in `app/async_context.rs` (lines 39, 45, 55, 65, 126, 135, 152, 168, 182) are now structured per Task 5 decision (K15 Known Limitation #1 discharged).
7. ✅ `AsyncApp::as_mut` panic at `app/async_context.rs:73` is replaced per Task 5 decision (K15 Known Limitation #2 discharged).
8. ✅ The K15 deferred decision on `ReentryMode::PanicLikeUpstream` is RESOLVED in the spec (re-introduced, dropped, or documented obsolete — pick one).
9. ✅ The 3 K15 platform deferral comments (`mac/platform.rs:500-502`, `mac/platform.rs:1254`, `windows/platform.rs:452-453`) are updated to reference K07.
10. ✅ `impl From<std::cell::BorrowMutError> for ReentryError` is REMOVED; `ReentryError::AppBorrowed` is now produced directly by the new `AppCell::try_borrow_mut`.
11. ✅ The surviving K15 reentrancy tests pass on the new primitive, and the deleted `borrow_mut_error_converts_to_app_borrowed` test is replaced by a direct `AppCell::try_borrow_mut -> ReentryError::AppBorrowed` test (contract preserved without the removed `From<BorrowMutError>` impl).
12. ✅ New tests added: ≥ 4 proptest scenarios in Task 26, ≥ 1 direct replacement AppCell contention test, ≥ 1 Miri pass in Task 27, ≥ 3 AsyncApp behavioral tests in Task 28. Sum ≥ 8.
13. ✅ `cargo build --workspace --all-features` green.
14. ✅ `cargo test --workspace` green; test count increases by ≥ 7.
15. ✅ `cargo clippy --workspace --all-targets -- -D warnings` zero new warnings vs Task 1 baseline.
16. ✅ `cargo fmt --all -- --check` clean.
17. ✅ `cargo doc --workspace --no-deps` zero new warnings.
18. ✅ `cargo +nightly miri test -p flui-core cell` green (or documented gap if miri not installed).
19. ✅ Design spec at `docs/superpowers/specs/2026-05-09-K07-appcell-removal-design.md` exists; "Decision log" section documents (a) candidate choice rationale, (b) any rev-2 narrowings from Task 6 adversarial review, (c) explicit Q8/Q9/Q10 decisions (`AppContext::as_mut` widening, Drop-order preservation, `Application: Clone` status).
20. ✅ Spec "Known Limitations" enumerates ≥ 5 documented scope decisions.
21. ✅ Spec "Open questions" section is empty (no deferrals to implementation — all 12 open questions resolved at spec-merge time).
21a. ✅ Post-migration `grep -rn 'BorrowMutError' crates/flui-core/src/` returns zero hits (Task 9 deletion verified).
21b. ✅ Post-migration `grep -rn 'TRACK_THREAD_BORROWS' crates/flui-core/src/` returns zero hits (Known Limitation #3 — replaced by `log::trace!`).
21c. ✅ `Application` Drop-order preserved per Q9 spec decision; `// Drop globals last` invariant at `app.rs:622-627` honored.
21d. ✅ `CHANGELOG.md` has `## [Unreleased] — K07 AppCell removal` entry above the S21 entry; one-paragraph summary + migration-guide link present.
21e. ✅ Migration guide at `docs/superpowers/migrations/K07-appcell-removal.md` exists with Before/After samples for each user-facing breaking change.
21f. ✅ `AGENTS.md` line 15 no longer lists `AppCell` among debt items; status updated per Q11 decision.
22. ✅ **(R2/R3/R4 — rev 4)** `crates/flui-core/src/app/cell.rs` `#[cfg(test)]` module asserts `AppCell: !Send + !Sync`, `AppRef<'static>: !Send + !Sync`, `AppRefMut<'static>: !Send + !Sync` via `static_assertions::assert_not_impl_any!`. `UnwindSafe` behavior matches pre-K07 baseline. `#[track_caller]` propagation verified on every `borrow*` method.
23. ✅ **(R5 — rev 4 / K15 Limitation #6)** `App::update_entity`, `App::update_window_id`, and `App::open_window` panic-leak paths fixed with raw-pointer field-projection guards. Tests verify `currently_updating_entity` and `window_update_stack` are restored after a caught panic, and `pending_updates` returns to zero after panic-through-`update_entity`.
24. ✅ **(R1 — rev 4)** `cargo +nightly miri test -p flui-core cell` green for the cell module's tests (PR-blocking gate per Q6 resolution).
25. ✅ **(rev 7)** Design spec / plan drift scrubbed: no actionable-current references remain to manual `UnwindSafe` impls, production `catch_unwind` Task 14a implementation, legacy AppCell callsite estimates, legacy `this.upgrade()` estimates, or unchanged-K15-test language.
26. ✅ `.ai-factory/RESEARCH.md` Active Summary has K07 entry.
27. ✅ ROADMAP K07 entry checked off; completion-date row added.
28. ✅ `/aif-docs` checkpoint completed.
29. ✅ All three pre-implementation adversarial reviews (Task 6) are absorbed into the spec.
30. ✅ All three post-implementation adversarial re-reviews (Tasks 42-44) completed; findings either patched, split into follow-up K-spec, or rejected with documented reason.
31. ✅ Manual smoke (~30s, `RUST_LOG=flui_core::app::cell=trace`) produces zero unexpected `warn!` events under normal navigation.
32. ✅ Web platform smoke (Task 41) passes OR documented gap with follow-up issue filed.

## Resolved Decision Checklist (formerly open questions)

These were the decision points Task 5 had to answer in the design spec. They are retained here as a checklist, not as open implementation questions. The design spec's `## Open questions` section must remain `EMPTY`.

- **Q1.** Candidate A vs B vs C — pick one. Document rationale referencing Tasks 3 + 4 outputs.
- **Q2.** `AsyncApp::as_mut` policy: drop, panic-with-structured-Display, or `Result`-ify? (cross-link Q8)
- **Q3.** `ReentryMode::PanicLikeUpstream` policy: re-introduce, drop, or document obsolete?
- **Q4.** `Weak::upgrade()` failure handling — silent no-op (current), `Result<…, ReentryError::AppGoneAway>`, or panic with structured Display?
- **Q5.** `option_env!("TRACK_THREAD_BORROWS")` debug-instrumentation replacement — `log::trace!` (project style) or new `track-borrows` Cargo feature?
- **Q6.** Miri CI integration: K07 PR-blocking, scheduled job, or one-shot verification documented in spec? **RESOLVED (rev 4):** **PR-blocking for `crates/flui-core/src/app/cell.rs` only** (scoped Miri test pass on the cell module's tests, not workspace-wide). Rationale: Stacked-Borrows / Tree-Borrows soundness for the new `unsafe` cannot be audited any other way; cost is bounded because the cell module is small (~200 LoC, ~6-10 tests). Workspace-wide Miri is deferred to a future R-track spec (cost is high on Windows). One-shot Miri at K07-merge time is insufficient — every subsequent `app/cell.rs` edit must pass Miri or it lands UB.
- **Q7.** Whether to split K07 into K07a (cell + App) and K07b (everything else) for reviewability — single-PR-too-large risk vs review-overhead trade-off.
- **Q8.** `AppContext::as_mut` trait return: keep `GpuiBorrow<'a, T>` (preserves trait shape, AsyncApp keeps panic) or widen to `Result<GpuiBorrow<'a, T>, ReentryError>` (BREAKING all 5 implementors)? Cross-link Q2.
- **Q9.** `Application` Drop-order preservation under new cell: spec MUST verify the new cell preserves the `// Drop globals last` invariant at [app.rs:622-627](crates/flui-core/src/app.rs#L622). Cross-link with K12 (drop-order codification).
- **Q10.** `Application: Clone` — leave absent (current state, K07 keeps as-is) or add to surface? Recommended: leave absent — adding Clone widens the public API beyond K07's mandate.
- **Q11.** CHANGELOG.md backfill: add K99 / K15 entries retroactively alongside K07, OR ship K07 entry only (K99/K15 backfilled in a separate hygiene PR)? Recommended: K07-only — backfilling unrelated specs in K07's PR widens the diff unnecessarily.
- **Q12.** PR title convention: `K15 — Re-entrancy contract (Phase 0-K, second spec)` style (no conventional prefix, used by `#9`) or `chore(k07)!: AppCell removal — token-based borrow model` (conventional, used by K99 `#8`)? Recommended: K07 follows K15 style ("K07 — AppCell removal (Phase 0-K, third spec)") because the change is architectural, not chore-level.

## Risk assessment (rev 4 — post-spike)

> Risks below are TIERED by severity. Each carries a Spike-finding ID (R1-R12 from rev 4 risk audit) and a specific Task-level mitigation. Tier 1 risks block K07 PR until resolved; Tier 2-3 risks are absorbed via documentation.

### Tier 1 — Soundness / `unsafe` (PR-blocking)

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| **R1** | Stacked Borrows / Tree Borrows under Miri — hand-rolled cell may derive `&mut App` through aliased path | Medium | High (UB in production) | Task 8 SAFETY discipline + RustBelt `RefCell` reference; Task 27 PR-blocking Miri scoped to `app/cell.rs` (Q6 resolved); `migration-risk-adversary` re-review |
| **R2** | `!Send + !Sync` not propagated → auto-trait regression | Low | High (semver + safety) | Task 8 `static_assertions::assert_not_impl_any!` compile-time tests |
| **R3** | `UnwindSafe` regression vs pre-K07 `RefCell<App>` | Low | Medium | Task 8 compile-time test; Task 1 baseline captures pre-K07 behavior |
| **R4** | `#[track_caller]` propagation missing → debug panic source-loc lost | Medium | Low (DX only) | Task 8 explicit checklist + code review |
| **R5** | K15 Limitation #6 panic-leak on `currently_updating_entity` / `window_update_stack` not fixed | High (without Task 14a) | High (open obligation from K15) | **Task 14a (NEW)** — RAII guards via inline scope or `scopeguard`; `prop_currently_updating_entity_restored_after_panic` test |
| **R6** | `BorrowState { Free, Mut, Shared(NonZeroU32) }` state transitions buggy | Medium | High (UB) | Task 26 proptest covers all transitions incl. saturation |

### Tier 2 — Process risks (review-gating)

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R7 | Miri CI not blocking → bad `unsafe` slips main | Low (Q6 now PR-blocking) | High | Q6 resolved → PR-blocking scoped Miri |
| R8 | `cargo-semver-checks` (R2 in roadmap) flags `pub struct AppCell` internal change | Low (R2 deferred) | Low | Document in spec; R2 spec lands post-K07 |
| Public API blast radius wider than spec promises | Low-Medium | High | Task 4 (`rust-api-migration-auditor` pre-implementation) + Task 44 (re-review); design spec revision-2 is the gate |
| AsyncApp redesign breaks existing async user code | Medium | Medium-High | Task 15 explicit migration table; Task 28 behavioral tests; Task 43 (`migration-risk-adversary`) re-review |
| K15 reentrancy tests fail under new primitive | Low | High (contract regression) | Task 26 ports forward the surviving K15 tests and adds a direct AppCell contention replacement for the deleted `BorrowMutError` conversion test; Task 14 + 14a sanity-check; if any fail, escalate |
| Web platform regression silent | Medium | Medium | Task 41 explicit smoke; Task 43 covers; if fails, follow-up K-spec |
| Drop-order dependence | Low-Medium | High | Q9 spec decision; Task 43 verifies; K12 codifies separately |

### Tier 3 — Future-coupling (documented, not blocker)

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R9 | K05 (Element ctx-object) needs partial borrows; monolithic AppCell complicates | Medium | Medium | Spec "Future considerations" §R9; K05's plan inherits this |
| R10 | Phase III multi-threaded UI wants `App: Send` | Low (no Phase III roadmap yet) | High (full redesign) | Spec "Future considerations" §R10 |
| R11 | "Looks like RefCell, isn't" — devs `.unwrap()` instead of `cx.defer` | Medium | Low (DX) | Module rustdoc + migration guide examples; K15 contract docs |
| R12 | Drop-on-panic leaves `App` partially mutated | Medium (under panic recovery only) | Medium | Module rustdoc warning (preserves pre-K07 behavior) |

### Tier 4 — Honest limitations (not bugs)

| ID | Limitation | Note |
|---|---|---|
| L1 | Runtime check not eliminated (still `O(1)` flag check per borrow) | ROADMAP Key Principle #8 ("no `Rc<RefCell<…>>` on hot paths") interpreted permissively — flag check is sub-microsecond, not a hot-path concern |
| L2 | AsyncApp surface redesign deferred (Candidate A's elegance left on table) | K15 Limitation #1 partially resolved via `try_borrow_mut() -> Result<_, ReentryError>`; full async redesign is its own multi-week spec |
| L3 | 5 K07 `unsafe` blocks added (2 in `app/cell.rs`, 3 in `app.rs`) | Project has 801 unsafe (FFI-heavy); marginal +0.6%. Cell projections are Miri-verified; `app.rs` raw-pointer field guards are SAFETY-commented and covered by panic-restoration tests |

### Inherited from rev 1 (still valid)

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| 103 callsite migration causes merge-conflict cascade with parallel K-track | Medium | Medium | Phase 0-K critical chain is sequential by design; K07 lands before K05/K01 start. Independent K-track items (K12-K17, K20-K22, K90-K98) are merge-safe |
| K07 invalidates K05's planning assumptions | Low (sequential) | Low | K07 → K05 dependency is design intent |
| Test count inflation hides K15 regressions | Low | Low | Task 35 verifies test-count delta; failed K15 tests surface by name |
| `cargo doc` warnings from new rustdoc | Low | Low | Task 8 + 38 verify |

## Refinement record

### Revision 1 (2026-05-09 initial draft)
- Initial draft task count: 45, across 9 phases.
- Primitive choice deliberately deferred to Phase 1 design-spec authoring (Task 5) instead of pre-decided. Three candidates documented with trade-offs.
- 4 K15-deferred items explicitly tracked through to discharge in Done criteria.
- 7 Open Questions enumerated as MUST-RESOLVE-IN-SPEC, not deferrable to implementation.
- Pre-implementation adversarial review (Task 6) treats this plan as the input; Task 7 produces "rev-2" of the spec only — the plan itself may be revised once.
- Post-implementation adversarial review (Tasks 42-44) mirrors the K15 rhythm but adds `rust-api-migration-auditor` because public-surface blast radius is wider than K15.
- Validation gates include explicit Miri (Task 39) — first K-track spec to require this. K15 had `unsafe`-zero shape; K07's `unsafe` (likely 1-3 blocks under Candidate B) requires Miri.

### Revision 2 (2026-05-09 aif-improve)
Codebase-grounded fact pass against revision 1; 10 findings absorbed:
- **Recount precision (FINDING E):** `~248 callsites` claim was an upper-bound conflation of AppCell + non-AppCell `RefCell` patterns. Refined to `100-220 AppCell-derived callsites`. Task 2 now produces the exact count via 4 documented one-liners (Steps 2.1-2.4) and writes them to `.k07-recon.txt`.
- **Task 9 expansion (FINDING F):** original task only deleted the `From<BorrowMutError>` impl. Now also deletes the `use std::cell::BorrowMutError;` import at `reentrancy.rs:76` AND updates rustdoc lines 45/71/154 with new wording. Verification step: `grep BorrowMutError` MUST return zero hits.
- **Tasks 15-19 explicit parallelism (FINDING O):** added `[PARALLEL with N-M]` annotation; task descriptions confirm the file-disjointness invariant.
- **Task 22 clarification (FINDING A):** `executor.rs` is a TEST fixture only (`#[cfg(test)] fn create_test_app`). The 1 `borrow()` call at line 573 and the `Rc<crate::AppCell>` return type are the affected surface. Production code has zero AppCell access in `executor.rs`. Note: line 581's `*task_ran.borrow_mut()` is `RefCell<bool>` (NOT AppCell) — leave untouched.
- **Task 23 exact comment text (FINDING Q):** the THREE platform deferral comments now have spec-frozen replacement text (per-comment) rather than a hand-wave "or whatever the spec decides". Spec verifies replacement, not append.
- **Task 24 widened scope (FINDING R):** `examples/` scan now uses `Glob examples/**/*.rs` + cross-cutting `Grep`, not just `legacy/window.rs`. Verification gate: `cargo build --workspace --examples` green.
- **Task 25 dependencies + verification gates (FINDING+dep fix):** explicit dependency on `{20, 21, 22, 23, 24}`. Final-scan now runs all four Task-2 grep one-liners against the post-migration tree, plus zero-hit assertions for `BorrowMutError` and `TRACK_THREAD_BORROWS`.
- **New Q8 (FINDING D):** `AppContext::as_mut` returning `pub struct GpuiBorrow<'a, T>` ([app.rs:2714](crates/flui-core/src/app.rs#L2714)) is a load-bearing trait surface decision. Three candidate handlings documented (widen trait / structured-panic / split-trait); Task 5 spec MUST decide.
- **New Q9 (FINDING I):** `Application` Drop-order preservation invariant — the `// Drop globals last` comment at `app.rs:622-627` MUST be honored under the new cell. Spec-level constraint; cross-links to K12.
- **New Q10 (FINDING J):** `Application: Clone` is currently absent (verified: zero hits for `impl Clone for Application`). K07 leaves this absent; adding `Clone` widens public API beyond K07's mandate. Documented for clarity.

Net: revision 2 is a fact-grounded refinement; task COUNT unchanged (45) — all findings are folded into existing tasks. Task 5 (spec author) gains 3 new mandatory sub-decisions (Q8, Q9, Q10).

### Revision 3 (2026-05-09 aif-improve round 2)
Project-convention pass against revision 2; 3 new tasks + 2 task improvements + 2 new questions:
- **CHANGELOG.md task missing (FINDING S):** `CHANGELOG.md` exists (introduced in S07.5b, Keep a Changelog format). S21 has an entry; K99 and K15 do NOT have entries (verified: `grep K15|K99|AppCell|reentrancy CHANGELOG.md` count = 0). K07 is BREAKING and warrants an explicit `[Unreleased]` entry. Added Task 32a.
- **Migration guide convention not followed (FINDING T):** `CHANGELOG.md` references `docs/superpowers/migrations/animation-flutter-parity.md` as a precedent. The `migrations/` subdirectory does NOT yet exist (verified: `docs/superpowers/` contains only `specs`). K07 is BREAKING in user-facing ways (AsyncApp surface, `BorrowMutError` removal, `AppContext::as_mut` per Q8). Added Task 32b: author migration guide at `docs/superpowers/migrations/K07-appcell-removal.md` with Before/After samples for the 6 known breaking-change classes.
- **AGENTS.md update missing (FINDING U):** [AGENTS.md:15](AGENTS.md#L15) lists `AppCell` among 24+ architectural debt items. K07 closes E3 and the AGENTS narrative MUST reflect that. Added Task 32c.
- **Task 32 too narrow (FINDING V):** original `/aif-docs` checkpoint covered rustdoc + README only. Now Tasks 32a/32b/32c are sibling tasks; commit 14 batches them.
- **Task 1 baseline weak (Task 1 improvement):** explicitly capture `cargo test -p flui-core --lib` count (K15 baseline 344; post-K15 ≈ 361).
- **Q11 (CHANGELOG backfill):** added — K99/K15 retro entries vs K07-only; recommended K07-only.
- **Q12 (PR title):** added — `K15 — …` style vs `chore(k07)!:` style; recommended K07 follows K15 style.

Net: revision 3 raises task count from 45 → 48 (+3 documentation-completeness tasks). Open questions: 10 → 12. Done criteria: +3 (21d, 21e, 21f).

### Revision 4 (2026-05-09 post-spike risk audit)
Phase 1 spike completed (Tasks 1-2 done; Task 3 candidate spike + Task 4 rust-api-migration-auditor research dispatched as 3 parallel general-purpose agents — Agent 1 UI-framework comparison, Agent 2 Rust borrow-primitive comparison, Agent 3 local candidate spike). All Q1-Q12 resolved with rationale. Candidate B locked. Risk audit produced 12 findings (R1-R12); 4 require plan apdates:

- **Candidate B locked** — see "Recommended candidate (LOCKED — revision 4)" §. Three rejected alternatives documented with reasons.
- **Recount: `this.upgrade()` patterns = 5 distinct sites** (not ~30 as plan rev 1-3 claimed). Spike-confirmed at app.rs:215, context.rs:74,110,166, test_context.rs:660. Migration is mechanical under Candidate B because callback factories generate the upgrade pattern but do not change shape.
- **K15 has 6 Known Limitations**, not 4. Plan rev 1-3 undercounted (K15 spec line 197-202 enumerates 6: AsyncApp non-`try_borrow_mut`, AsyncApp::as_mut panic, web dispatcher exposure, `AppBorrowed` no source loc, K17-deferred behavioral tests, panic-leak on `currently_updating_entity` / `window_update_stack`). Limitation #6 is K07's explicit obligation per inline comment at app.rs:2483 — plan now has dedicated Task 14a.
- **R5 — Task 14a (NEW)**: closes K15 Limitation #6. Raw-pointer field-projection guards restore `currently_updating_entity` and `window_update_stack` on panic. Property tests: `prop_currently_updating_entity_restored_after_panic`, `prop_window_update_stack_restored_after_panic`, and `prop_pending_updates_zero_after_panic_through_update_entity`.
- **R2/R3/R4 — Task 8 expansion**: compile-time auto-trait tests via `static_assertions` (assert `!Send + !Sync` on `AppCell` / `AppRef` / `AppRefMut`); explicit `UnwindSafe` test; explicit `#[track_caller]` discipline checklist.
- **R7 — Q6 RESOLVED**: PR-blocking scoped Miri (`cargo +nightly miri test -p flui-core cell`), not workspace-wide. Cost bounded; the only way to audit Stacked-Borrows soundness for the cell.
- **R9/R10/R12 — Task 5 design spec adds "Future considerations" section**: K05 partial-borrow caveat; Phase III `App: Send` blocker; drop-on-panic semantics warning (preserves pre-K07 behavior, requires explicit module rustdoc).
- **Risk assessment table rewritten** — 4-tier severity model (Tier 1 PR-blocking, Tier 2 review-gating, Tier 3 future-coupling, Tier 4 honest limitations).

Net: revision 4 raises task count from 48 → **50** (+1 expanded Task 8 sub-deliverables, +1 new Task 14a). Open questions: 12 → 12 (Q6 resolved, but Q1-Q12 all now have committed answers in plan; spec authoring just transcribes them). Done criteria: +2 (R5 closure, R2/R3/R4 compile-time tests).

### Revision 5 (2026-05-09 — Task 6 adversarial review absorbed)

Three reviewers dispatched in parallel on spec rev 1: 8 BLOCKERs + 12 MAJORs + 10 MINORs. Plan changes (spec rev 2 captures all):

- **BLOCKERs absorbed (cross-confirmed by ≥2 reviewers):**
  1. `WindowUpdateGuard`/`EntityUpdateGuard` types do not exist (verified: zero matches via grep). K15 plan documented them but implementation chose inline push/pop. Spec K15 contract preservation table CORRECTED to reflect actual inline pattern.
  2. `UnwindSafe` claim was believed inverted in rev 5; rev 6 later corrected this again after std-source verification. Final Task 8 uses auto-trait assertions only, no manual impls.
  3. `EntityScope { app: &mut App }` does NOT compile under Candidate B's `AppRefMut` — same conflict K15 documented. Task 14a was temporarily re-scoped to `catch_unwind(AssertUnwindSafe(...))`, then rev 6 replaced that with raw-pointer field-projection guards.
  4. Test `borrow_mut_error_converts_to_app_borrowed` at reentrancy.rs:253 will fail compile after Task 9. Task 9 amended to MANDATORY deletion (was "if any").

- **BLOCKERs from single reviewer:**
  5. `#[track_caller]` on `Drop::drop` is no-op in Rust. Plan rev 4 required it — wrong. Task 8 amended: ONLY acquire-side methods carry `#[track_caller]`.
  6. `AsyncApp::app()` private widening cascades to 10+ public methods. Q4 amended: methods returning `Result<T>` propagate via `?`; methods returning `T` use typed `std::panic::panic_any(e)` (rev 6 correction). Net: panic semantics preserved, payload structured.
  7. Four other `as_mut` panic sites (async_context.rs:412, test_context.rs:68, headless_app_context.rs:230, visual_test_context.rs:430) not covered by Q2. Task 15 + Q2 amended to cover all 5.
  8. `AppCell::new` standalone constructor incompatible with `Rc::new_cyclic`. Task 8 amended: NO standalone constructor; cell constructed inline via `Rc::new_cyclic` in `App::new_app`.

- **MAJORs absorbed:**
  - `From<BorrowMutError>` deletion is semver MAJOR break (not "becomes redundant"). CHANGELOG (Task 32a) flags explicitly.
  - `pub use cell::{...}`, NOT `pub(crate) use`. `lib.rs:125 pub use app::*` requires `pub use` for the test-context fields to remain accessible.
  - `pub Rc<AppCell>` on test contexts preserved via Deref — works as-is.
  - `AsyncContextAsMut` variant is panic-only Display, never returnable in Result. Documented as dual-nature in spec.
  - `BorrowState` needs `#[derive(Debug)]` for `unreachable!("{:?}", other)`.

- **MINORs absorbed:**
  - TLCell "drop-in replacement" softened to "functional alternative requiring API surface changes" in Future Considerations.
  - "3 unsafe blocks" → temporarily "4 unsafe blocks" in rev 5; rev 6 later re-corrects this to 2 cell projection blocks + 2 `app.rs` raw-pointer field-guard writes, with no unsafe impl blocks.
  - Tree Borrows vs Stacked Borrows separated: Q6 PR-blocking is Stacked Borrows; Tree Borrows added as scheduled non-blocking gate.

- **NEW Task 14b** (paint/dispatch hot-path audit) — covers `flui-arch-reviewer` BLOCKER A1 (Key Principle #8 paint/dispatch reachability).

- **False positives identified:**
  - `migration-risk-adversary` finding #22 (Cargo.lock policy contradiction with `static_assertions` dev-dep) — `rust-api-migration-auditor` verified static_assertions v1.1.0 is ALREADY in lockfile transitively via postage v0.5.0. No policy violation.

Net: revision 5 raises checkbox task count from 49 → **50** (+1 hot-path audit). Rev 6 later supersedes the manual-UnwindSafe and catch-unwind portions of this revision.

### Revision 6 (2026-05-09 — second adversarial review absorbed)

Rev 2 itself was reviewed by 4 agents and several rev 5 fixes were corrected:

- Dropped manual `UnwindSafe` / `RefUnwindSafe` impls entirely; final Task 8 uses auto-trait assertions.
- Replaced the rejected `catch_unwind(AssertUnwindSafe(...))` Task 14a path with raw-pointer field-projection guards that do not cross `App::update`'s `finish_update` frame.
- Moved the `async_context.rs:96` `.map_err(ReentryError::from)?` deletion into Task 9 so commit 6 remains green.
- Switched Q2/Q4 panic payloads to typed `std::panic::panic_any(e)`.
- Added Task 7a, Task 8a, Task 26b, and Task 27a.

Net: revision 6 raises checkbox task count from 50 → **54**.

### Revision 7 (2026-05-10 — aif-improve drift cleanup)

Plan/spec consistency pass before implementation:

- Added **Task 7b** as an explicit pre-implementation spec-scrub gate, bringing the active plan to **55 checkbox tasks**.
- Synchronized active task count, dependency graph, commit plan, and Done criteria with rev 6 additions (`7a`, `8a`, `26b`, `27a`) plus the new `7b`.
- Removed actionable Task 14a `catch_unwind(AssertUnwindSafe(...))` instructions; rejected rev 5 catch/resume text now lives only in refinement history.
- Updated K15 test accounting: Task 9 deletes `borrow_mut_error_converts_to_app_borrowed`; Task 26 adds a direct AppCell contention replacement instead of claiming all 11 K15 tests pass unchanged.
- Converted Task 26b from a Criterion benchmark to an existing-style `examples/bench/app_cell.rs` microbenchmark because `criterion` is not in `Cargo.lock` and K07 keeps the no-lockfile-change non-goal.
- Added Linux package prerequisites / verified-feature-set requirement to Task 27a Miri CI.
- Replaced stale active counts (`~248`, `175 + 73`, `~30`) with `.k07-recon.txt` facts: **103** narrow AppCell-derived borrow hits and **5** `this.upgrade()` sites.

## Next steps

After K07 lands and is merged:

```
/aif-plan full K05-element-context-object
```

K05 inherits: the new `AppCell` primitive, the discharged K15 Known Limitations, the AsyncApp surface redesign, the `From<BorrowMutError>` removal, and any new `ReentryError` variants (`AppGoneAway` if Q4 chose Result-shape).
