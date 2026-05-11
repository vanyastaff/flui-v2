## Test Cases: K02 Element Identity and Key

> Post-implementation update (2026-05-11): these cases now apply to the completed K02 branch.
> Focused identity/provider/component/deferred lifecycle coverage is implemented in
> `crates/flui-core`; broader Framework reconciliation remains out of scope for K02.

---

### TC-001: K02 spec chooses one public identity model

**Priority:** High
**Type:** Positive

**Precondition:**

K02 design spec exists for the implementation branch.

**Steps:**

1. Open the K02 design spec.
2. Locate the section that defines the public identity model.
3. Confirm it states whether `ElementId` is renamed, wrapped, aliased, or kept under a new `Key`.
4. Confirm it defines precedence when an existing explicit id and a new key-like API could both apply.
5. Confirm migration guidance names the old and new API forms.

**Expected result:**

The spec has one coherent identity model. It does not leave `Key` and `ElementId` as competing public concepts with unclear precedence.

**Test data:**

```text
Existing identity names: ElementId, GlobalElementId
New identity concepts: Local, Value, Global key
```

---

### TC-002: Local fallback is deterministic for repeated same-callsite siblings

**Priority:** High
**Type:** Positive

**Precondition:**

K02 implementation includes Local key generation and sibling occurrence handling.

**Steps:**

1. Build or inspect an element subtree where three sibling elements are constructed from the same source location without explicit value keys.
2. Render the tree once with logical order `row(0)`, `row(1)`, `row(2)`.
3. Capture the three generated global identity paths.
4. Render the same tree again with the same order.
5. Compare the second render's global identity paths with the first render.

**Expected result:**

The same repeated Local-key sibling structure produces deterministic global identity paths across identical renders.

**Test data:**

```text
Sibling construction pattern:
- row(0) from same callsite
- row(1) from same callsite
- row(2) from same callsite
No explicit Value keys
```

---

### TC-003: Local fallback is not treated as reorder-stable identity

**Priority:** High
**Type:** Edge case

**Precondition:**

K02 implementation defines Local occurrence fallback behavior and state retention behavior.

**Steps:**

1. Use three sibling elements from the same source location without explicit value keys.
2. Associate distinguishable state with each sibling: `state-a`, `state-b`, `state-c`.
3. Render in order `A`, `B`, `C`.
4. Reorder to `C`, `A`, `B` without adding explicit Value keys.
5. Inspect which state is retained by each position.
6. Compare observed behavior with the K02 spec.

**Expected result:**

The behavior matches the documented Local fallback limitation. Local occurrence identity is deterministic but is not presented as reorder-stable identity.

**Test data:**

```text
Initial order: A, B, C
Reordered: C, A, B
State labels: state-a, state-b, state-c
Keys: Local fallback only
```

---

### TC-004: Explicit Value keys preserve state across supported reorder

**Priority:** High
**Type:** Positive

**Precondition:**

K02 implementation includes explicit Value key support and state retention.

**Steps:**

1. Create three sibling elements with explicit Value keys: `"row-a"`, `"row-b"`, `"row-c"`.
2. Attach distinguishable state to each keyed sibling.
3. Render in order `"row-a"`, `"row-b"`, `"row-c"`.
4. Reorder to `"row-c"`, `"row-a"`, `"row-b"`.
5. Inspect state retained by each keyed sibling after reorder.

**Expected result:**

Each element retains state by its explicit Value key, not by old position.

**Test data:**

```text
Initial keys: row-a, row-b, row-c
Reordered keys: row-c, row-a, row-b
Expected state ownership:
row-a -> state-a
row-b -> state-b
row-c -> state-c
```

---

### TC-005: Duplicate explicit sibling keys are diagnosed

**Priority:** High
**Type:** Negative

**Precondition:**

K02 implementation defines duplicate-key behavior for siblings.

**Steps:**

1. Create two sibling elements under the same parent namespace.
2. Assign both siblings the same explicit Value key `"dup"`.
3. Render the parent subtree.
4. Observe the diagnostic behavior in the configured build mode.
5. Compare the behavior with the K02 spec's debug/release rule.

**Expected result:**

Duplicate sibling keys are handled exactly as documented. In diagnostic builds, the failure is deterministic and names the duplicated key or identity path clearly.

**Test data:**

```text
Parent namespace: list
Sibling 1 key: dup
Sibling 2 key: dup
```

---

### TC-006: Lifecycle phase repeats do not look like duplicate siblings

**Priority:** High
**Type:** Regression

**Precondition:**

K02 implementation includes duplicate-key diagnostics.

**Steps:**

1. Create one keyed element with key `"single"`.
2. Render it through layout, prepaint, and paint.
3. Confirm duplicate-key detection sees this as one element visited across phases, not as three sibling uses of the same key.
4. Repeat with a nested keyed child.

**Expected result:**

No duplicate-key diagnostic is emitted for normal layout/prepaint/paint visits of the same keyed element.

**Test data:**

```text
Element key: single
Nested child key: child
Lifecycle phases: layout, prepaint, paint
```

---

### TC-007: Nested namespaces compose global identity paths

**Priority:** High
**Type:** Positive

**Precondition:**

K02 implementation includes namespace/key path composition.

**Steps:**

1. Create a parent namespace `"outer"`.
2. Inside it, create a child namespace `"inner"`.
3. Inside the child namespace, create a keyed element `"field"`.
4. Render the subtree.
5. Inspect the `GlobalElementId` path exposed to layout/prepaint/paint contexts.

**Expected result:**

The global identity path composes parent and child namespace segments in deterministic order and remains stable across lifecycle phases.

**Test data:**

```text
Parent namespace: outer
Child namespace: inner
Element key: field
Expected path shape: outer.inner.field or the exact equivalent chosen by the K02 spec
```

---

### TC-008: State retention prunes disappeared keyed elements

**Priority:** High
**Type:** Regression

**Precondition:**

K02 implementation preserves frame cleanup for accessed element states.

**Steps:**

1. Render keyed siblings `"row-a"`, `"row-b"`, `"row-c"` with retained state.
2. Render the next frame with only `"row-a"` and `"row-c"`.
3. Inspect state retention for `"row-a"` and `"row-c"`.
4. Confirm the state for `"row-b"` is not retained as an accessed state in the next frame.
5. Render `"row-b"` again later.

**Expected result:**

State for still-present keyed elements survives. State for the disappeared keyed element is pruned and does not silently attach to a later unrelated element.

**Test data:**

```text
Frame 1 keys: row-a, row-b, row-c
Frame 2 keys: row-a, row-c
Frame 3 keys: row-a, row-c, row-b
```

---

### TC-009: Provider scope identity still supports nested nearest-provider wins

**Priority:** High
**Type:** Regression

**Precondition:**

K02 implementation has migrated K01 Provider identity to the new key model.

**Steps:**

1. Create an outer `Provider<i32>` with value `1`.
2. Inside it, create a child that reads inherited `i32`.
3. Add an inner `Provider<i32>` with value `2`.
4. Inside the inner provider, create a child that reads inherited `i32`.
5. After the inner provider subtree, create another child that reads inherited `i32`.
6. Render the subtree and inspect read values.

**Expected result:**

The first child sees `1`, the inner child sees `2`, and the child after the inner provider sees `1`. K02 identity changes do not break provider active-scope restoration.

**Test data:**

```text
Outer provider: i32 = 1
Inner provider: i32 = 2
Expected reads: 1, 2, 1
```

---

### TC-010: Repeated same-callsite providers do not collide under K02 rules

**Priority:** High
**Type:** Regression

**Precondition:**

K02 implementation defines provider identity under the new key model.

**Steps:**

1. Create two sibling providers of the same value type from the same callsite.
2. Give them explicit keys `"left"` and `"right"` if the K02 spec requires explicit Value keys for reorder-stable provider identity.
3. Set provider values to `"left-theme"` and `"right-theme"`.
4. Render children under each provider.
5. Inspect inherited values read by each child.

**Expected result:**

Provider scopes do not collide. Each child reads the value from its own provider scope.

**Test data:**

```text
Provider type: ThemeName
Left provider key: left
Left value: left-theme
Right provider key: right
Right value: right-theme
```

---

### TC-011: Cached view replay preserves inherited provider dependency

**Priority:** High
**Type:** Regression

**Precondition:**

K02 implementation keeps K01 cached inherited dependency replay.

**Steps:**

1. Render a cached view that subscribes to an inherited value.
2. Render a second frame where the cached view output is reused.
3. Change the provider value.
4. Render again.
5. Inspect whether the dependent cached view is invalidated and refreshed according to K01 behavior.

**Expected result:**

The cached view does not lose its provider dependency during reuse. Provider changes still invalidate the dependent view.

**Test data:**

```text
Inherited value: ThemeName("light") -> ThemeName("dark")
Cached view inputs unchanged except provider value
```

---

### TC-012: Deferred draw restores full identity resolver state

**Priority:** High
**Type:** Regression

**Precondition:**

K02 implementation includes deferred draw identity snapshot/restore rules.

**Steps:**

1. Render a keyed parent namespace.
2. Schedule a deferred draw under that namespace.
3. Ensure the deferred subtree contains keyed siblings that require Local occurrence or duplicate tracking state.
4. Execute deferred prepaint and paint.
5. Inspect global identity paths and duplicate-key diagnostics.

**Expected result:**

Deferred prepaint/paint uses the correct parent identity context and does not lose resolver state. No false duplicate-key diagnostics occur.

**Test data:**

```text
Parent namespace: overlay-host
Deferred children: repeated Local-key siblings or explicit keys overlay-a, overlay-b
```

---

### TC-013: Macro-generated component participates in Local identity model

**Priority:** Medium
**Type:** Regression

**Precondition:**

K02 implementation handles `derive(IntoElement)` and `Component<C: RenderOnce>` according to the spec.

**Steps:**

1. Use a component type that derives `IntoElement`.
2. Have the component render a keyed child and an unkeyed child using Local fallback.
3. Render two instances of the component as siblings.
4. Inspect global identity paths for children inside each component.
5. Confirm source-location and component namespace behavior matches the K02 spec.

**Expected result:**

Macro-generated component wrappers do not collapse child identity across sibling component instances and do not require hand-written identity code to be correct.

**Test data:**

```text
Component type: RowComponent
Component instances: RowComponent #1, RowComponent #2
Child keys: explicit child-key, Local fallback child
```

---

### TC-014: `AnyView::cached` behavior remains compatible

**Priority:** Medium
**Type:** Regression

**Precondition:**

K02 implementation refactors or preserves `AnyView::cached`.

**Steps:**

1. Render a cached view with fixed bounds, content mask, and text style.
2. Render a second frame with the same cache inputs.
3. Confirm cached prepaint/paint output is reused.
4. Change bounds or text style.
5. Render again.

**Expected result:**

Cached output is reused only when the documented cache inputs remain unchanged. Changing bounds or text style invalidates reuse.

**Test data:**

```text
Bounds: 100x100 -> 120x100
Text style: default -> bold
Content mask: unchanged
```

---

### TC-015: Inspector source location remains separate from identity semantics

**Priority:** Medium
**Type:** Regression

**Precondition:**

K02 implementation distinguishes Local identity source data from inspector-only debug metadata.

**Steps:**

1. Render an element with a source location visible to inspector/debug tooling.
2. Render the same element in a configuration where inspector-only metadata is not available.
3. Verify that required identity semantics still work if the K02 spec requires source location for Local keys.
4. Verify inspector navigation still uses the correct source location when available.

**Expected result:**

Identity behavior does not depend accidentally on debug-only inspector fields. Inspector source navigation remains correct where supported.

**Test data:**

```text
Element: Div with source location
Identity: Local key fallback
Inspector metadata: available / unavailable
```

---

### TC-016: K02 does not introduce Framework-tier API into `flui-core`

**Priority:** High
**Type:** Negative

**Precondition:**

K02 implementation branch is ready for review.

**Steps:**

1. Inspect public exports and new public types in `flui-core`.
2. Look for Framework-tier concepts: `Widget`, `State`, `BuildCx`, `setState`, `InheritedWidget`, or full reconciliation APIs.
3. Inspect docs and migration guide for any claim that K02 implements SF01 or SF02.
4. Compare findings with K02 out-of-scope section.

**Expected result:**

K02 provides Engine identity primitives only. No final Framework-tier public API is introduced in `flui-core`.

**Test data:**

```text
Forbidden K02 public concepts:
- Widget
- State
- BuildCx
- setState
- InheritedWidget
- full reconciliation API
```

---

### TC-017: Hot-path identity resolution has no committed per-element diagnostics

**Priority:** Medium
**Type:** Negative

**Precondition:**

K02 implementation branch is ready for review.

**Steps:**

1. Inspect identity resolution code paths used by layout, prepaint, and paint.
2. Confirm no committed per-element diagnostic output is present.
3. Confirm invariant failures use deterministic assertions or test-only diagnostics.
4. Confirm the migration plan's no hot-path logging rule is satisfied.

**Expected result:**

Identity hot paths do not emit committed per-element logs or diagnostic output during normal rendering.

**Test data:**

```text
Hot paths:
- identity stack push/pop
- GlobalElementId construction
- duplicate-key tracking
- cache lookup
- provider scope lookup
```

---

## Test Data

### Positive

* Explicit Value keys: `"row-a"`, `"row-b"`, `"row-c"`
* Nested namespaces: `"outer"`, `"inner"`, `"field"`
* Provider values: `1`, `2`, `"left-theme"`, `"right-theme"`
* Cached view inputs: fixed bounds, content mask, text style, inherited value

### Negative

* Duplicate sibling key: `"dup"` used twice under the same parent
* Unsupported Global key move if K02 explicitly defers cross-tree moves
* Local occurrence fallback used for reorder-sensitive list state
* Framework-tier type names appearing in K02 public `flui-core` API

### Edge

* Same keyed element visited during layout, prepaint, and paint
* Deferred subtree under keyed parent namespace
* Macro-generated component siblings
* Disappeared keyed element that later reappears
