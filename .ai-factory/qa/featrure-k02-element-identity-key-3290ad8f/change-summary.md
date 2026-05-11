## Change Summary

> Post-implementation update (2026-05-11): K02 runtime implementation has landed on this
> branch. The earlier pre-implementation risk inventory below is retained as QA context, but the
> active change set now includes the `flui-core` identity substrate, provider convergence,
> component callsite propagation, lifecycle/deferred identity fixes, K02 docs, and verification
> updates.

**Commits:** 0
**Changed files:** implementation, tests, specs, migration guide, and project status artifacts
**Risk level:** High

---

### What Changed

The current branch introduces and refines the K02 planning artifact for Element identity and Key work. No runtime implementation has landed yet. The change defines the intended QA surface for a future API-breaking `flui-core` identity refactor that will affect element identity, state retention, provider scope identity, cached view reuse, macro-generated components, and future Framework reconciliation.

Because this is a design/planning change, QA focuses on whether the plan is complete enough to safely drive implementation and on the scenarios the eventual K02 implementation must satisfy.

---

### Affected Areas

| Component | Change type | Description |
|---|---|---|
| `.ai-factory/plans/feature-K02-element-identity-key.md` | Added / refined | New 36-task implementation plan for K02, with second-pass refinements for deferred-draw identity snapshots, lifecycle duplicate detection, macro-generated components, state retention, and bounded cache substrate. |
| `flui-core` Element identity model | Planned | Future work will touch `Element::id`, `ElementId`, `GlobalElementId`, `Window::element_id_stack`, lifecycle contexts, and built-in element id implementations. |
| `Window` state retention | Planned | Future work must preserve `with_element_state`, `use_state`, `use_keyed_state`, and frame cleanup keyed by `(GlobalElementId, TypeId)`. |
| K01 Provider registry | Planned regression area | Provider scope keys and cached inherited dependency replay must keep working after K02 identity changes. |
| `AnyView::cached` / element cache behavior | Planned | Future work must either generalize cache internals safely or explicitly defer public keyed stateless caching without breaking current view cache behavior. |
| `flui-macros` `derive(IntoElement)` | Planned regression area | Macro-generated `Component<Self>` wrappers must participate in the chosen Local identity model or be explicitly excluded by design. |

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
