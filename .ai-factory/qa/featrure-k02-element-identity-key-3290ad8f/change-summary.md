## Change Summary

> Post-implementation update (2026-05-11): K02 runtime implementation has landed on this
> branch. The earlier pre-implementation risk inventory below is retained as QA context, but the
> active change set now includes the `flui-core` identity substrate, provider convergence,
> component callsite propagation, lifecycle/deferred identity fixes, K02 docs, and verification
> updates.

**Commits:** 2
**Changed files:** implementation, tests, specs, migration guide, and project status artifacts
**Risk level:** High

---

### What Changed

The current branch includes the landed K02 runtime implementation for Element identity and Key. The change defines the implemented QA surface for the API-breaking `flui-core` identity refactor across element identity, state retention, provider scope identity, cached view reuse, macro-generated components, lifecycle replay, deferred draw snapshots, and future Framework reconciliation.

QA now focuses on validating the landed behavior and guarding regressions across the scenarios below.

---

### Affected Areas

| Component | Change type | Description |
|---|---|---|
| `.ai-factory/plans/feature-K02-element-identity-key.md` | Completed / refined | 36-task implementation plan for K02, with second-pass refinements for deferred-draw identity snapshots, lifecycle duplicate detection, macro-generated components, state retention, and bounded cache substrate. |
| `flui-core` Element identity model | Implemented | `Element::id`, `ElementId`, `GlobalElementId`, `Window::element_id_stack`, lifecycle contexts, and built-in element id implementations now participate in normalized identity. |
| `Window` state retention | Implemented | `with_element_state`, `use_state`, `use_keyed_state`, and frame cleanup remain keyed by `(GlobalElementId, TypeId)` with Local / Value / Global key input support. |
| K01 Provider registry | Regression-covered | Provider scope keys and cached inherited dependency replay keep working after the K02 identity changes. |
| `AnyView::cached` / element cache behavior | Preserved | Existing cached-view behavior is retained while public keyed stateless element cache wrappers remain deferred to SF02/SF05. |
| `flui-macros` `derive(IntoElement)` | Regression-covered | Macro-generated `Component<Self>` wrappers participate in Local identity through preserved component callsite propagation. |

---

### Risks

High priority:

- `Key` and `ElementId` could become overlapping public identity concepts, leaving downstream authors unsure which one preserves state or drives provider scopes.
- Local source-location identity can collide for repeated same-callsite siblings and can lose state on reorder unless the limitation is explicit and tested.
- Duplicate-key diagnostics can false-positive because the same keyed element is visited during layout, prepaint, and paint.
- Deferred draws currently snapshot `element_id_stack`; if K02 adds counters or duplicate sets, raw path snapshots will be insufficient.
- `Window::with_element_state` and frame cleanup can retain, drop, or reuse state incorrectly if global ids change shape.
- K01 Provider scope keys can regress, especially provider removal invalidation and cached inherited dependency replay.

Medium priority:

- `derive(IntoElement)` and `Component<C: RenderOnce>` can be missed if only hand-written elements are migrated.
- Generalized stateless caching can grow beyond K02 scope and accidentally become a Framework-level API inside `flui-core`.
- Inspector source-location behavior can drift if identity source data is confused with debug-only inspector metadata.
- Identity resolution can introduce per-element allocation or committed hot-path logs.

Low priority:

- Documentation can preserve obsolete gpui-ce wording or leave migration guidance unclear for explicit ids and provider keys.

---

### Testing Recommendations

First priority:

- [ ] Verify the K02 design spec chooses one coherent public identity model and does not leave `Key` / `ElementId` precedence ambiguous.
- [ ] Verify Local, Value, and Global key semantics are documented with concrete loop, reorder, duplicate, and provider examples.
- [ ] Verify lifecycle duplicate detection distinguishes sibling collisions from ordinary layout/prepaint/paint repeats.
- [ ] Verify state retention behavior remains correct for insertion, deletion, and reorder scenarios.
- [ ] Verify K01 Provider identity and cached inherited dependency replay remain correct after the identity model changes.
- [ ] Verify deferred/root/overlay identity snapshots restore all resolver state, not only raw path segments.

Regression:

- [ ] Re-check `AnyView::cached` behavior, including accessed entity replay and inherited dependency replay.
- [ ] Re-check `derive(IntoElement)` and `Component<C: RenderOnce>` behavior under the selected Local identity model.
- [ ] Re-check built-in elements with explicit ids: `Div`, text/image/surface/animation wrappers, `UniformList`, and focus/dispatch helper elements.
- [ ] Re-check docs and migration guide for explicit ids, repeated callsites, provider keys, and Framework handoff boundaries.
