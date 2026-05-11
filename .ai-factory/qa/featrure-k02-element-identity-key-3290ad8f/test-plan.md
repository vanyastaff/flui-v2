## Test Plan: K02 Element Identity and Key

**Date:** 2026-05-11
**Branch / Version:** `featrure/k02-element-identity-key`
**Environment:** local development branch, post-implementation QA

> Post-implementation update: K02 now includes the runtime identity substrate and focused
> regression coverage. The scenarios below remain the QA contract; implemented checks are covered
> by `cargo test -p flui-core identity --lib`, `cargo test -p flui-core provider --lib`,
> `cargo test -p flui-core --tests`, `cargo check -p flui-widgets --all-targets`, and
> `cargo check --workspace --all-targets`.

---

### 1. Testing Goal

Verify that K02's planning and eventual implementation safely establish a coherent Element identity and Key substrate in `flui-core`. The main goal is to prevent identity regressions that would corrupt state retention, provider dependencies, cached view reuse, deferred draw behavior, or future Framework reconciliation.

This plan is the QA contract for the completed K02 implementation and review.

---

### 2. Test Scope

**In Scope** — we test:

- K02 design/spec completeness and decision quality.
- Public identity API clarity: Local, Value, and Global key semantics.
- `GlobalElementId` construction across layout, prepaint, and paint.
- `Window` identity stack behavior, frame cleanup, and state retention.
- Duplicate-key handling and lifecycle-phase repeat handling.
- `DeferredDraw` identity snapshot and restore behavior.
- K01 Provider scope identity, value inheritance, provider removal invalidation, and cached inherited dependency replay.
- Current `AnyView::cached` behavior and any new bounded cache substrate.
- Macro-generated `Component<Self>` wrappers from `derive(IntoElement)`.
- Built-in elements with explicit identity.
- Documentation and migration guidance.

**Out of Scope** — we don't test:

- Full Framework `Widget`, `State`, `BuildCx`, `setState`, or SF02 reconciliation behavior, because K02 must not implement those APIs.
- Platform backends, GPU rendering correctness, and scene rasterization, unless implementation unexpectedly touches those files.
- Full Window decomposition, because K06 owns that work.
- Unrelated K-track items such as action dispatch, style decomposition, and hit-test arena refactors.

---

### 3. Test Types

| Type | Priority | Area |
|---|---|---|
| Functional | High | Identity model, key equality, global id path construction, state retention, provider scope lookup. |
| Regression | High | K01 Provider registry, `AnyView::cached`, frame cleanup, deferred draw replay, built-in element ids. |
| Edge cases | Medium | Repeated same-callsite siblings, insertion/deletion/reorder, duplicate keys, missing keys, nested namespaces. |
| Negative | Medium | Invalid duplicate keys, unsupported Global key moves if deferred, unstable Local key misuse in reorder-sensitive lists. |
| Performance | Medium | Steady-state identity resolution, duplicate tracking, cache lookup, and state lookup on layout/prepaint/paint paths. |
| Documentation | Medium | Migration guide, rustdoc, Framework handoff boundaries, accepted limitations. |

---

### 4. Test Data

| Category | Data | Purpose |
|---|---|---|
| Local key callsite | Three siblings constructed from the same source location in a loop: `row(0)`, `row(1)`, `row(2)` without explicit value keys. | Verify deterministic Local fallback and documented non-reorder-stability. |
| Value keys | Explicit stable keys: `"row-a"`, `"row-b"`, `"row-c"` and reordered sequence `"row-c"`, `"row-a"`, `"row-b"`. | Verify state retention across supported reorders. |
| Duplicate keys | Two siblings using the same explicit value key `"dup"`. | Verify duplicate diagnostics and release behavior match the spec. |
| Nested namespaces | Parent namespace `"outer"`, child namespace `"inner"`, child key `"field"`. | Verify composed `GlobalElementId` paths. |
| Provider scopes | Nested `Provider<i32>` values `1` and `2`; repeated same-callsite providers with explicit keys `"left"` and `"right"`. | Verify nearest-provider-wins and no provider scope collision. |
| Cached view | Cached view with unchanged bounds/style/content mask/text style and inherited provider dependency. | Verify cache reuse and dependency replay. |
| Deferred draw | Element scheduled for deferred draw under a keyed parent namespace. | Verify identity resolver snapshot/restore. |
| Macro component | A type deriving `IntoElement` that renders keyed children through `Component<Self>`. | Verify macro-generated wrappers follow Local identity rules. |

---

### 5. Preconditions

- [ ] K02 implementation branch contains the K02 design spec.
- [ ] K02 implementation branch contains the K02 migration guide before final review.
- [ ] K02 implementation has not introduced Framework-tier public APIs into `flui-core`.
- [ ] K01 Provider behavior is understood as the baseline for provider regression checks.
- [ ] QA is run with a clean understanding of untracked local files so unrelated `.agents` / `.codex` changes are excluded from evaluation.

---

### 6. Acceptance Criteria

- [ ] All High-priority identity model, state retention, provider regression, and deferred-draw scenarios pass.
- [ ] The K02 design spec makes one clear `Key` / `ElementId` decision and lists accepted limitations.
- [ ] Local keys are deterministic and documented as not reorder-stable when occurrence fallback is involved.
- [ ] Value or Global keys preserve state across the reorder scenarios explicitly supported by the spec.
- [ ] Duplicate-key diagnostics do not trigger during ordinary lifecycle repeats of the same element.
- [ ] Provider scope identity and cached inherited dependency replay remain correct.
- [ ] `AnyView::cached` behavior remains compatible.
- [ ] Macro-generated components are covered by the identity model.
- [ ] The implementation does not add committed per-element logs or new identity hot-path interior mutability.

---

### 7. Plan Risks

| Risk | Impact | Mitigation |
|---|---|---|
| K02 design overreaches into Framework reconciliation | High | Keep tests focused on Engine identity substrate; reject public `Widget` / `State` / `BuildCx` additions in K02. |
| Duplicate-key checks confuse lifecycle phases with sibling collisions | High | Include lifecycle-repeat cases for layout, prepaint, and paint. |
| Deferred draws lose identity resolver state | High | Include deferred-draw snapshot/restore scenario with keyed parent namespace. |
| Provider identity regresses silently | High | Include nested and repeated provider scope scenarios plus cached inherited dependency replay. |
| Stateless cache substrate becomes too broad | Medium | Accept internal substrate only if public wrapper is not required for K02. |
| Local occurrence fallback is mistaken for reorder-safe identity | Medium | Include reorder scenarios that distinguish Local fallback from explicit Value/Global keys. |

### 8. Checklist

| Check | Priority |
|---|---|
| K02 spec has one public identity model decision. | High |
| Local / Value / Global key semantics are documented with examples. | High |
| `GlobalElementId` paths remain stable across lifecycle phases. | High |
| State retention works for keyed insertion, deletion, and reorder. | High |
| Duplicate explicit sibling keys are detected according to the spec. | High |
| Lifecycle repeats do not produce false duplicate-key errors. | High |
| Deferred draw snapshot/restore includes all required resolver state. | High |
| K01 Provider scope identity and dependency replay still work. | High |
| `AnyView::cached` keeps current behavior. | Medium |
| Macro-generated components follow the identity model. | Medium |
| Built-in explicit-id elements migrate coherently. | Medium |
| Migration guide covers explicit ids, repeated callsites, provider keys, and cache behavior. | Medium |
| Hot-path audit finds no committed per-element logs or unnecessary allocation. | Medium |
