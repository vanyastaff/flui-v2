# K07 — AppCell removal (token-based borrow model)

**Branch:** `feature/K07-appcell-removal-token-borrow`
**Created:** 2026-05-09
**Refined (aif-improve):** 2026-05-09 — codebase-grounded fact pass; recount precision (`~248` → `100-220, exact in Task 2`); added open questions Q8 (`AppContext::as_mut` widening), Q9 (Drop-order semantics), Q10 (`Application: Clone` status); reentrancy.rs rustdoc cleanup added to Task 9; executor.rs test-fixture clarified; explicit parallel labels on Tasks 15-19; examples scan widened in Task 24; explicit dep on Task 25 from {20,21,22,23,24}.
**Refined (aif-improve round 2):** 2026-05-09 — project-convention pass; +3 doc-completeness tasks (32a CHANGELOG entry, 32b migration guide at `docs/superpowers/migrations/K07-appcell-removal.md`, 32c AGENTS.md update); Q11 (CHANGELOG backfill policy) + Q12 (PR title convention) added; Done criteria 21d/21e/21f added.
**Refined (post-spike risk audit):** 2026-05-09 — Phase 1 spike + 3-agent research (UI frameworks, Rust borrow primitives, local candidate analysis) **lock in Candidate B** (UnsafeCell + flag → ReentryError). 4 risk-driven apdates: Task 14a (R5 — panic-leak fields RAII fix, K15 Limitation #6 closure), Task 8 expansion (R2/R3/R4 — auto-trait + `#[track_caller]` compile-time tests), Q6 re-eval to PR-blocking Miri, "Future considerations" section in design spec for R9/R10/R12. Q1-Q12 all resolved with rationale. Recount: `this.upgrade()` = **5 distinct sites** (not ~30); K15 has **6 Known Limitations** (not 4). 48 → **50 tasks** (+1a/14a, +Future-considerations).
**Refined (rev 5 — Task 6 adversarial review absorbed):** 2026-05-09 — three reviewers (`flui-arch-reviewer`, `migration-risk-adversary`, `rust-api-migration-auditor`) returned **8 BLOCKERs + 12 MAJORs + 10 MINORs**. Comprehensive triage: spec rev 2 published with all BLOCKERs patched. Major plan changes: Task 8 (Drop NO `#[track_caller]` — no-op; `#[derive(Debug)]` on BorrowState; manual `unsafe impl UnwindSafe`; `pub use cell::{...}` not `pub(crate) use`; NO standalone `AppCell::new` — Rc::new_cyclic integration); Task 9 (MANDATORY deletion of `borrow_mut_error_converts_to_app_borrowed` test at reentrancy.rs:253-259); Task 14a (re-scoped — `EntityScope` does NOT compile, replaced by `catch_unwind(AssertUnwindSafe(...))` pattern, also covers `window_update_stack`); Task 14b (NEW — paint/dispatch hot-path audit for Key Principle #8); Task 15 (5 `as_mut` panic sites, not 1; AsyncApp::app() cascade policy explicit per public method). 50 → **51 tasks** (+1 hot-path audit). Cross-confirmed BLOCKERs: WindowUpdateGuard/EntityUpdateGuard fabricated (don't exist), UnwindSafe inverted, EntityScope won't compile, BorrowMutError test orphan. False-positive: static_assertions Cargo.lock contradiction (already in lockfile transitively).
**Refined (rev 6 — second adversarial review of rev 2 absorbed):** 2026-05-09 — 4 agents reviewed rev 2 (3 adversarial + 1 quality general-purpose). **Rev 2 introduced new BLOCKERs while fixing rev 1's.** rust-api-migration-auditor verified std source: `unsafe impl UnwindSafe` is `error[E0199]` (UnwindSafe is not unsafe trait); UnwindSafe regression narrative is FALSE (`UnsafeCell<T>: UnwindSafe` automatic, no negative impl). Migration-risk found: `catch_unwind` pattern leaks `pending_updates` via `resume_unwind` through `App::update::finish_update` — silent permanent regression. Arch + migration cross-confirmed: catch_unwind also clears entity guard while leaving slot leaked (worse than K15). Major rev 6 plan changes: Task 8 (DROP `unsafe impl UnwindSafe`/`RefUnwindSafe`; auto-trait + `assert_impl_all!(UnwindSafe)` + `assert_not_impl_any!(RefUnwindSafe)` regression guards; `debug_assert!` in Drop cfg-gated to avoid abort-on-double-panic); Task 9 (atomic with `async_context.rs:96` line update — fix compile-error window); Task 14a (re-re-scoped — `catch_unwind` LEAKS pending_updates; replaced with raw-pointer field-projection guard pattern: `Guard { ptr: *mut Field, prev }`, Drop runs `*self.ptr = self.prev` without crossing `App::update` frame); Task 14b (unconditionally PR-blocking; output committed not gitignored); Task 15 (Q4 cascade uses `panic_any(ReentryError::AppGoneAway)` for typed payload, not `panic!("{}", e)`); 5 `as_mut` Display rephrased context-agnostic. New tasks: 7a (audit whether `try_borrow` shared-cellAvenues are needed — possibly drop `BorrowState::Shared` entirely), 8a (Cargo.toml `[dev-dependencies]` += static_assertions), 26b (criterion bench `bench_borrow_mut_acquire_release`), 27a (CI job for `cargo +nightly miri test -p flui-core cell`). 51 → **55 tasks**.
**Phase:** 0-K (Kernel Cleanup) — third spec in the critical chain (gates K05 → K01 → K02 → K03 → K04 → Phase II-F)
**Type:** structural refactor of the App ownership primitive (replaces `RefCell<App>` with a compile-time-checked borrow model). **HIGH-RISK** per ROADMAP — but Phase 1 spike + 3-agent research (UI framework comparison, Rust borrow primitive comparison, local candidate analysis) **LOCKED IN Candidate B** (hand-rolled `UnsafeCell<App>` + `BorrowState` flag returning `ReentryError`). Migration is signature-compatible (~200 LoC primitive + 0 callsite-LoC; 103 narrow-pattern AppCell-derived callsites compile unchanged after the cell rewrite).
**Tasks:** 55 (45 + 3 doc-completeness from rev 3 + 2 risk-driven from rev 4 + 1 hot-path audit from rev 5 + 4 from rev 6 second adversarial review).
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
  - `App.this: Weak<AppCell>` ([app.rs:585](crates/flui-core/src/app.rs#L585)) — back-pointer used by ~30+ `this.upgrade()?` callsites in App methods
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
| Internal back-pointer | `App::this: Weak<AppCell>` ([app.rs:585](crates/flui-core/src/app.rs#L585)) | used by `~30+` `this.upgrade()?.borrow_mut()` patterns |
| Async context | `AsyncApp { app: Weak<AppCell>, … }` ([async_context.rs:23](crates/flui-core/src/app/async_context.rs#L23)) | Holds `Weak`; calls `.upgrade()` per operation |
| Test contexts | `TestAppContext`, `TestApp`, `VisualTestContext`, `HeadlessAppContext` each own `Rc<AppCell>` | mirrors `Application` shape |
| Mutable-borrow API | `AppCell::borrow_mut() -> AppRefMut<'_>`, `try_borrow_mut() -> Result<…, BorrowMutError>` | runtime borrow check |
| Re-entry detection | K15 contract via `WindowUpdateGuard` + `EntityUpdateGuard` + `double_lease_panic` unified Display | structured AT CALLBACK BOUNDARIES, but the primitive itself still uses `RefCell` |
| `app.borrow_mut()` / `try_borrow_mut()` callsites | 175 across 16 files | most are 1-3 chars per change → mass rename via tooling |
| `app.borrow()` callsites | 73 across 13 files | as above |
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
- `unsafe` confined to one file (the cell impl); `wgpu-gpu-reviewer` does NOT need to engage (cell is independent of GPU path).

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
4. **K15 contract preservation:** Candidate B's cell flag = `ReentryError::AppBorrowed`; K15's `WindowUpdateGuard` / `EntityUpdateGuard` (same-target re-entry detection) are ORTHOGONAL — different concerns, both stay. No K15 contract regression.
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
  - **`migration-risk-adversary`** — paranoid sweep: what functionality is lost / silently regressed when ~248 callsites migrate? Specifically: subscription handler signatures, observer callbacks, drop-time runs, async-spawn paths, web event-loop integration.
  - **`rust-api-migration-auditor`** — semver impact, trait object safety, feature flag matrix, MSRV idiom usage, `unsafe` audit.
  - Goal: each agent returns a list of BLOCKER / MAJOR / MINOR findings with file:line citations.

- [x] **Task 7.** ✅ **(rev 5 — completed 2026-05-09.)** Comprehensive triage of 8 BLOCKERs + 12 MAJORs + 10 MINORs. ALL BLOCKERs patched into spec rev 2 + plan rev 5. ALL MAJORs absorbed (most into spec rev 2 directly; semver MAJOR documented in Task 32a CHANGELOG). MINORs: TLCell language softened; "3 → 4" unsafe block count corrected; Tree Borrows vs Stacked Borrows separated. False positive (Cargo.lock contradiction) identified via cross-reviewer triangulation. Spec rev 2 Decision log §"Revision 2 (post-Task 6 adversarial review)" enumerates all changes.

  Original Task 7 instructions retained for reference:

### Phase 1.5 — pre-implementation audits (rev 6 NEW)

- [ ] **Task 7a. (NEW — rev 6, quality review)** Audit whether `try_borrow` (shared) has any genuine callers in flui-core. Steps:
  - `grep -rEn 'app.*\.borrow\(\)' crates/flui-core/src/` to find shared-borrow patterns.
  - For each hit, verify whether the call is actually shared (e.g., `app.borrow().platform.clone()` at app.rs:190 — borrow is held briefly for read-only access).
  - Decision criterion: if ZERO genuine shared-borrow callsites remain after migration, drop `BorrowState::Shared(NonZeroU32)` variant entirely. Cell becomes `enum BorrowState { Free, Mut }` — half the state machine, half the unsafe surface, simpler proptest.
  - If ≥1 genuine site remains, KEEP `BorrowState::Shared`. Document the audit result in `.k07-shared-borrow-audit.md` (gitignored).
  - Output: decision recorded in spec rev 4 (if rev 3 → 4 transition needed) OR plan note.
  - **Done criterion:** explicit decision recorded; if dropping Shared, Task 8's `BorrowState` enum simplified accordingly.

- [ ] **Task 8a. (NEW — rev 6, api-auditor MAJOR)** Add `static_assertions = "1"` to `crates/flui-core/Cargo.toml` `[dev-dependencies]`. Verify:
  - Crate is already in `Cargo.lock` transitively via `postage v0.5.0` (no lockfile modification expected).
  - Run `cargo check -p flui-core --tests` after adding to verify resolution.
  - **Note:** the workspace policy "Does NOT modify Cargo.lock" applies to runtime deps; dev-deps that are already in the lockfile transitively don't trigger the policy.
  - File: `crates/flui-core/Cargo.toml` `[dev-dependencies]` section.

### Phase 2 — Public type surface (recommended Candidate B; pivot if Task 5 chooses A or C)

- [ ] **Task 8.** Create `crates/flui-core/src/app/cell.rs` (NEW module owned by `app/`). Contents:
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
    // UnwindSafe preservation — preserves pre-K07 RefCell<App> behavior:
    static_assertions::assert_impl_all!(AppCell: std::panic::UnwindSafe);  // OR not, depending on App: confirm pre-K07 behavior in Task 1 baseline.
    ```
    If `static_assertions` not yet in dev-deps, add it (single-use is fine). All three negative assertions are MANDATORY — `App: !Send + !Sync` invariant must propagate compile-time.
  - **`#[track_caller]` propagation (rev 4 R4 mitigation, rev 5 amendment):** every `borrow*` method on `AppCell` MUST carry `#[track_caller]`. **Drop impls MUST NOT** — `#[track_caller]` on `Drop::drop` is a no-op per Rust semantics (drop glue does not carry caller Location). Logging from `Drop` records the drop-glue's internal location, not the borrow callsite. This is acceptable because acquire-side methods supply the diagnostic info.
  - **`Rc::new_cyclic` integration (rev 5 — adversarial review BLOCKER #8):** **NO standalone `AppCell::new(app)` constructor.** `App.this: Weak<AppCell>` requires cyclic init. The cell is constructed inline in `App::new_app` via `Rc::new_cyclic(|this: &Weak<AppCell>| AppCell { app: UnsafeCell::new(App { this: this.clone(), … }), borrowed: Cell::new(BorrowState::Free), _not_send: PhantomData })`. No public constructor exposed.
  - **(rev 6 — adversarial review BLOCKER A — corrects rev 5):** **NO manual `unsafe impl UnwindSafe` or `unsafe impl RefUnwindSafe` blocks.** Two facts that rev 5 got wrong:
    1. `unsafe impl UnwindSafe` is `error[E0199]` — `UnwindSafe` is NOT an `unsafe trait`. Same for `RefUnwindSafe`.
    2. `UnsafeCell<T>: UnwindSafe` is automatic (NO negative impl in std). The "regression" rev 5 was trying to prevent doesn't exist. api-auditor verified `core/src/panic/unwind_safe.rs:181-202` (1.95 toolchain): only `!RefUnwindSafe for UnsafeCell` exists; no `!UnwindSafe`. Both pre-K07 and post-K07 `AppCell` are `UnwindSafe` (auto), `!RefUnwindSafe` (auto via UnsafeCell).
    Replace rev 5's `unsafe impl` blocks with:
    ```rust
    // Lock auto-trait behavior as regression guard — no manual impl needed.
    // Pre-K07 RefCell<App>: UnwindSafe (auto). Post-K07 UnsafeCell<App>: UnwindSafe
    // (auto). Identical. The assertion below catches any future code change that
    // accidentally introduces a !UnwindSafe field.
    static_assertions::assert_impl_all!(AppCell: std::panic::UnwindSafe);
    // RefUnwindSafe stays !-impl'd via UnsafeCell's negative impl. Matches pre-K07.
    static_assertions::assert_not_impl_any!(AppCell: std::panic::RefUnwindSafe);
    ```
  - **`#[derive(Debug)]` on `BorrowState` (rev 5 — adversarial review MINOR):** required because `unreachable!("{:?}", other)` in `AppRef::Drop` formats `BorrowState`. Without Debug, the unreachable! arm is a compile error. Change `#[derive(Clone, Copy)]` to `#[derive(Clone, Copy, Debug)]`.
  - **(rev 6 — `debug_assert!` in Drop = abort vector mitigation):** `debug_assert!(matches!(self.cell.borrowed.get(), BorrowState::Mut))` in `AppRefMut::Drop` was specified in rev 1-5. If this assert fires during stack unwind from a panicking inner closure, double-panic → process abort. To avoid: gate with `#[cfg(debug_assertions)] { if !std::thread::panicking() { debug_assert!(...); } }` so the assertion is skipped during unwinding. Same treatment for `unreachable!("{:?}", other)` in `AppRef::Drop`.
  - File: `crates/flui-core/src/app/cell.rs` (new), `crates/flui-core/src/app.rs` (delete lines 75-135 incl. `AppCell`/`AppRef`/`AppRefMut`; replace with `mod cell; pub use cell::{AppCell, AppRef, AppRefMut};` — **`pub use`, NOT `pub(crate) use`** (rev 5 — adversarial review MAJOR M10): `lib.rs:125 pub use app::*` does not re-export `pub(crate)` items, breaking `HeadlessAppContext.app: Rc<AppCell>` field type accessibility for `test-support` consumers).
  - Module-level rustdoc explains the new contract: "AppCell is a single-mutable-borrow cell; recursive borrow_mut produces ReentryError::AppBorrowed via the K15 contract. Use cx.defer to schedule work that must touch App. Drop-on-panic releases the borrow flag but does NOT undo partial mutations to App — App is in best-effort consistent state after a panicking closure (matches pre-K07 RefCell semantics)."

- [ ] **Task 9.** Update `crates/flui-core/src/reentrancy.rs`:
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
  - VERIFY all 11 K15 reentrancy tests still pass (Task 26 ports them forward, but Task 9 must not break them in the meantime).
  - Files: `crates/flui-core/src/reentrancy.rs`. No new dependencies.

- [ ] **Task 10.** Update `crates/flui-core/src/prelude.rs` — no change unless the design spec exposes a new public type. The existing `ReentryError` and `ReentryMode` re-exports from K15 stay.

### Phase 3 — Internal migration: `App`, `Application`

- [ ] **Task 11.** Migrate `App::new_app` ([app.rs:684](crates/flui-core/src/app.rs#L684)) to return the new `Rc<AppCell>` shape (signature unchanged — both shapes are `Rc<AppCell>`; only the internals differ).

- [ ] **Task 12.** Migrate `Application` ([app.rs:139-241](crates/flui-core/src/app.rs#L139)). Each `self.0.borrow()` / `self.0.borrow_mut()` callsite (lines 160, 170, 179, 190, 192, 203, 213-216, 224, 229, 234, 239) becomes a method call on the new `AppCell`. Mass replace; semantics preserved.

- [ ] **Task 13.** Migrate `App::this: Weak<AppCell>` consumers. `git grep 'this.upgrade()' crates/flui-core/src/app.rs` to enumerate; expect ~30 sites. Each `Weak::upgrade()? .borrow_mut()` becomes `Weak::upgrade()? .borrow_mut()` against the new cell — identical at the use site. Verify by `cargo check` after this task.

- [ ] **Task 14.** Audit `App::pending_effects` queue / drain pathway ([app.rs:603 + 1389-1424](crates/flui-core/src/app.rs#L603)) for any reliance on `RefCell::borrow_mut` panic shape. K15's `WindowUpdateGuard` covers this. Confirm by running existing K15 reentrancy tests against the new primitive — they MUST pass unchanged.

- [ ] **Task 14a.** **(rev 4 — closes K15 Known Limitation #6.)** Fix the panic-leak class on `currently_updating_entity` and `window_update_stack` fields. Per K15 design spec line 202: *"No panic-safety on `currently_updating_entity` and `window_update_stack` fields on the panic-during-update path — same as the pre-K15 manual push/pop pattern. Acceptable parity; not a regression."* And K15 inline comment at [app.rs:2483-2496](crates/flui-core/src/app.rs#L2483): *"RAII guards were considered and rejected during planning because they conflict with Rust borrow rules — a guard borrowing `&mut App` cannot coexist with `App` flowing through this closure body. **Fixing both panic-leak classes is K07's job (it redesigns the borrow primitive).**"*
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

  - **OLD `catch_unwind` text retained for historical reference (rev 5 plan):**
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
            panic!("{}", err);
        }
        self.update(|cx| {
            let prev_updating = cx.currently_updating_entity.replace(id);
            // rev 5 — wrap the panic-prone body in catch_unwind so we can
            // restore `currently_updating_entity` even on panic.
            // AssertUnwindSafe required because cx: &mut App is !UnwindSafe.
            // SAFETY: the cell is UnwindSafe (manual impl in app/cell.rs);
            // App's internal state may be partially mutated on panic,
            // matching pre-K07 behavior. We restore only the K15 field.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut entity = cx.entities.lease(handle);
                let r = update(&mut entity, &mut Context::new_context(cx, handle.downgrade()));
                cx.entities.end_lease(entity);
                r
            }));
            cx.currently_updating_entity = prev_updating;
            match result {
                Ok(r) => r,
                Err(payload) => std::panic::resume_unwind(payload),
            }
        })
    }
    ```
  - **`window_update_stack` panic-safety (rev 5 — verify K15 inline pattern):** at [app.rs:1605-1611](crates/flui-core/src/app.rs#L1605), the `cx.window_update_stack.push(id)` / `.pop()` pair lives across `update(root_view, &mut window, cx)` and `trail(id, window, cx)?`. Both can panic. K15's inline pattern leaves the stack dirty if either panics. Apply the same `catch_unwind(AssertUnwindSafe(...))` wrap around the body between push and pop. Same for `App::open_window` at [app.rs:1109-1111](crates/flui-core/src/app.rs#L1109).
  - **Test addition (Task 26 expansion):** new property test `prop_currently_updating_entity_restored_after_panic` — `std::panic::catch_unwind` wraps an `update_entity` call whose closure panics; assert `app.currently_updating_entity == None` after catch. Same for `window_update_stack` — `prop_window_update_stack_restored_after_panic`.
  - **NO `scopeguard` dep** — replaced by `catch_unwind` from std. Phase 0-K dep-minimization preserved.
  - File: `crates/flui-core/src/app.rs:2469-2505` (update_entity), `1559-1641` (update_window_id including trail()), `1080-1115` (open_window). Logging: `log::trace!(target: "flui_core::app::reentry", "currently_updating_entity restored after panic");` after `resume_unwind`.
  - **Done criterion:** running existing 11 K15 tests, then `prop_currently_updating_entity_restored_after_panic` AND `prop_window_update_stack_restored_after_panic` — all green. K15 spec line 202 stated "Acceptable parity; not a regression"; K07 closes Limitation #6 explicitly via this catch_unwind pattern.

- [ ] **Task 14b. (NEW — rev 5; rev 6 — UNCONDITIONALLY PR-blocking; arch-reviewer MAJOR 3)** Paint/dispatch hot-path reachability audit for `try_borrow_mut`. ARCHITECTURE Key Principle #8 compliance is a CORRECTNESS invariant, not optional.
  - **Scope expanded (rev 6):** grep includes `crates/flui-core/src/elements/` observer dispatch paths AND `crates/flui-core/src/window.rs` (Window::draw / Window::dispatch_event), NOT just direct `Window::*` callsites.
  - Steps:
    1. `grep -rEn 'try_borrow_mut|borrow_mut' crates/flui-core/src/`
    2. For each hit, trace caller upward through `cargo expand` if needed to determine reachability from `Window::draw` / `Window::dispatch_event` / `Element::paint` / observer dispatch.
    3. Build a callgraph table: file:line → caller chain → hot-path reachability (yes/no) → expected frequency.
  - **Output committed to `docs/superpowers/audits/K07-hot-path-audit.md`** (NOT gitignored — durable record for Tasks 42-44 re-review).
  - **If audit finds ANY reachable hits**: spec rev 3+ Known Limitations section escalated; route reachable calls through deferred-effect path OR document the per-frame perf cost.
  - **If audit finds zero hits**: spec's permissive interpretation of Key Principle #8 stands; document audit conclusion explicitly.
  - **Status: PR-blocking unconditionally.** Audit MUST run and produce a definitive answer (yes/no with citations) before K07 PR can merge.

### Phase 4 — Internal migration: `AppContext` implementors

> **Tasks 15-19 are PARALLEL-SAFE.** Each touches a separate file (`app/async_context.rs`, `app/test_context.rs`, `app/test_app.rs`, `app/headless_app_context.rs`, `app/visual_test_context.rs`). They share no symbols beyond the `AppCell` type itself (Task 8). Run as 5 concurrent tasks once Task 14 lands.

- [ ] **Task 15.** [PARALLEL with 16-19] Migrate `AsyncApp` ([app/async_context.rs](crates/flui-core/src/app/async_context.rs)). 15+ `borrow_mut()` callsites. Per the K15 deferral, this is also where the 10+ unstructured sites get structured:
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

- [ ] **Task 16.** [PARALLEL with 15, 17-19] Migrate `TestAppContext` ([app/test_context.rs](crates/flui-core/src/app/test_context.rs)). 19+ callsites. `app: Rc<AppCell>` field type unchanged. Verify `Rc::downgrade(&self.app)` at [test_context.rs:425](crates/flui-core/src/app/test_context.rs#L425) still produces a valid `Weak<AppCell>` for the `to_async()` path.

- [ ] **Task 17.** [PARALLEL with 15-16, 18-19] Migrate `TestApp` ([app/test_app.rs](crates/flui-core/src/app/test_app.rs)). 8 callsites. Two `app: Rc<AppCell>` fields (lines 41 and 322 in different impl blocks) unchanged. Verify `Rc::downgrade(&self.app)` at [test_app.rs:226](crates/flui-core/src/app/test_app.rs#L226) still produces a valid `Weak<AppCell>`.

- [ ] **Task 18.** [PARALLEL with 15-17, 19] Migrate `HeadlessAppContext` ([app/headless_app_context.rs](crates/flui-core/src/app/headless_app_context.rs)). 11 callsites. `app: Rc<AppCell>` field unchanged.

- [ ] **Task 19.** [PARALLEL with 15-18] Migrate `VisualTestContext` ([app/visual_test_context.rs](crates/flui-core/src/app/visual_test_context.rs)). 11 callsites. `app: Rc<AppCell>` field unchanged.

### Phase 5 — Internal migration: Element / Window / platform / subscription

- [ ] **Task 20.** Migrate `crates/flui-core/src/elements/`. Files: `uniform_list.rs` (6+5=11), `div.rs` (8+11=19), `list.rs` (14+6=20), `text.rs` (3+9=12). Combined: ~62 sites. Mass-replace; verify `cargo check` after each file.

- [ ] **Task 21.** Migrate `crates/flui-core/src/subscription.rs` (3 sites). Cross-check K15 `SubscriberSet::retain` snapshot pattern still holds.

- [ ] **Task 22.** Migrate `crates/flui-core/src/executor.rs` test fixture only. The site is [executor.rs:556-573](crates/flui-core/src/executor.rs#L556) — `#[cfg(test)] fn create_test_app() -> (TestDispatcher, BackgroundExecutor, Rc<crate::AppCell>)` constructs an `Rc<AppCell>`; line 573 calls `app.borrow().foreground_executor.clone()`. NO production-code AppCell access. Migration: preserve the `Rc<AppCell>` return type (test-support surface invariant); replace `app.borrow()` with the new cell's shared-borrow API. Note: line 581's `*task_ran.borrow_mut() = true` is a `RefCell<bool>` (NOT AppCell-derived) — leave untouched.

- [ ] **Task 23.** Migrate `crates/flui-core/src/platform/` (9 files, ~72 sites in upper-bound count — narrow set is much smaller because most `borrow_mut` here is `RefCell<WindowState>` etc., not AppCell). Sub-tasks:
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

- [ ] **Task 24.** Examples comprehensive scan & migration. Steps:
  - `Glob examples/**/*.rs` to enumerate every example file in the workspace.
  - `Grep AppCell|app\.borrow_mut\(\)|app\.borrow\(\)` across the result.
  - Known: `crates/flui-core/examples/legacy/window.rs` (the 2 callsites K15 updated for `prompt`).
  - Migrate every match; verify each example still compiles via `cargo build --workspace --examples`.
  - If any example uses `AppCell` directly (rare — usually goes through `Application`), audit the use and update.

- [ ] **Task 25.** Final scan (depends on Tasks 20, 21, 22, 23, 24 — all migrations complete). Steps:
  - Run all four Task-2 grep one-liners (Steps 2.1-2.4) against the post-migration tree.
  - Compare AppCell-derived count: MUST be zero. Every remaining `.borrow_mut()` / `.borrow()` hit must be against a non-AppCell `RefCell` (`Window`, `Keymap`, `Arena`, `Cell<bool>` test fixtures, etc.). Audit each remaining hit.
  - `grep -rn 'AppCell|AppRef|AppRefMut' crates/flui-core/src/`: every hit must be (a) the new module declaration, (b) the type definition, (c) `Rc<AppCell>` / `Weak<AppCell>` storage in the 12 known sites, or (d) the new module's tests.
  - `grep -rn 'BorrowMutError' crates/flui-core/src/`: MUST return zero hits (Task 9 deletion).
  - `grep -rn 'TRACK_THREAD_BORROWS' crates/flui-core/src/`: MUST return zero hits (Known Limitation #3 — replaced by `log::trace!`).
  - Document the post-migration counts in `.k07-recon-final.txt` (gitignored).

### Phase 6 — Test infrastructure & property tests

- [ ] **Task 26.** Add `crates/flui-core/src/app/cell/tests.rs` (or `crates/flui-core/tests/app_cell.rs`) using existing `proptest` dev-dep. New tests:
  - `prop_borrow_mut_then_borrow_mut_returns_app_borrowed_in_strict` — random nested-borrow sequences; same-cell nesting returns `Err(ReentryError::AppBorrowed)`.
  - `prop_drop_releases_borrow` — random `borrow_mut → drop` sequences; subsequent borrow succeeds.
  - `prop_panic_during_borrow_releases_borrow` — `std::panic::catch_unwind` wraps a borrow that panics; assert subsequent borrow succeeds (panic-safety guarantee).
  - `prop_borrow_share_count_caps` (if Candidate B uses `Shared(NonZeroU32)`): saturating add at u32::MAX returns `Err`; documented as a structural impossibility but pinned as a regression guard.
  - **Port forward all 11 K15 reentrancy tests** to the new primitive; confirm all green. K15 contract is preserved.

- [ ] **Task 27.** Run Miri test for K07 cell module (`cargo +nightly miri test -p flui-core cell` with default Stacked Borrows). PR-blocking gate per Q6 resolution. Tree Borrows (`MIRIFLAGS=-Zmiri-tree-borrows`) added as separate non-blocking gate via Task 27a CI job. Capture both outputs.

- [ ] **Task 27a. (NEW — rev 6, api-auditor MAJOR — Miri CI infrastructure)** Add CI job to `.github/workflows/ci.yml` that runs Miri scoped to the cell module. Required because Q6 declares Miri PR-blocking but no CI infrastructure exists. New job entry:
  ```yaml
  miri-cell:
    name: Miri (cell module — Stacked Borrows)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
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
      - uses: dtolnay/rust-toolchain@nightly
        with:
          components: miri
      - name: Miri Tree Borrows
        env:
          MIRIFLAGS: '-Zmiri-tree-borrows'
        run: cargo +nightly miri test -p flui-core cell
  ```
  Stacked Borrows is PR-blocking (Q6); Tree Borrows is non-blocking research signal. Docs Tree Borrows divergence in `.github/workflows/ci.yml` comments.

- [ ] **Task 26b. (NEW — rev 6, quality review)** Add criterion benchmark `bench_borrow_mut_acquire_release` in `crates/flui-core/benches/app_cell.rs` (or wherever existing benches live):
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
  Backs the spec's Known Limitation L1 ("flag check is sub-microsecond, not a hot-path concern") with empirical measurement. K07 gates K05 → K01 → SF04/SF05; if flag check turns out to be 50ns × 10k frame events = 500μs hot-path overhead, escalate. Bench result captured in `.k07-bench-results.txt` (gitignored).

- [ ] **Task 28.** Behavioral tests for the AsyncApp redesign (Task 15):
  - `async_app_update_after_app_drop_returns_app_gone_away` (was: silent `unwrap` panic).
  - `async_app_as_mut_after_drop_returns_structured_error` (or panics with structured Display, per Task 5 decision).
  - `async_app_borrow_mut_propagates_reentry_error` — emit re-entry from a spawned task; assert structured error, NOT `BorrowMutError`.

- [ ] **Task 29.** Update existing K15 tests in `crates/flui-core/src/reentrancy.rs` if any of them reference `std::cell::BorrowMutError` (Task 9 deletion). Use `grep -n 'BorrowMutError' crates/flui-core/src/reentrancy.rs`.

### Phase 7 — Documentation & spec close-out

- [ ] **Task 30.** Update `.ai-factory/RESEARCH.md` Active Summary with one-paragraph K07 entry. Mention: AppCell replaced by `flui_core::app::cell::AppCell` (the Candidate B/A/C primitive — fill in actual choice); 248 callsites migrated; K15 Known Limitation #1 (10+ AsyncApp sites) and #2 (`as_mut` panic) discharged; AppCell `#[doc(hidden)]` retained.

- [ ] **Task 31.** Mark K07 done in `.ai-factory/ROADMAP.md` — checkbox flip at line 58; completion-date row in `## Completed` table.

- [ ] **Task 32.** Run `/aif-docs` to absorb rustdoc / README drift. Confirm `cargo doc --workspace --no-deps` zero new warnings vs Task 1 baseline. Specifically:
  - The `_ownership_and_data_flow.rs` doctest references AppCell — verify it still compiles or is gated. (Note: K98 is the dedicated rewrite spec; for K07 we just keep it green.)
  - `flui-core::app` module docs reflect new primitive.

- [ ] **Task 32a.** Add `CHANGELOG.md` entry for K07 under `## [Unreleased]` section, following the S21 entry style ([CHANGELOG.md](CHANGELOG.md)). Required content:
  - Section heading: `## [Unreleased] — K07 AppCell removal (token-based borrow model)`.
  - One paragraph summarizing: AppCell replaced by `flui_core::app::cell::AppCell` (Candidate B/A/C — fill in choice); breaking changes (AsyncApp surface, `BorrowMutError` removal, K15 `From<BorrowMutError>` deletion); link to plan + design spec + migration guide (Task 32b).
  - "Migration guide:" line referencing Task 32b path.
  - **Decision (Q11):** K99 / K15 backfill — recommended NO (separate hygiene PR). Document this decision in the entry footer.
  - File: [CHANGELOG.md](CHANGELOG.md). Place above the S21 entry.

- [ ] **Task 32b.** Author migration guide at `docs/superpowers/migrations/K07-appcell-removal.md`. Pattern: same as `docs/superpowers/migrations/animation-flutter-parity.md` (referenced from CHANGELOG S21 entry). **Note:** the `migrations/` subdirectory may not exist yet — create it (`mkdir -p docs/superpowers/migrations`). Required sections:
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

- [ ] **Task 32c.** Update `AGENTS.md` to reflect K07 closure of E3:
  - [AGENTS.md:15](AGENTS.md#L15) currently lists `AppCell` among 24+ debt items: "broken Provider, …, AppCell, action globals, undefined re-entrancy contract, …". Remove `AppCell` from this list (and update the count if it's a literal "24+").
  - Add K07 to "Done" or status list per the file's convention (read AGENTS.md to determine the exact section).
  - Verify no other `AppCell` mention in `AGENTS.md` besides line 15.
  - File: [AGENTS.md](AGENTS.md).

- [ ] **Task 33.** Update CLAUDE.md if any new rule emerges from K07 (e.g., "do not pattern-match on `std::cell::BorrowMutError` from flui-core APIs — use `ReentryError::AppBorrowed`").

### Phase 8 — Validation gates

- [ ] **Task 34.** `cargo build --workspace --all-features` green.
- [ ] **Task 35.** `cargo test --workspace` green. Pre-existing test count increases by ≥ N (where N is the sum of new tests in Tasks 26-29; expected ≥ 7).
- [ ] **Task 36.** `cargo clippy --workspace --all-targets -- -D warnings` zero new warnings vs Task 1 baseline.
- [ ] **Task 37.** `cargo fmt --all -- --check` clean.
- [ ] **Task 38.** `cargo doc --workspace --no-deps` zero new warnings vs Task 1 baseline.
- [ ] **Task 39.** `cargo +nightly miri test -p flui-core cell` (Task 27) — ALL green. If miri not installed, document via `rustup component add miri`.
- [ ] **Task 40.** Manual smoke: run `cargo run --example nav_demo` ~30 seconds with `RUST_LOG=flui_core::app::cell=trace`. Verify zero `warn!` events under normal navigation.
- [ ] **Task 41.** Web platform smoke (if reachable from test infra): build with `--target wasm32-unknown-unknown` if a recipe exists; otherwise document gap and file follow-up.

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
        ├──► 13. App::this consumers (~30 sites)
        │              │
        │              ▼
        ├──► 14. pending_effects audit
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
        ├──► Tests:  26. proptest + K15-port ║ 27. Miri ║ 28. AsyncApp behavioral ║ 29. K15 fixup
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

   Critical path: 1 → 2 → 5 → 7 → 8 → 11 → 12 → 13 → 15 → 25 → 26 → 34 → 42 → 45
   Total: ~14 sequential nodes; ~6-8 parallel slots in the migration phases.
```

## Commit Plan

K07 has **45 tasks**. Per skill convention, commit checkpoints every 3-5 tasks. Each commit MUST be green at HEAD — `cargo build` + `cargo test` MUST pass. Several commits are documentation-only and easy to land standalone.

| # | Tasks | Conventional commit message |
|---|---|---|
| 1 | 1, 2 | `chore(k07): pre-flight baseline + cite-drift audit` |
| 2 | 3, 4 | `docs(k07): candidate spike notes + auditor input` (notes-only — may be dropped if spike artifacts kept untracked) |
| 3 | 5 | `docs(spec): K07 design spec — primitive choice, migration plan, Decision log rev-1` |
| 4 | 6, 7 | `docs(spec)!: K07 design spec rev-2 — adversarial-review absorbed` |
| 5 | 8, 9, 10 | `feat(app)!: introduce flui_core::app::cell::AppCell + remove BorrowMutError From` (BREAKING — `From<BorrowMutError>` impl removed; new public-but-doc-hidden `AppCell` module) |
| 6 | 11, 12, 13, 14 | `refactor(app): migrate App + Application + Weak<AppCell> consumers to new cell` |
| 7 | 15 | `refactor(async)!: AsyncApp surface — structure 10+ unstructured borrow_mut sites; resolve as_mut panic` (BREAKING — `AsyncApp::as_mut` signature change per Task 5 decision) |
| 8 | 16, 17, 18, 19 | `refactor(test): migrate Test/Visual/Headless contexts to new cell` |
| 9 | 20, 21, 22 | `refactor(elements): migrate elements + subscription + executor to new cell` |
| 10 | 23 | `refactor(platform)!: migrate flui-core::platform/* to new cell; update K15 deferral comments` |
| 11 | 24, 25 | `refactor(examples): migrate legacy example + final scan` |
| 12 | 26, 27, 28, 29 | `test(app::cell): proptest + Miri + AsyncApp behavioral + K15-port` |
| 13 | 30, 31 | `docs(research+roadmap): close K07; cross-references updated` |
| 14 | 32, 32a, 32b, 32c, 33 | `docs(rustdoc+changelog+migration+agents+claude): K07 sweep` |
| 15 | 34-41 | `chore(k07): validation pass — build/test/clippy/fmt/doc/miri/smoke/web` (likely empty — fold into commit 13/14 if no fixups) |
| 16 | 42, 43, 44, 45 | `docs(k07): adversary re-review triage` |

If commits 2 or 15 are empty, drop them. **Rollback note:** Commits 5-onwards have forward type-dependencies; rollback of K07 = revert all-as-unit (migration-risk finding pre-recorded).

## Done criteria

K07 is done when:

1. ✅ `crates/flui-core/src/app/cell.rs` (or equivalent path per Task 5) module exists with the chosen primitive; `unsafe` blocks all carry SAFETY comments; module-level rustdoc IS the new contract document.
2. ✅ `AppCell`, `AppRef`, `AppRefMut` keep `#[doc(hidden)]` (matches pre-K07 shape).
3. ✅ `Application(Rc<AppCell>)` shape preserved at the public surface; `App::this: Weak<AppCell>` shape preserved.
4. ✅ `AsyncApp`, `TestAppContext`, `TestApp`, `HeadlessAppContext`, `VisualTestContext` all migrated to the new primitive with their `Rc<AppCell>` / `Weak<AppCell>` field types unchanged in spelling.
5. ✅ All ~248 `borrow_mut()` / `borrow()` callsites migrated; final `grep` of `crates/flui-core/src/` returns zero AppCell hits (every remaining hit is non-AppCell `RefCell`).
6. ✅ The 10+ unstructured `app.borrow_mut()` sites in `app/async_context.rs` (lines 39, 45, 55, 65, 126, 135, 152, 168, 182) are now structured per Task 5 decision (K15 Known Limitation #1 discharged).
7. ✅ `AsyncApp::as_mut` panic at `app/async_context.rs:73` is replaced per Task 5 decision (K15 Known Limitation #2 discharged).
8. ✅ The K15 deferred decision on `ReentryMode::PanicLikeUpstream` is RESOLVED in the spec (re-introduced, dropped, or documented obsolete — pick one).
9. ✅ The 3 K15 platform deferral comments (`mac/platform.rs:500-502`, `mac/platform.rs:1254`, `windows/platform.rs:452-453`) are updated to reference K07.
10. ✅ `impl From<std::cell::BorrowMutError> for ReentryError` is REMOVED; `ReentryError::AppBorrowed` is now produced directly by the new `AppCell::try_borrow_mut`.
11. ✅ ALL 11 K15 reentrancy tests pass unchanged on the new primitive (contract preserved).
12. ✅ New tests added: ≥ 4 proptest scenarios in Task 26, ≥ 1 Miri pass in Task 27, ≥ 3 AsyncApp behavioral tests in Task 28. Sum ≥ 7.
13. ✅ `cargo build --workspace --all-features` green.
14. ✅ `cargo test --workspace` green; test count increases by ≥ 7.
15. ✅ `cargo clippy --workspace --all-targets -- -D warnings` zero new warnings vs Task 1 baseline.
16. ✅ `cargo fmt --all -- --check` clean.
17. ✅ `cargo doc --workspace --no-deps` zero new warnings.
18. ✅ `cargo +nightly miri test -p flui-core cell` green (or documented gap if miri not installed).
19. ✅ Design spec at `docs/superpowers/specs/2026-05-09-K07-appcell-removal-design.md` exists; "Decision log" section documents (a) candidate choice rationale, (b) any rev-2 narrowings from Task 6 adversarial review, (c) explicit Q8/Q9/Q10 decisions (`AppContext::as_mut` widening, Drop-order preservation, `Application: Clone` status).
20. ✅ Spec "Known Limitations" enumerates ≥ 5 documented scope decisions.
21. ✅ Spec "Open questions" section is empty (no deferrals to implementation — all 10 open questions resolved at spec-merge time).
21a. ✅ Post-migration `grep -rn 'BorrowMutError' crates/flui-core/src/` returns zero hits (Task 9 deletion verified).
21b. ✅ Post-migration `grep -rn 'TRACK_THREAD_BORROWS' crates/flui-core/src/` returns zero hits (Known Limitation #3 — replaced by `log::trace!`).
21c. ✅ `Application` Drop-order preserved per Q9 spec decision; `// Drop globals last` invariant at `app.rs:622-627` honored.
21d. ✅ `CHANGELOG.md` has `## [Unreleased] — K07 AppCell removal` entry above the S21 entry; one-paragraph summary + migration-guide link present.
21e. ✅ Migration guide at `docs/superpowers/migrations/K07-appcell-removal.md` exists with Before/After samples for each user-facing breaking change.
21f. ✅ `AGENTS.md` line 15 no longer lists `AppCell` among debt items; status updated per Q11 decision.
22. ✅ **(R2/R3/R4 — rev 4)** `crates/flui-core/src/app/cell.rs` `#[cfg(test)]` module asserts `AppCell: !Send + !Sync`, `AppRef<'static>: !Send + !Sync`, `AppRefMut<'static>: !Send + !Sync` via `static_assertions::assert_not_impl_any!`. `UnwindSafe` behavior matches pre-K07 baseline. `#[track_caller]` propagation verified on every `borrow*` method.
23. ✅ **(R5 — rev 4 / K15 Limitation #6)** `App::update_entity` panic-leak fixed: `prop_currently_updating_entity_restored_after_panic` test passes; `currently_updating_entity == None` after `catch_unwind` of a panicking closure. `WindowUpdateGuard` panic-safety re-verified.
24. ✅ **(R1 — rev 4)** `cargo +nightly miri test -p flui-core cell` green for the cell module's tests (PR-blocking gate per Q6 resolution).
22. ✅ `.ai-factory/RESEARCH.md` Active Summary has K07 entry.
23. ✅ ROADMAP K07 entry checked off; completion-date row added.
24. ✅ `/aif-docs` checkpoint completed.
25. ✅ All three pre-implementation adversarial reviews (Task 6) are absorbed into the spec.
26. ✅ All three post-implementation adversarial re-reviews (Tasks 42-44) completed; findings either patched, split into follow-up K-spec, or rejected with documented reason.
27. ✅ Manual smoke (~30s, `RUST_LOG=flui_core::app::cell=trace`) produces zero unexpected `warn!` events under normal navigation.
28. ✅ Web platform smoke (Task 41) passes OR documented gap with follow-up issue filed.

## Open questions (must be RESOLVED in design spec — NOT deferred to implementation)

These are decision points the spec author MUST answer in Task 5; deferring any of them to implementation is the kind of mistake K15's "revision 3" had to retroactively fix. Phase 0-K's HIGH-RISK status warrants front-loading.

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
| K15 reentrancy tests fail under new primitive | Low | High (contract regression) | Task 26 ports forward all 11 tests; Task 14 + 14a sanity-check; if any fail, escalate |
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
| L3 | 3-5 `unsafe` blocks added (was 0 in `app/`) | Project has 801 unsafe (FFI-heavy); marginal +0.5%. Confined to one file; SAFETY-commented; Miri-verified |

### Inherited from rev 1 (still valid)

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| 103 callsite migration causes merge-conflict cascade with parallel K-track | Medium | Medium | Phase 0-K critical chain is sequential by design; K07 lands before K05/K01 start. Independent K-track items (K12-K17, K20-K22, K90-K98) are merge-safe |
| K07 invalidates K05's planning assumptions | Low (sequential) | Low | K07 → K05 dependency is design intent |
| Test count inflation hides K15 regressions | Low | Low | Task 35 verifies test-count delta; failed K15 tests surface by name |
| `cargo doc` warnings from new rustdoc | Low | Low | Task 8 + 38 verify |

## Refinement record

### Revision 1 (2026-05-09 initial draft)
- 45 tasks, 9 phases.
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
- **R5 — Task 14a (NEW)**: closes K15 Limitation #6. RAII guard for `currently_updating_entity` (and verify K15's `WindowUpdateGuard` panic-safety). Property test `prop_currently_updating_entity_restored_after_panic`.
- **R2/R3/R4 — Task 8 expansion**: compile-time auto-trait tests via `static_assertions` (assert `!Send + !Sync` on `AppCell` / `AppRef` / `AppRefMut`); explicit `UnwindSafe` test; explicit `#[track_caller]` discipline checklist.
- **R7 — Q6 RESOLVED**: PR-blocking scoped Miri (`cargo +nightly miri test -p flui-core cell`), not workspace-wide. Cost bounded; the only way to audit Stacked-Borrows soundness for the cell.
- **R9/R10/R12 — Task 5 design spec adds "Future considerations" section**: K05 partial-borrow caveat; Phase III `App: Send` blocker; drop-on-panic semantics warning (preserves pre-K07 behavior, requires explicit module rustdoc).
- **Risk assessment table rewritten** — 4-tier severity model (Tier 1 PR-blocking, Tier 2 review-gating, Tier 3 future-coupling, Tier 4 honest limitations).

Net: revision 4 raises task count from 48 → **50** (+1 expanded Task 8 sub-deliverables, +1 new Task 14a). Open questions: 12 → 12 (Q6 resolved, but Q1-Q12 all now have committed answers in plan; spec authoring just transcribes them). Done criteria: +2 (R5 closure, R2/R3/R4 compile-time tests).

### Revision 5 (2026-05-09 — Task 6 adversarial review absorbed)

Three reviewers dispatched in parallel on spec rev 1: 8 BLOCKERs + 12 MAJORs + 10 MINORs. Plan changes (spec rev 2 captures all):

- **BLOCKERs absorbed (cross-confirmed by ≥2 reviewers):**
  1. `WindowUpdateGuard`/`EntityUpdateGuard` types do not exist (verified: zero matches via grep). K15 plan documented them but implementation chose inline push/pop. Spec K15 contract preservation table CORRECTED to reflect actual inline pattern.
  2. `UnwindSafe` claim INVERTED (RefCell<T>: UnwindSafe ALWAYS in std; UnsafeCell<T>: !UnwindSafe ALWAYS in std). Without manual `unsafe impl UnwindSafe for AppCell {}`, K07 regresses pre-K07 behavior. Task 8 amended.
  3. `EntityScope { app: &mut App }` does NOT compile under Candidate B's `AppRefMut` — same conflict K15 documented. Task 14a re-scoped to `catch_unwind(AssertUnwindSafe(...))` pattern.
  4. Test `borrow_mut_error_converts_to_app_borrowed` at reentrancy.rs:253 will fail compile after Task 9. Task 9 amended to MANDATORY deletion (was "if any").

- **BLOCKERs from single reviewer:**
  5. `#[track_caller]` on `Drop::drop` is no-op in Rust. Plan rev 4 required it — wrong. Task 8 amended: ONLY acquire-side methods carry `#[track_caller]`.
  6. `AsyncApp::app()` private widening cascades to 10+ public methods. Q4 amended: methods returning `Result<T>` propagate via `?`; methods returning `T` use `unwrap_or_else(|e| panic!("{}", e))`. Net: panic semantics preserved, panic Display structured.
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
  - "3 unsafe blocks" → "4 unsafe blocks" (audit table corrected: 2 projection blocks + 2 unsafe impl UnwindSafe blocks; Drop impls have NO unsafe).
  - Tree Borrows vs Stacked Borrows separated: Q6 PR-blocking is Stacked Borrows; Tree Borrows added as scheduled non-blocking gate.

- **NEW Task 14b** (paint/dispatch hot-path audit) — covers `flui-arch-reviewer` BLOCKER A1 (Key Principle #8 paint/dispatch reachability).

- **False positives identified:**
  - `migration-risk-adversary` finding #22 (Cargo.lock policy contradiction with `static_assertions` dev-dep) — `rust-api-migration-auditor` verified static_assertions v1.1.0 is ALREADY in lockfile transitively via postage v0.5.0. No policy violation.

Net: revision 5 raises task count from 50 → **51** (+1 hot-path audit). Plan task list now matches spec rev 2.

## Next steps

After K07 lands and is merged:

```
/aif-plan full K05-element-context-object
```

K05 inherits: the new `AppCell` primitive, the discharged K15 Known Limitations, the AsyncApp surface redesign, the `From<BorrowMutError>` removal, and any new `ReentryError` variants (`AppGoneAway` if Q4 chose Result-shape).
