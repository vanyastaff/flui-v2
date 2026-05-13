# SF01 — Widget + Key Trait (Framework Tier — public surface only)

**Branch:** plan authored on `happy-ellis-69db20` worktree. Implementation PR should land on a dedicated `feature/sf01-widget-key-trait` branch when execution starts.
**Created:** 2026-05-12
**Phase:** II-F Framework tier — first spec. Opens the Tier B crate (`flui-framework`) on top of the fully-landed Phase 0-K kernel cleanup (K99 → K15 → K07 → K05 → K01 → K02 → K03 → K04 all complete).
**Type:** Greenfield crate scaffold + new public trait surface in `flui-framework`, plus a `derive(Widget)` proc-macro skeleton in `flui-macros`. Engine source code in `flui-core` is **read-only** for SF01 — no `flui-core` edits are sanctioned by this plan beyond optional `pub use` decisions that are explicitly scoped here.
**Tasks:** 25 checkbox tasks across 6 phases with 5 commit checkpoints. (+2 from refinement pass: `T2.4` prelude module, `T4.5` derive-with-renamed-field positive test. Refinement also rewrote T0.1, T1.1/T1.2, T2.1, T3.1, T4.1–T4.4, T5.1 — see `/aif-improve` history for the 12-finding rationale.)

> **Design-first spec.** SF01 fixes the *public surface* of `flui-framework` for the next decade. The trait shapes (object-safe vs generic, `&self` vs `&mut self`, `impl Widget` returns vs `BoxedWidget`, key accessor placement, blanket impls) and the macro contract get baked into every Tier C crate that ever ships against this API. We freeze the contract in a design spec **before** writing the Rust code, run the pre-PR reviewer triple (`flui-arch-reviewer` + `migration-risk-adversary` + `rust-api-migration-auditor`) on the spec, and only then build the crate. SF02 (reconciliation), SF03 (BuildCx + Provider), SF04 (State<W> + StateMap), and SF05 (setState + dirty list) all depend on this surface — getting it wrong forces re-doing every Framework spec downstream.

## Settings

| Setting | Value | Rationale |
|---|---|---|
| Testing | yes | New public trait surface needs conformance tests, `trybuild` compile-fail tests for the derive macro, and integration tests proving the Framework `Key` re-exports keep `flui_core::Key` semantics intact. |
| Logging | verbose during implementation; ZERO log statements committed in `flui-framework` (no hot-path mutation lands in SF01) | Per `docs/promt.md` §3.1 and the "no allocation on rebuild hot path" invariant. SF01 ships only trait declarations + re-exports + a derive macro — there is no rebuild loop to log inside. Build-time macro diagnostics use `proc_macro::Diagnostic` / `syn::Error`, not `log` / `tracing`. |
| Docs | yes (mandatory checkpoint) | SF01 introduces a new crate at the documented Tier B slot, freezes a public trait surface, and adds a `derive(Widget)` macro. Needs design spec, rustdoc module-level docs, migration / "how to write a widget" guide, and explicit RESEARCH.md + ROADMAP.md sync at the end. |
| Roadmap linkage | linked | SF01 is the first Phase II-F spec in `.ai-factory/ROADMAP.md:120`. Closes the "Framework tier is empty" gap from the kernel audit. |

## Roadmap Linkage

**Milestone:** SF01 — Widget + Key trait. First Phase II-F spec. Creates the `flui-framework` crate, defines `Widget` and `StatefulWidget` traits with `&self` build semantics, re-exports the K02 `flui_core::Key` family at the Framework surface, and lands a `derive(Widget)` macro skeleton in `flui-macros`.

**Rationale:** `.ai-factory/ROADMAP.md:120` names SF01 as the entry point of Phase II-F. The Phase 0-K critical chain (`K99 → K15 → K07 → K05 → K01 → K02 → K03 → K04`) is fully landed as of 2026-05-12 — all SF01 dependencies (K02 identity/Key, K03 Render/Build separation, K05 Element ctx-object) are satisfied. SF02/SF03/SF04/SF05 are explicitly gated on SF01. Tier C population (`flui-widgets`, `flui-material`, etc.) is gated on SF05. SF01 is the single un-blockable next step.

**SF01 is explicitly NOT:** reconciliation (SF02), `BuildCx` ergonomics or `inherit<T>` / `read<T>` (SF03), `State<W>` / `StateMap` / `did_update_widget` / `dispose` (SF04), `setState` or dirty propagation (SF05), `InheritedWidget` analogs (SF06), Widget→Element compilation/mounting adapter (SF07), async widgets (SF08), or a widget catalogue (Tier C). The spec freezes only the **trait shapes** plus the minimum scaffolding that makes them usable in isolation. Mounting is stubbed via a transitional `BuildElement` bridge so the trait surface compiles and round-trips through `cargo check`, but real Widget→Element compilation lands in SF07.

## Research Context

Source: `.ai-factory/RESEARCH.md` Active Summary, `.ai-factory/ROADMAP.md`, `.ai-factory/ARCHITECTURE.md` §"Framework Tier Internals" + §"Code Examples", K02 design spec (`docs/superpowers/specs/2026-05-11-K02-element-identity-key-design.md`), K03 design spec (`docs/superpowers/specs/2026-05-11-K03-render-build-separation-design.md`), and current `crates/flui-core/src/element/identity.rs`, `crates/flui-core/src/build.rs`.

- **K02 is the identity substrate.** `flui_core::Key` is `Key(KeyKind { Local(panic::Location) | Value(ElementId) | Global(GlobalKey) })`. Constructors: `Key::local()` (`#[track_caller]`), `Key::value(impl Into<ValueKey>)`, `Key::global(impl Into<GlobalKey>)`. `ValueKey` accepts `usize`, `i32` (fallible), `SharedString`, `String`. `GlobalKey` is opaque. `From<Key> for ElementId` converts to engine path segments. SF01 wraps this — does NOT redefine it. The Framework `Key` is a thin re-export plus documentation about widget-author intent.
- **K03 is the render/build boundary.** `ElementBuilder` + `ElementBuildCx` + `BuildElement` + `build_element(...)` in `crates/flui-core/src/build.rs` provide an immutable engine recipe substrate built from `&self`. The Framework `Widget` trait can lower to `BuildElement` at the engine boundary; SF07 will define the full adapter. SF01 may only ship the **trait shape** that makes this lowering possible — no live mounting adapter.
- **K05 is the lifecycle-context substrate.** `LayoutCx` / `PrepaintCx` / `PaintCx` exist for engine element lifecycle. Framework's `BuildCx` (SF03) is a separate context — SF01 does NOT introduce `BuildCx`. The `Widget::build(&self) -> impl Widget` signature in SF01 takes **no context** (placeholder); SF03 will widen it to `build(&self, cx: &mut BuildCx<'_>) -> impl Widget`. Forward compatibility: any SF01 `Widget` impl will only need to add the `cx: &mut BuildCx<'_>` parameter when SF03 lands, with `#[allow(unused_variables)]` until then or via a default method.
- **K01 is the per-Window Provider substrate.** Framework's `Provider` ergonomics (SF03) wrap it. SF01 does **not** introduce any Provider API — but the design spec must verify the Widget trait shape leaves room for `BuildCx::read<T>()` / `BuildCx::inherit<T>()` to be added without trait migration cost.
- **K04 is the frame contract.** Frame phases include a reserved `Build` phase (`PreFrame → AnimationTick → Build (reserved no-op for SF05) → Layout → Prepaint → Paint → PostFrame`). SF01 must keep the trait shape compatible with `Build` being the eventual phase where `Widget::build` runs — no `&mut App` or `&mut Window` in `build` signatures (those belong in SF03's BuildCx).
- **Architecture mandate.** `.ai-factory/ARCHITECTURE.md` §"Code Examples" already sketches the target widget shape:
  ```rust
  #[derive(Widget)]
  pub struct Counter {
      initial: i32,
      #[widget(key)] key: Option<Key>,
  }
  impl StatefulWidget for Counter {
      type State = CounterState;
      fn create_state(&self) -> CounterState { … }
  }
  ```
  SF01 must produce a trait surface that makes this exact example compile. The `WidgetState<W>` trait body (with `build` / `did_update_widget` / `dispose`) is SF04 territory and is **not** finalized here, but the `StatefulWidget::State` associated type must exist so the example shape is reachable.
- **"2 structures + 1 cache" invariant.** Per ARCHITECTURE.md §"Framework tier as '2 structures + 1 cache'", Widget = immutable config, Element = existing flui-core runtime, State = flat `HashMap<ElementId, Box<dyn State>>`. SF01 introduces only the Widget half of the first structure. Reconciliation, mounting, and state storage are downstream.
- **Hard-fork posture.** flui-framework is a NEW crate, not a port of upstream `gpui` types. No upstream API to preserve. Trait shapes are chosen for flui-v2's goals (Flutter DX, Rust 2024 idioms, zero-alloc rebuild hot path), not for upstream compatibility.
- **`derive_more` 0.99 → 2.x** is K92 territory; SF01's `derive(Widget)` uses `syn` 2.x / `quote` 1.x / `proc-macro2` 1.x directly (consistent with current `flui-macros/Cargo.toml`). No `derive_more` dependency added by SF01.
- **Workspace MSRV** is 1.95 / edition 2024 (K99). SF01 may use stable AFIT / RPITIT (`fn build(&self) -> impl IntoWidget` per the FROZEN spec — Option B chosen at T0.3) — this is the explicit K99 unlock and a hard requirement for "no `Box<dyn>` on the rebuild hot path".

## Current State

| Area | Current shape | SF01 concern / decision |
|---|---|---|
| `crates/flui-framework/` | Does not exist | **Create** the crate at this path with Tier B Cargo manifest (depends on `flui-core` only). Add to workspace members in root `Cargo.toml`. |
| `flui_core::Key` family | `Key`, `ValueKey`, `GlobalKey`, `LocalElementId`, `ElementId` in `crates/flui-core/src/element/identity.rs`, re-exported from `flui-core` | **Re-export** `pub use flui_core::{Key, ValueKey, GlobalKey};` at the Framework public surface with widget-author-facing rustdoc. Do NOT redefine. Do NOT re-export `LocalElementId` / `ElementId` at the Framework surface — those are Engine path-segment types; Framework users speak `Key`, not raw IDs. |
| `flui_core::build` (K03) | `ElementBuilder` + `ElementBuildCx` + `BuildElement` + `build_element(...)` | **Reference** only. SF01 ships a stub `impl BuildElement for AnyWidget` or equivalent transitional bridge so SF01 compiles end-to-end, but the real mounting/lowering adapter is SF07. The design spec must enumerate the precise shape of this transitional bridge so reviewers can confirm it does not pin the SF07 design. |
| `Render` / `RenderOnce` / `Component<C: RenderOnce>` | Engine substrate; `Component<C>` is `#[doc(hidden)]` one-shot RenderOnce shim | **Distinct from Widget.** SF01 rustdoc must explicitly call out the difference (Engine substrate vs Framework API) to prevent confusion. Anti-pattern table in module docs. |
| `flui-macros` | `proc-macro = true`, uses `syn` 2.x / `quote` / `proc-macro2`, existing derives (Render, IntoElement, …) | **Add** `Widget` derive. New file `crates/flui-macros/src/widget.rs` plus registration in `crates/flui-macros/src/flui_macros.rs`. No new external dependencies. |
| `flui-widgets`, `flui-material`, `flui-navigator`, etc. (Tier C) | Currently depend directly on `flui-core` | **Untouched by SF01.** No Tier C crate migrates to depend on `flui-framework` in SF01. Migration is gated on SF02–SF05 (i.e., when there is real Framework value to consume — reconciliation, state, setState). SF01 ships in isolation and is verified by `cargo check --workspace` passing and the new crate's own tests, not by Tier C use. |
| Examples | Currently use `Render` directly | **Add one** `examples/widget_surface_demo/` micro-example demonstrating a `Widget` impl + `derive(Widget)` usage at the trait-surface level. Not a runnable GUI app — a `cargo check`-only example that proves the surface is usable by an out-of-tree crate. |
| Reviewer agents | `flui-arch-reviewer`, `migration-risk-adversary`, `rust-api-migration-auditor`, `wgpu-gpu-reviewer` configured in `.claude/agents/` | **Pre-PR triple launch** mandated by user-memory feedback for SF-track PRs: `flui-arch-reviewer` + `migration-risk-adversary` + `rust-api-migration-auditor` in one parallel batch, both at design-spec freeze (T2.x) and at code-complete (T6.x). `wgpu-gpu-reviewer` is NOT in scope (no GPU code in SF01). |

## Tasks

### Phase 0 — Design spec (text-only, no Rust source touched)

Per user-memory feedback "Keep docs work separate from code work — ADR sessions stay text-only; do not also patch Rust source in the same pass." Phase 0 produces only Markdown.

- [x] **T0.1 — Draft the SF01 design spec.** Write `docs/superpowers/specs/2026-05-12-SF01-widget-key-trait-design.md`. Must contain:
  - Scope statement (what SF01 IS / IS NOT) — mirror this plan's §"SF01 is explicitly NOT" list verbatim.
  - **Trait surface freeze:** exact signatures of `Widget` and `StatefulWidget`, blanket impls (if any), super-trait bounds, where-clauses, `#[non_exhaustive]` decisions, object-safety analysis. Decide explicitly: `Widget` is **not** object-safe (its `build` method returns an opaque type per K99 AFIT unlock) — object-safe erasure lives in SF07 via `BoxedWidget` newtype.
  - **DESIGN RISK — `Widget::build` return type.** The codebase's strong convention for `build`/`render`-like methods is to return `impl IntoX`, not `impl Self::Trait`: `Render::render -> impl IntoElement` (`crates/flui-core/src/element.rs:475`), `RenderOnce::render -> impl IntoElement` (`element.rs:494`), `ElementBuilder::build -> impl IntoElement` (`crates/flui-core/src/build.rs:103`). The spec MUST decide explicitly between two options and justify the pick:
    - **Option A:** `fn build(&self) -> impl Widget` — recursive trait shape; clean Flutter analogue; but a child can only be a *single* widget type per `build` call (Rust opaque types are concrete).
    - **Option B:** `fn build(&self) -> impl IntoWidget` — mirrors `Render`/`RenderOnce`/`ElementBuilder` precedent; introduces a sibling `IntoWidget` trait with a blanket `impl<W: Widget> IntoWidget for W`; matches engine conventions; better long-term ergonomics for reconciliation.
    - Recommendation: **Option B** unless reviewer triple in T0.2 produces a load-bearing reason for Option A. Document the decision in a dedicated subsection.
  - **Evolution to SF03 (breaking trait change — NOT forward-compatible).** Show the SF03 evolution: `fn build(&self) -> impl IntoWidget` → `fn build(&self, cx: &mut BuildCx<'_>) -> impl IntoWidget`. Per the FROZEN spec §"Evolution to SF03", this is a breaking method change — no default-method trick can inject a new required parameter. Every SF01 Widget impl needs a one-line edit (`, _cx: &mut BuildCx<'_>`) on SF03 day. Document this as semver-major for `flui-framework`.
  - **DESIGN RISK — reconciliation traversal under non-object-safe `Widget`.** Because `Widget` is not `dyn`-compatible, SF02 reconciliation cannot use `dyn Widget` to walk a heterogeneous tree of children. Surface for reviewer attention: SF02 will need either (a) enum-based erasure of the visible child set, (b) monomorphized generic recursion, or (c) the SF07 `BoxedWidget` newtype as the erasure point. Spec records the constraint, not the solution.
  - **K15 / K04 forward-compat statement.** SF01 traits do NOT enter the K15 re-entrancy contract because no Widget code runs in this spec. SF02 (reconciliation), SF03 (BuildCx callbacks), and SF05 (setState propagation) each inherit the K15 contract as they add runtime behavior. SF01 traits also do not interact with K04 frame phases — Widget code will run inside the `Build` phase (reserved no-op in K04) once SF03/SF05 land.
  - **DECISION — trybuild vs compile_fail doctests for the derive macro test surface.** Project precedent uses compile_fail doctests inside macro docstrings (`crates/flui-macros/src/flui_macros.rs:48-56` derive_app_context, lines 67-89 derive_visual_context). `trybuild` is the industry standard, gives `.stderr` snapshot tests, but introduces a new dev-dep. Spec MUST pick one — current plan defaults to `trybuild` for snapshot-based diagnostics; record the deviation from precedent and the justification (or pivot to compile_fail doctests). Reviewer triple should examine both options.
  - **`StatefulWidget` shape.** `trait StatefulWidget: Widget { type State: WidgetState<Self> where Self: Sized; fn create_state(&self) -> Self::State; }`. The `WidgetState<W>` trait body is SF04 territory; SF01 publishes only a forward-declared marker `pub trait WidgetState<W: Widget>: 'static {}` with `Self: Sized` and zero required methods so the SF04 spec can fill it in additively. Document this explicitly.
  - **Key surface.** `pub use flui_core::{Key, ValueKey, GlobalKey};` with module-level rustdoc explaining widget-author intent (Local for source-location identity, Value for reorder-stable lists, Global for cross-tree references — *but* cross-tree reachability is SF02 territory; SF01 just publishes the type).
  - **`derive(Widget)` macro contract.** Field attributes: `#[widget(key)]` marks an optional `Key` field that becomes the widget's identity, default identity is `Key::local()` (source-location), error if more than one `#[widget(key)]` field. Generated code: `impl Widget for X` with a `fn key(&self) -> Option<&Key>` accessor. No `Clone` / `Debug` requirements on widget structs forced by the derive.
  - **Anti-pattern table.** Explicit "Widget vs `Render` vs `RenderOnce` vs `Component<C>`" decision table with one-line guidance per axis. Prevents Tier C crates from accidentally implementing `Render` when they mean `Widget`.
  - **Transitional `BuildElement` bridge.** Spell out the exact shape of the `impl BuildElement for ???` (or equivalent) bridge that lets SF01 `Widget` impls compile end-to-end without SF07. Two options: (a) leave it un-implemented and prove the surface compiles in isolation via stand-alone tests, (b) ship a `#[doc(hidden)]` `widget_to_element_stub<W: Widget>()` that panics at runtime with a documented "SF07 not yet landed" message. **Decide in the spec — do not leave undecided.**
  - **Out-of-scope check.** No `BuildCx`, no `State<W>` body, no `setState`, no reconciliation, no mounting. Explicit one-line denial of each.
  - **Public surface enumeration.** Bullet list of every `pub` item that lands in `flui-framework` in SF01. Reviewer will diff this against the implementation.
  - **Migration cost analysis.** No SF01 migration cost — there are zero callers today. Document this and note: SF02–SF05 will each add a migration step for SF01 widgets (e.g., SF03 adds `cx` parameter, SF04 fills `WidgetState<W>` body).

  **File:** `docs/superpowers/specs/2026-05-12-SF01-widget-key-trait-design.md`. **Logging:** none — design doc. **Validation:** spec compiles in Markdown, contains all sections above.

- [x] **T0.2 — Pre-PR reviewer triple launch on the design spec (parallel).** **DONE 2026-05-12** — three reviewers ran in parallel; 13 blockers, 18 concerns, 14 future-proofing items captured in spec §"Reviewer Notes (T0.2 — 2026-05-12)". 8 blockers fixed in-spec, 5 plan-side fixes applied in T0.3 wrap. Two future-proofing items rejected as premature. Per user-memory feedback "Pre-PR review-agent triple launch (parallel) — for K-track / SF-track PRs, dispatch flui-arch-reviewer + migration-risk-adversary + rust-api-migration-auditor in one message; they routinely find real bugs other checks miss."
  - Launch all three in **a single message** with three parallel Agent tool calls.
  - `flui-arch-reviewer`: verify SF01 surface respects Tier B / Tier A boundary, no Framework concerns leaking into Engine, no Engine internals leaking into Framework public surface.
  - `migration-risk-adversary`: hunt for silent functionality loss / API regressions vs current `flui-core::Key` consumers (none expected, since `flui-core::Key` re-exports stay), and the forward-compat SF03 promotion path.
  - `rust-api-migration-auditor`: semver impact analysis of new public surface, trait object-safety, feature-flag implications, workspace dependency direction, blanket impl risks.
  - Each agent works from the design spec alone — no code yet.
  - Capture findings in a `## Reviewer Notes` appendix in the design spec.

  **File:** updates `docs/superpowers/specs/2026-05-12-SF01-widget-key-trait-design.md` (adds Reviewer Notes section). **Logging:** none. **Validation:** all three reviewer summaries captured; every raised concern has a documented response (accept / reject / defer to SF##).

- [x] **T0.3 — Address reviewer findings; freeze the contract.** **DONE 2026-05-12** — spec sections "Trait Surface Freeze", "IntoWidget" (added object-safety seal + coherence note), "WidgetState<W>" (removed `#[doc(hidden)]`, added stability rustdoc), "prelude" (6 items, excludes WidgetState), "Evolution to SF03 (Breaking — Not Forward-Compatible)", "derive(Widget) Macro Contract" (helper-narrow-scope clarification + type-validation strategy), "Additional Frozen Decisions" (AFIT lifetime capture, widget_surface_demo CI safety, ValueKey leak acceptance, Tier C ElementId gap deferral, cargo-semver-checks R2 deferral, trybuild re-bless procedure, reserved [features] block, K91 cross-track T6.3 add-on), and "Reviewer Notes" appendix are all FROZEN at 2026-05-12. Plan-side fixes applied: T2.4 prelude content, T2.3 i32 fallible roundtrip, T1.1 `flui-macros` dep timing note, T5.1 / T6.3 example-path consistency, T6.3 K91 cross-track + ARCHITECTURE.md code-example update. Implementation (Phase 1+) MUST match the FROZEN spec exactly.

**Commit 1 (Phase 0):** `docs(sf01): freeze Widget + Key trait design spec` — commits only `docs/superpowers/specs/2026-05-12-SF01-widget-key-trait-design.md`.

### Phase 1 — Crate scaffolding

- [x] **T1.1 — Create `crates/flui-framework/` skeleton.** New files:
  - `crates/flui-framework/Cargo.toml` — `name = "flui-framework"`, explicit `version = "0.1.0"` (workspace does NOT inherit version — checked against `crates/flui-macros/Cargo.toml` precedent), workspace-inherited `edition` / `rust-version` / `authors` / `license` / `repository`, `description = "Framework tier for flui — Widget / Key / State / BuildCx / reconciliation"`. Dependencies: `flui-core = { path = "../flui-core" }` ONLY (no `flui-macros` yet — derive is consumed by users, not by `flui-framework` itself; macros dep lands in T4.4 alongside the re-export). `[lints] workspace = true`. **NO** `flui-widgets`, `flui-material`, `flui-platform` deps (would be tier violations or pointless).
  - `crates/flui-framework/src/lib.rs` — crate-level rustdoc explaining Tier B's role, the "2 structures + 1 cache" model, the relationship to `flui-core` Engine substrate, and the Tier A / B / C dependency direction. `#![deny(missing_docs)]` lint at crate root (workspace does not currently deny `missing_docs` — Framework tier is the Tier C-facing API, stricter than engine is intentional). Empty module declarations for `widget`, `key`, `prelude`. No `pub use crate::*` — explicit re-exports only (per ARCHITECTURE.md principle 6).

  **Files:** `crates/flui-framework/Cargo.toml`, `crates/flui-framework/src/lib.rs`. **Logging:** none (no runtime code yet). **Validation:** `cargo check -p flui-framework` succeeds.

- [x] **T1.2 — Register `flui-framework` in workspace.** Edit root `Cargo.toml`:
  - Add `"crates/flui-framework",` to `[workspace] members` immediately after `"crates/flui-macros",` and before `"crates/flui-widgets",` — this keeps the visual ordering Tier A (`flui-core`, `flui-platform`, `flui-macros`) → Tier B (`flui-framework`) → Tier C (`flui-widgets`, `flui-navigator`, `flui-a11y`, `flui-theme`, `flui-material`) in the members list.
  - Do NOT add a workspace dependency entry (no other crate depends on `flui-framework` in SF01).

  **File:** root `Cargo.toml`. **Logging:** none. **Validation:** `cargo check --workspace` succeeds; `cargo metadata --format-version 1 | jq '.workspace_members'` lists `flui-framework`.

- [x] **T1.3 — Verify Tier-A-only dependency direction at the metadata level.** Run `cargo tree -p flui-framework --depth 1` and confirm only `flui-core` (plus its transitive deps) appears. Document the output in a new `.ai-factory/qa/SF01-tier-isolation.md` note so reviewers can re-run the same check.

  **File:** `.ai-factory/qa/SF01-tier-isolation.md`. **Logging:** none. **Validation:** `cargo tree` output captured; no `flui-widgets` / `flui-material` / `flui-platform` deps present.

**Commit 2 (Phase 1):** `feat(framework): introduce flui-framework crate skeleton (SF01 phase 1)` — commits `crates/flui-framework/{Cargo.toml,src/lib.rs}`, root `Cargo.toml`, `.ai-factory/qa/SF01-tier-isolation.md`.

### Phase 2 — Key substrate re-exports

- [x] **T2.1 — Add `crates/flui-framework/src/key.rs`.** Contents:
  - Module-level rustdoc explaining widget-author intent: Local = source-location identity (`#[track_caller]`, parent-scoped, sibling-occurrence-disambiguated, NOT reorder-stable), Value = reorder-stable list-item identity, Global = cross-tree references (cross-tree reachability reaches the spec slate only at SF02/SF05 — SF01 just publishes the type).
  - `pub use flui_core::{Key, ValueKey, GlobalKey};`
  - **Do NOT** re-export `flui_core::ElementId` / `LocalElementId` at the Framework public surface — they are engine path-segment types and Framework users speak `Key`, not raw IDs (per RESEARCH.md decision and Current State table).
  - **Do NOT** wrap `Key` in a newtype. The K02 spec explicitly designed `Key` as the cross-tier identity intent type; wrapping it would force conversion at every Tier C → Tier B → Tier A boundary.
  - Add one-line examples (rustdoc `///` blocks) for each of the three constructors, each example `cargo check`-able via doctests.
  - **K91 dependency.** Today `flui_core::Key` is visible at the crate root via the `pub use element::*;` glob at `crates/flui-core/src/lib.rs:154`. K91 (29 globs → explicit re-exports) will replace that glob — when K91 lands, the new explicit re-export list MUST keep `Key`, `ValueKey`, `GlobalKey` at the `flui_core` crate root. The SF01 design spec records this as a binding requirement for K91 so the cross-track contract is documented in both plans.

  **File:** `crates/flui-framework/src/key.rs`. **Logging:** none. **Validation:** `cargo check -p flui-framework` succeeds; rustdoc `cargo doc -p flui-framework --no-deps` renders examples; doctests pass under `cargo test -p flui-framework --doc`.

- [x] **T2.2 — Wire `key` module into `lib.rs`.** Add `pub mod key;` and `pub use crate::key::{Key, ValueKey, GlobalKey};` to `crates/flui-framework/src/lib.rs`. Explicit re-exports — no `pub use crate::key::*;`.

  **File:** `crates/flui-framework/src/lib.rs`. **Logging:** none. **Validation:** `cargo check -p flui-framework` succeeds; `cargo doc` shows `flui_framework::Key`, `flui_framework::ValueKey`, `flui_framework::GlobalKey` at the crate root.

- [x] **T2.3 — Key roundtrip integration test.** Add `crates/flui-framework/tests/key_roundtrip.rs`:
  - Prove that `flui_framework::Key::local()` produces the same `ElementId` (via `From<Key> for ElementId`) as `flui_core::Key::local()` would at the matching source location.
  - Prove `Key::value(42usize)` and `Key::value(String::from("x"))` round-trip identically via the Framework re-export.
  - **Prove `ValueKey::try_from(42_i32)` succeeds and round-trips** via the Framework re-export (the fallible `i32` path was added in T0.3 wrap-up — reviewer T0.2 flagged it as an SR2 silent-regression vector if the K02 `TryFrom<i32> for ValueKey` impl ever regressed).
  - Prove `Key::global(GlobalKey::new(...))` (or whatever K02's public constructor exposes) round-trips.
  - This is a *re-export semantics* test, not a Key behavior test (K02 already has those). Goal: prove the Framework surface is a thin alias, not a divergent type.

  **File:** `crates/flui-framework/tests/key_roundtrip.rs`. **Logging:** none. **Validation:** `cargo test -p flui-framework --test key_roundtrip` passes.

- [x] **T2.4 — Minimal explicit `prelude` module.** Add `crates/flui-framework/src/prelude.rs`:
  - `pub use crate::{Empty, GlobalKey, IntoWidget, Key, StatefulWidget, ValueKey, Widget};` — **7 items** (Amendment 1, 2026-05-12, added `Empty` per the sealed null-widget addition). Per the FROZEN design spec, `WidgetState` is intentionally OMITTED from the prelude (stability rationale: unstable until SF04). `IntoWidget` IS included (needed for `impl IntoWidget` return type in user code). `Empty` IS included for `#[derive(Widget)]`-free widget impls that need to write `fn build(&self) -> impl IntoWidget { Empty }`. Reviewer T0.2 fixed an earlier inconsistency between this plan and the spec — spec wins.
  - Module-level rustdoc explaining that the prelude is an **opt-in convenience** for Tier C / app authors; consumers may always import items individually from the crate root (including `WidgetState`). Explicit-only, **no `pub use crate::*`** (per ARCHITECTURE.md principle 6).
  - Wire `pub mod prelude;` into `crates/flui-framework/src/lib.rs` — the `prelude` module itself is `pub`, not re-exported at the crate root (consumers write `use flui_framework::prelude::*;` exactly once when they want it).
  - Rationale for landing prelude in SF01, not later: prelude IS public-surface API. Adding it post-hoc invites consumers to write their own ad-hoc preludes and creates documentation drift. K94 already validates the precedent at the engine level.

  **Files:** `crates/flui-framework/src/prelude.rs`, `crates/flui-framework/src/lib.rs`. **Logging:** none. **Validation:** `cargo check -p flui-framework` succeeds; rustdoc shows `flui_framework::prelude` with all seven items listed (Amendment 1).

### Phase 3 — Widget + StatefulWidget trait surface

- [x] **T3.1 — Add `crates/flui-framework/src/widget.rs` with the `Widget` trait.** Per the FROZEN contract from T0.3 (which picks the `Widget::build` return type during reviewer triple T0.2):
  - `pub trait Widget: 'static + Sized` — the `Sized` bound matches "Widget is an immutable owned config struct, cheap to clone, recreated each rebuild" (ARCHITECTURE.md §"Framework Tier Internals"). Widgets are NEVER stored as `dyn Widget` directly — erasure goes through SF07's `BoxedWidget` newtype.
  - `fn key(&self) -> Option<&Key> { None }` — default impl returns `None` so widgets that don't carry a Key field still implement the trait with zero boilerplate. Override produced by `derive(Widget)` when a `#[widget(key)]` field is present. Note: the trait method name `key` collides with the conventional struct field name `key`; this is harmless in Rust (method dispatch vs `self.field` access never collide) and matches Flutter's `Widget.key` precedent, but the rustdoc must explicitly call out the dual naming so users don't try to refactor it away.
  - `fn build(&self) -> impl <BUILD_RETURN_TYPE>` — `<BUILD_RETURN_TYPE>` is `Widget` or `IntoWidget` per T0.1 decision (see DESIGN RISK in T0.1; recommendation: `IntoWidget` for consistency with `Render`/`RenderOnce`/`ElementBuilder` engine convention). **No `cx` parameter** (SF03 adds it). The opaque return uses the K99 AFIT/RPITIT unlock. Default impl: panics with `unimplemented!("Widget::build must be implemented by SF02+ widgets; SF01 publishes only the trait surface — see SF01 design spec")`. Justification: every SF01 widget is a leaf widget for trait-surface-test purposes; real build bodies require SF02 reconciliation + SF03 BuildCx + SF04 State, none of which exist yet. The default-panic stub is documented as transitional.
  - Trait-level rustdoc must include the **engine anti-pattern quote** verbatim from `crates/flui-core/src/element.rs:467-472`: "intentionally distinct from immutable element recipes such as [crate::ElementBuilder] and from the future Framework-tier `Widget` API". Cross-link to `flui_core::Render`, `flui_core::RenderOnce`, `flui_core::ElementBuilder` so rustdoc renders the engine/framework boundary visibly. Also include: "2 structures + 1 cache" recap, immutability contract (`&self` build, no interior mutability in Widget structs), the K99 AFIT unlock.

  **File:** `crates/flui-framework/src/widget.rs`. **Logging:** none. **Validation:** `cargo check -p flui-framework` succeeds; `cargo doc` renders the trait with its full doc block.

- [x] **T3.2 — Add `StatefulWidget` trait in the same file.** Per the FROZEN contract:
  - `pub trait StatefulWidget: Widget` — super-trait `Widget` so every `StatefulWidget` is automatically a `Widget`.
  - `type State: WidgetState<Self>` — associated type bound to the `WidgetState<W>` forward-declared marker trait.
  - `fn create_state(&self) -> Self::State` — required method.
  - No default impls. Object safety not relevant (trait carries `Self` in associated-type bound).

- [x] **T3.3 — Forward-declare `WidgetState<W>` marker trait.** In `crates/flui-framework/src/widget.rs`:
  - `pub trait WidgetState<W: Widget>: 'static { /* SF04 fills body */ }` with rustdoc explaining: "SF01 publishes the marker only — `build` / `did_update_widget` / `dispose` arrive in SF04. Do not implement this trait yet outside of trait-surface tests; the contract is unstable until SF04 lands."
  - Add `#[doc(hidden)]` if the spec's reviewer triple flags it as an end-user footgun. (Decision deferred to T0.2 review — record outcome in spec.)

- [x] **T3.4 — Wire `widget` module into `lib.rs`.** Add `pub mod widget;` and explicit re-exports `pub use crate::widget::{Widget, StatefulWidget, WidgetState};` to `crates/flui-framework/src/lib.rs`. Verify the T2.4 `prelude` module picks up these re-exports correctly.

  **File:** `crates/flui-framework/src/lib.rs`. **Logging:** none. **Validation:** `cargo check -p flui-framework` succeeds; the four public items (`Widget`, `StatefulWidget`, `WidgetState`, `Key` etc.) are visible at crate root in rustdoc.

- [x] **T3.5 — Trait conformance tests.** Add `crates/flui-framework/tests/trait_surface.rs`:
  - Define a trivial `struct Leaf;` and `impl Widget for Leaf {}` using only default methods (no `build`, no `key`). Prove it compiles.
  - Define a `struct Container { key: Option<Key> }` and manually `impl Widget for Container { fn key(&self) -> Option<&Key> { self.key.as_ref() } }`. Prove it compiles and `widget.key()` returns the expected `Some(&Key)`.
  - Define a `struct Counter { initial: i32 }` and `impl StatefulWidget for Counter { type State = CounterState; fn create_state(&self) -> CounterState { CounterState { value: self.initial } } }` plus `struct CounterState { value: i32 }; impl WidgetState<Counter> for CounterState {}`. Prove the ARCHITECTURE.md §"Code Examples" target shape compiles end-to-end at the SF01 surface (modulo the `build` body, which is SF04 territory).
  - Each test is a compile-only test; no `assert!` needed beyond construction. Add one `#[test] fn key_dispatch_returns_some_when_provided() { … }` smoke test for `Widget::key()`.

  **File:** `crates/flui-framework/tests/trait_surface.rs`. **Logging:** none. **Validation:** `cargo test -p flui-framework --test trait_surface` passes.

**Commit 3 (Phase 2 + 3):** `feat(framework): Key re-exports + Widget/StatefulWidget trait surface (SF01 phases 2-3)` — commits `crates/flui-framework/src/{key.rs,widget.rs,lib.rs}` and `crates/flui-framework/tests/{key_roundtrip.rs,trait_surface.rs}`.

### Phase 4 — `derive(Widget)` macro skeleton

- [x] **T4.1 — Add `crates/flui-macros/src/derive_widget.rs` proc-macro implementation.**
  - Module name `derive_widget` follows the project convention used by every existing derive in `flui-macros` (`derive_action`, `derive_render`, `derive_into_element`, `derive_app_context`, `derive_visual_context` — see `crates/flui-macros/src/flui_macros.rs:1-9`). Do NOT pick a different name.
  - Reuse the existing `get_simple_attribute_field(ast, "widget")` helper at `crates/flui-macros/src/flui_macros.rs:289-299` for finding the `#[widget(key)]` field. The helper already enforces struct-only input (returns `None` for enum/union) and field-attribute matching by ident.
  - `#[proc_macro_derive(Widget, attributes(widget))]` entry point registered in `crates/flui-macros/src/flui_macros.rs` calling into `derive_widget::derive_widget(input)`.
  - Parse the struct via `syn::DeriveInput`. Enforce: `struct` only (reject `enum` / `union` with a `compile_error!` — note that `get_simple_attribute_field` silently returns `None` for non-struct inputs, so the derive function must also explicit-check `data` shape upfront to produce a clean error message).
  - Iterate fields. At most one field may carry `#[widget(key)]`; that field must have type `Option<Key>` (validate via `syn::Type` matching against the path `Option<Key>` with the allowlist `Option<flui_framework::Key>`, `Option<Key>`). Multiple `#[widget(key)]` fields → `compile_error!`. `#[widget(key)]` on a non-`Option<Key>` field → `compile_error!` with span pointing at the field. The attribute is **field-name-agnostic** — the user may name the field `key`, `id`, `widget_key`, or anything else; the attribute is what marks identity, not the field name.
  - Generate `impl ::flui_framework::Widget for #StructName { fn key(&self) -> Option<&::flui_framework::Key> { self.<keyfield>.as_ref() } }` — the generated `fn key` returns `self.<keyfield>.as_ref()` if a `#[widget(key)]` field exists, otherwise the derive omits the `fn key` override (relying on `Widget`'s default `None`).
  - Use `::flui_framework::` absolute paths in generated code so the macro works regardless of how the user imports.
  - Handle generic types: `impl<T> Widget for Foo<T> where T: 'static` etc. — derive `where` clause must thread through generics correctly. (Reviewer agent will hunt for missing-bound bugs in T0.2 and T6.2.)

  **File:** `crates/flui-macros/src/derive_widget.rs`. **Logging:** none (proc-macro diagnostics via `syn::Error::to_compile_error`). **Validation:** `cargo check -p flui-macros` succeeds.

- [x] **T4.2 — Register the derive in `crates/flui-macros/src/flui_macros.rs`.** Add `mod derive_widget;` (placed next to the existing `mod derive_*;` lines for ordering consistency) and the `#[proc_macro_derive(Widget, attributes(widget))]` entry function (placed near `derive_render` for thematic grouping). Verify the macro is picked up by `cargo doc -p flui-macros`.

  **File:** `crates/flui-macros/src/flui_macros.rs`. **Logging:** none. **Validation:** `cargo check -p flui-macros` succeeds.

- [x] **T4.3 — Add `trybuild` compile-pass/fail tests for the derive.** **Test crate placement:** tests live in `crates/flui-framework/tests/`, NOT `crates/flui-macros/tests/`. Rationale: `flui-macros` is the producer of the derive but `flui-framework` is the consumer (it re-exports the derive in T4.4, and ALL real-world callers use the framework re-export). Placing trybuild tests on the consumer side keeps the dependency graph linear (`flui-framework → flui-macros`) and avoids the dev-dep circular smell that would arise from `flui-macros [dev-dependencies] flui-framework = { path = … }`.
  - Add `trybuild = "1"` to `crates/flui-framework/[dev-dependencies]`. Acknowledge in the design spec that this introduces a new dev-dep that the project did not previously use — the alternative (compile_fail doctests per the existing precedent at `flui_macros.rs:48-89`) was considered and rejected in T0.1's "DECISION — trybuild vs compile_fail doctests" subsection.
  - Add `crates/flui-framework/tests/widget_derive_compile.rs` runner and test cases under `crates/flui-framework/tests/widget_derive/`:
    - `pass_simple.rs` — `#[derive(Widget)] struct Leaf;`. Compiles.
    - `pass_with_key.rs` — `#[derive(Widget)] struct WithKey { #[widget(key)] key: Option<Key> }`. Compiles; `WithKey { key: Some(Key::local()) }.key().is_some()`.
    - `pass_generic.rs` — `#[derive(Widget)] struct Generic<T: 'static> { value: T }`. Compiles.
    - `fail_enum.rs` — `#[derive(Widget)] enum E { A }`. Fails with "Widget derive only supports structs".
    - `fail_multiple_keys.rs` — two `#[widget(key)]` fields. Fails with "only one #[widget(key)] field allowed".
    - `fail_wrong_key_type.rs` — `#[widget(key)] key: String`. Fails with "#[widget(key)] field must be `Option<Key>`".

  **Files:** `crates/flui-framework/Cargo.toml`, `crates/flui-framework/tests/widget_derive_compile.rs`, `crates/flui-framework/tests/widget_derive/*.rs`. **Logging:** none. **Validation:** `cargo test -p flui-framework --test widget_derive_compile` passes; all three fail-case `.stderr` snapshots match.

- [x] **T4.4 — Re-export the macro from `flui-framework` for ergonomics.** Add `flui-macros = { path = "../flui-macros" }` to `crates/flui-framework/[dependencies]` (regular dep, not dev — the re-export is part of the public surface). Re-export `pub use flui_macros::Widget;` from `crates/flui-framework/src/lib.rs`. Now Tier C users write `use flui_framework::Widget;` (trait) plus `#[derive(Widget)]` from the same path. **Decision check:** does the trait-name collision with the derive name cause `cargo doc` or rustc to emit ambiguity warnings? Rust normally permits this (trait and macro live in different namespaces) but rustdoc rendering may need explicit disambiguation. If yes, capture the outcome in the design spec; if no, document the fact explicitly so future-readers don't re-investigate.

  **Files:** `crates/flui-framework/Cargo.toml`, `crates/flui-framework/src/lib.rs`. **Logging:** none. **Validation:** `cargo check -p flui-framework` succeeds; an example doctest `use flui_framework::Widget; #[derive(Widget)] struct X;` compiles.

- [x] **T4.5 — Positive test: `#[widget(key)]` with non-default field name.** Add `crates/flui-framework/tests/widget_derive/pass_renamed_key_field.rs` (and register in the trybuild runner):
  - `#[derive(Widget)] struct R { #[widget(key)] id: Option<Key> }`. Compiles. Construct `R { id: Some(Key::local()) }` and assert that `Widget::key(&r)` returns `Some(&Key)` — i.e., the derive does NOT hard-code the field name `key` and correctly picks up whatever field carries the `#[widget(key)]` attribute. Guards against a subtle macro bug where the generated body uses `self.key` instead of `self.<attribute_field_name>`.
  - Also add a doctest in `crates/flui-macros/src/derive_widget.rs` showing the `id` example so the macro contract is self-documenting.

  **File:** `crates/flui-framework/tests/widget_derive/pass_renamed_key_field.rs`. **Logging:** none. **Validation:** `cargo test -p flui-framework --test widget_derive_compile` passes including the new case.

**Commit 4 (Phase 4):** `feat(macros): derive(Widget) skeleton + trybuild cases (SF01 phase 4)` — commits `crates/flui-macros/src/derive_widget.rs`, `crates/flui-macros/src/flui_macros.rs`, plus `crates/flui-framework/Cargo.toml`, `crates/flui-framework/src/lib.rs`, `crates/flui-framework/tests/widget_derive_compile.rs`, `crates/flui-framework/tests/widget_derive/*`.

### Phase 5 — Mini-example + workspace validation

- [x] **T5.1 — Add `examples/widget_surface_demo/` micro-example.** Naming follows existing `nav_demo` / `material_demo` / `animation_demo` convention (`*_demo`, feature-themed, no spec-number prefix). Cargo manifest depending on `flui-framework` only (no `flui-core` import — Framework users speak Framework, not Engine). `src/main.rs` defines a `Counter` widget per ARCHITECTURE.md §"Code Examples", matches the FROZEN spec target shape, exercises the T2.4 `prelude` import (`use flui_framework::prelude::*;`), and `fn main()` constructs an instance and prints `widget.key()`. This is a `cargo check`-only example — it does NOT spin up a window (no mounting until SF07). Document the "trait-surface-only" nature in the top comment with a link to the SF01 design spec.

  **Files:** `examples/widget_surface_demo/Cargo.toml`, `examples/widget_surface_demo/src/main.rs`. Add `"examples/widget_surface_demo",` to root `Cargo.toml` `[workspace] members` (immediately after the existing `examples/animation_demo` line for visual ordering). **Logging:** none. **Validation:** `cargo check -p widget_surface_demo` succeeds; the example does NOT run as a real GUI (no `App::run` etc.).

- [x] **T5.2 — Workspace-wide validation.** Run in order:
  - `cargo fmt --check` — must pass clean.
  - `cargo clippy --workspace --all-targets -- -D warnings` — must pass clean (incl. new framework crate's `missing_docs` deny).
  - `cargo check --workspace --all-targets` — must pass clean.
  - `cargo test --workspace` — all existing tests + the new SF01 tests must pass.
  - `cargo doc --workspace --no-deps` — must build without warnings (especially intra-doc-link warnings for the new crate).

  **Files:** none modified by this task; this is a verification gate. **Logging:** capture outputs in commit message body. **Validation:** all five commands exit 0.

### Phase 6 — Documentation + Post-Implementation Reviewer Triple + Sync

- [x] **T6.1 — Write the migration / authoring guide.** Add `docs/superpowers/migrations/SF01-widget-key-trait.md`:
  - "How to write a widget in flui-framework today" — show the `Leaf` / `Container` / `Counter` examples with full annotations.
  - "What works in SF01 vs what arrives in SF02/SF03/SF04/SF05" — explicit feature matrix.
  - "Anti-patterns" — Widget vs Render vs RenderOnce vs Component<C> decision table.
  - "Forward compatibility" — note the SF03 `cx` parameter promotion and the SF04 `WidgetState<W>` body fill-in. Note that today's SF01 widgets compile but cannot be **mounted** until SF07 lands.

  **File:** `docs/superpowers/migrations/SF01-widget-key-trait.md`. **Logging:** none. **Validation:** Markdown lints clean; cross-links to the design spec and to ARCHITECTURE.md resolve.

- [x] **T6.2 — Post-implementation reviewer triple launch on the code (parallel).** **DONE 2026-05-12** — three reviewers ran in parallel against the landed implementation. Common findings (all three converged): stale doc in widget.rs:44-45 (post-Amendment-1 drift), K91 cross-track contract not added to ROADMAP K91 entry, ARCHITECTURE.md still shows `impl Widget` instead of `impl IntoWidget`, ROADMAP SF01 entry still `[ ]`, plan T2.4/T3.1/T3.5 descriptions are pre-Amendment-1. All blockers actionable in T6.3. Per the same user-memory feedback. Three parallel Agent calls in one message:
  - `flui-arch-reviewer` reviews the actual `crates/flui-framework/` source for tier-boundary integrity vs the FROZEN design spec.
  - `migration-risk-adversary` hunts for silent regressions in the `flui_core::Key` re-export path and the transitional `BuildElement` bridge decision.
  - `rust-api-migration-auditor` re-runs semver / object-safety / feature-flag / blanket-impl analysis against the implementation diff.
  - Capture findings in a `## Post-Implementation Reviewer Notes` section appended to the design spec.

  **File:** updates `docs/superpowers/specs/2026-05-12-SF01-widget-key-trait-design.md`. **Logging:** none. **Validation:** every reviewer concern has a documented resolution (fix-in-place / defer-to-SF## / accept-with-rationale). If a reviewer flags a fix-in-place, loop back to the relevant T#.# task and re-run T5.2.

- [x] **T6.3 — Sync `.ai-factory/` artifacts and K91 cross-track contract.** Update:
  - `.ai-factory/ROADMAP.md` — mark `SF01 Widget + Key trait` checkbox done in the Phase II-F section; add to the `## Completed` table with date `2026-05-12` and a one-sentence summary mirroring K-track completion entries. Update the Phase II-F intro paragraph: "SF01 done — Framework tier scaffolding exists; SF02 (reconciliation) is the next critical-chain item gated on SF01 + K15."
  - `.ai-factory/ROADMAP.md` K91 entry — **add the SF01 cross-track contract** to the K91 description: "When K91 replaces the `pub use element::*;` glob at `crates/flui-core/src/lib.rs:154`, the new explicit re-export list MUST preserve crate-root visibility of `Key`, `ValueKey`, `GlobalKey` (and the engine-private path types `ElementId`, `LocalElementId` if Tier C still consumes them). Otherwise `flui_framework`'s Key re-exports break. Per SF01 design spec §'Current Inventory'." This binds K91's eventual implementer even if they never read the SF01 spec.
  - `.ai-factory/ARCHITECTURE.md` — update the "Framework: defining a Stateful Widget" code example at line ~359 and ~386: change `fn build(&mut self, cx: &mut BuildCx<'_>) -> impl Widget` to `fn build(&mut self, cx: &mut BuildCx<'_>) -> impl IntoWidget` to align with the FROZEN SF01 trait return type (Option B). Reviewer arch-B1 flagged this divergence.
  - `.ai-factory/RESEARCH.md` Active Summary — add an `**SF01 status (2026-05-12):**` paragraph mirroring the K-status paragraph style. Note: "Phase II-F is now open; SF02 (reconciliation) is the next item." Update the "Next step" list at the bottom: replace the stale "Next run `/aif-plan full K04-effect-frame-contract`" item with "Next run `/aif-plan full SF02-reconciliation`".
  - `AGENTS.md` — verify (do not blindly edit) that the agent role descriptions still match SF01's reality. Add a one-line note under `flui-arch-reviewer` if needed: "now reviews `flui-framework` Tier B crate as well as `flui-core`."
  - `docs/superpowers/specs/2026-04-13-flui-core-roadmap.md` — Cross-link the new SF01 design spec under the Phase II-F section if the file structure already accommodates SF references; otherwise leave a one-line note in the design spec itself.

  **Files:** `.ai-factory/ROADMAP.md`, `.ai-factory/RESEARCH.md`, `AGENTS.md` (verify-only edit if needed). **Logging:** none. **Validation:** `cargo check --workspace` still clean after artifact edits (Markdown files only — no Rust touched); spec / migration / ROADMAP / RESEARCH cross-links all resolve.

**Commit 5 (Phase 5 + 6):** `docs(sf01): migration guide + roadmap/research sync + post-impl reviewer notes` — commits `docs/superpowers/migrations/SF01-widget-key-trait.md`, design spec updates, `examples/widget_surface_demo/{Cargo.toml,src/main.rs}`, root `Cargo.toml` (examples member add), `.ai-factory/ROADMAP.md`, `.ai-factory/RESEARCH.md`, and any `AGENTS.md` / `ARCHITECTURE.md` (code-example update from `impl Widget` to `impl IntoWidget` per reviewer arch-B1) edits.

## Commit Plan

Five commits ordered along the phase boundaries. Each phase's commit body should include the validation commands run and their exit status.

| # | Commit | Scope | Files |
|---|---|---|---|
| 1 | `docs(sf01): freeze Widget + Key trait design spec` | Phase 0 design (text-only) | `docs/superpowers/specs/2026-05-12-SF01-widget-key-trait-design.md` |
| 2 | `feat(framework): introduce flui-framework crate skeleton (SF01 phase 1)` | Phase 1 crate scaffolding | `crates/flui-framework/{Cargo.toml,src/lib.rs}`, root `Cargo.toml`, `.ai-factory/qa/SF01-tier-isolation.md` |
| 3 | `feat(framework): Key re-exports + prelude + Widget/StatefulWidget trait surface (SF01 phases 2-3)` | Phase 2 + 3 traits + prelude + tests | `crates/flui-framework/src/{key.rs,prelude.rs,widget.rs,lib.rs}`, `crates/flui-framework/tests/{key_roundtrip.rs,trait_surface.rs}` |
| 4 | `feat(macros): derive(Widget) skeleton + trybuild cases on framework side (SF01 phase 4)` | Phase 4 proc-macro + framework-side trybuild | `crates/flui-macros/src/{derive_widget.rs,flui_macros.rs}`, `crates/flui-framework/{Cargo.toml,src/lib.rs}`, `crates/flui-framework/tests/widget_derive_compile.rs`, `crates/flui-framework/tests/widget_derive/*` |
| 5 | `docs(sf01): migration guide + roadmap/research sync + post-impl reviewer notes` | Phase 5 + 6 docs + example | `docs/superpowers/migrations/SF01-widget-key-trait.md`, design spec updates, `examples/widget_surface_demo/{Cargo.toml,src/main.rs}`, root `Cargo.toml`, `.ai-factory/{ROADMAP.md,RESEARCH.md}`, optional `AGENTS.md` |

## Dependencies / Order

```
T0.1 → T0.2 → T0.3                       (Phase 0 — design freeze, text-only)
        ↓
T1.1 → T1.2 → T1.3                       (Phase 1 — crate scaffold)
        ↓
T2.1 → T2.2 → T2.3 → T2.4                (Phase 2 — Key re-exports + prelude)
        ↓
T3.1 → T3.2 → T3.3 → T3.4 → T3.5         (Phase 3 — trait surface)
        ↓
T4.1 → T4.2 → T4.3 → T4.4 → T4.5         (Phase 4 — derive macro + renamed-field test)
        ↓
T5.1 → T5.2                              (Phase 5 — example + validation)
        ↓
T6.1 → T6.2 → T6.3                       (Phase 6 — docs + post-impl reviewers + sync)
```

Notes on ordering:
- T2.4 (prelude) ships before Phase 3 fills the trait surface because the prelude module needs the trait re-exports it points at; the actual `pub use` lines inside `prelude.rs` reference items added in T3.x, so the file is staged in T2.4 and the re-exports are completed (or finalized) at T3.4 when `lib.rs` is wired. Either land both touches atomically inside commit 3, or split into a follow-up minor edit during T3.4 — implementer's choice, but final state must match the spec.
- T4.5 (renamed-field positive test) extends T4.3's trybuild runner — implement T4.5 in the same trybuild test crate, no separate `.rs` runner.

T0.2 (pre-PR reviewer triple on the design spec) is a hard gate. T6.2 (post-implementation reviewer triple) is a hard gate before T6.3 syncs `.ai-factory/`. Both reviewer triples MUST dispatch all three agents in a single parallel message — per user-memory feedback "for K-track / SF-track PRs, dispatch flui-arch-reviewer + migration-risk-adversary + rust-api-migration-auditor in one message; they routinely find real bugs other checks miss."

## Out of scope (explicit denial — for reviewer cross-check)

The following are explicitly NOT in SF01 and will be rejected by reviewers if they appear in the implementation diff:

- `BuildCx` context type, `cx.read::<T>()`, `cx.inherit::<T>()` — SF03.
- `WidgetState<W>` trait body (`build`, `did_update_widget`, `dispose` methods) — SF04. SF01 only forward-declares the marker.
- `StateMap` / `HashMap<ElementId, Box<dyn State>>` — SF04.
- `setState` / dirty-list / rebuild propagation — SF05.
- Widget → Element compilation adapter / live mounting — SF07. SF01 ships only a documented transitional bridge (T0.1 decides exact shape).
- `InheritedWidget` analog — SF06.
- Async widgets (`StreamBuilder`, `FutureBuilder`) — SF08.
- Concrete widget catalogue (`Container`, `Row`, `Column`, `Text`, `Button`, etc.) — Tier C, gated on SF05.
- Migration of `flui-widgets` / `flui-material` / `flui-navigator` to depend on `flui-framework` — gated on SF05.
- Any edits to `crates/flui-core/src/**` beyond the read-only references documented in §"Current State". If the spec freeze (T0.3) ends up requiring a `flui-core` change, the change ships as a **separate** plan and a separate PR — per user-memory feedback "Keep docs work separate from code work — ADR sessions stay text-only".
- Hot-reload, inspector intro API (K22), Theme / MediaQuery / DefaultTextStyle implementations (SF06 + S14).

## Notes for the implementer

- **No code in Phase 0.** Phase 0 produces only Markdown. The design spec freeze is text-only per user-memory feedback.
- **Reviewer triple is mandatory at two gates** (T0.2 and T6.2). Do NOT serialize the three agents — dispatch in a single message with three Agent tool calls per user-memory feedback. The agents routinely find bugs CI misses.
- **Verbose logging policy applies to development tooling**, not committed source. SF01 ships zero log statements in `crates/flui-framework/**` and `crates/flui-macros/src/widget.rs`. Build-time macro errors use `syn::Error::to_compile_error()` → `compile_error!` token streams. Run-time panics use plain `panic!` / `unimplemented!` with documented messages.
- **The transitional `BuildElement` bridge decision is the highest-risk part of the spec freeze.** Reviewers in T0.2 should pay extra attention. Two acceptable outcomes: (a) defer the bridge entirely (SF01 ships traits-only, with a clear note that SF07 lands the mounting story), (b) ship a `#[doc(hidden)]` runtime-panic stub. Either is fine; what is NOT fine is shipping something that pins SF07's design.
- **K99 AFIT/RPITIT is the central enabler.** `fn build(&self) -> impl IntoWidget` (Option B, per FROZEN spec) works only because MSRV is 1.95. Fallback would have been `fn build(&self) -> Box<dyn Widget>` — but that allocates on the rebuild hot path, violating ARCHITECTURE.md principle 7. Frozen choice: `impl IntoWidget`, no fallback.
- **Workspace lint discipline:** `flui-framework` opts into `[lints] workspace = true` (per ARCHITECTURE.md §"Manifest tiering") and additionally denies `missing_docs` at the crate root (`#![deny(missing_docs)]` in `lib.rs`). Reviewers will check both.
- **No `flui-platform` / `flui-widgets` / `flui-material` / `flui-navigator` involvement.** Any temptation to migrate Tier C crates onto `flui-framework` in this PR is out of scope. SF01 ships in isolation; Tier C migration starts at SF05+.
