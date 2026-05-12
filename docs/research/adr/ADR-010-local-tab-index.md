# ADR-010: Local tab-index — already present, contract to be made explicit

**Date:** 2026-05-12
**Status:** Draft — documents an existing capability and fixes its public
contract. No code changes land with this ADR.
**Scope:** `flui-core/src/tab_stop.rs`, `flui-a11y` (currently a stub),
the focus chain in `flui-core/src/window.rs`.
**Drivers:** [zed-industries/zed#34796](https://github.com/zed-industries/zed/issues/34796).

## Context

GPUI #34796 asks for a local tab-index API. The complaint upstream is that
the standard web tab-index forces a *global* ordering across the whole
page, which is painful when widgets are composed: every library author
has to pick a number, and the numbers conflict.

flui-v2 inherited from GPUI a `TabStopMap` that **already implements**
hierarchical tab order via `begin_group(tab_index)` / `end_group()`. The
order is stored as a `SumTree` of `TabStopPath`s (a `SmallVec<TabIndex>`),
which gives lexicographic comparison across nested groups for free. In
other words, the upstream feature request is, at the engine level, **a
closed gap in flui-v2**.

What is missing is not the capability, it is the **contract**: the API is
`pub(crate)`-leaning, the semantics are not documented, and the
keyboard-navigation behaviour (`next`, `prev`, wrap-around, skip-disabled)
has no explicit specification a widget author can read. This ADR fixes
that.

## Current behaviour (verified)

References cite the commit this ADR is written against.

[`crates/flui-core/src/tab_stop.rs:10`](../../../crates/flui-core/src/tab_stop.rs#L10):

```rust
pub(crate) struct TabStopMap {
    current_path: TabStopPath,
    pub(crate) insertion_history: Vec<TabStopOperation>,
    by_id: FxHashMap<FocusId, TabStopNode>,
    order: SumTree<TabStopNode>,
}
```

`TabStopPath` ([line 37](../../../crates/flui-core/src/tab_stop.rs#L37)) is
a `SmallVec<[TabIndex; 6]>`. Inline-allocates up to 6 levels of nesting;
deeper trees spill to the heap.

`TabIndex` ([line 34](../../../crates/flui-core/src/tab_stop.rs#L34)) is
`isize`. Negative values are permitted, supporting "in tab order but
behind everyone else" patterns (the web semantics of `tabindex="-1"`
combined with explicit ordering).

Grouping API ([lines 92, 98](../../../crates/flui-core/src/tab_stop.rs#L92)):

```rust
pub fn begin_group(&mut self, tab_index: isize) { /* push path */ }
pub fn end_group(&mut self) { /* pop path */ }
```

Insertion: each `FocusHandle` carries its own `tab_index`; the
TabStopMap inserts at the *current path*, so siblings inside a `group(5)`
are ordered by their own `tab_index` within the group, and the group
itself sits among its peers by `5`.

Navigation: `next(focused_id)` ([line 111](../../../crates/flui-core/src/tab_stop.rs#L111))
walks the SumTree to the next entry with `tab_stop = true`, falling
through groups boundary-wise.

[`crates/flui-a11y/src/lib.rs:1`](../../../crates/flui-a11y/src/lib.rs#L1)
is a 4-line stub:

```text
// flui-a11y: Accessibility support for flui
//
// Will provide semantic tree, ARIA-like roles,
// screen reader integration, and keyboard navigation helpers.
```

So screen-reader navigation has no implementation yet; only keyboard
focus order is wired.

## Findings vs upstream issues

| Issue | Symptom | Repro in flui-v2 today |
|-------|---------|-------------------------|
| [zed-industries/zed#34796](https://github.com/zed-industries/zed/issues/34796) | Global-only tab-index API forces every region to participate in one ordering. | **no** — already solved at the engine level by `begin_group`/`end_group` and the `TabStopPath`-based SumTree. The remaining work is API surfacing, not capability. |

## Decision (contract)

1. **Tab order is hierarchical.** A FocusHandle's effective order is the
   lexicographic comparison of its `TabStopPath`. Siblings compare by
   their per-level `tab_index`; depth differences fall through the
   `SumTree`'s `Bias::Right` semantics.

2. **`tab_index = 0` means "default" (document order within the group).**
   Negative values come after every non-negative entry in the same group
   (web `tabindex="-1"` semantics, except that we never disable the
   stop entirely — that is `tab_stop = false`).

3. **`tab_stop: bool` on the `FocusHandle` is independent of
   `tab_index`.** A focus handle may participate in the *map* (so it
   can receive programmatic focus) without participating in keyboard
   navigation (`tab_stop = false`).

4. **Group boundaries are not absorbing.** Tab from the last element of
   a group lands on the next element in the parent's order, not on a
   group sentinel. Reverse tab is symmetric.

5. **`next` and `prev` wrap.** Reaching the end of the document tab
   cycle wraps to the beginning. A future `Window::focus_chain_strategy`
   may opt out of wrapping; until then wrap is the contract.

6. **Local groups are the user-facing primitive.** Widget authors call
   `cx.tab_group(tab_index, |cx| { … })` (the public sugar for
   `begin/end_group`); they do not reason about `TabStopPath` directly.
   That helper is the API the user actually composes with.

7. **flui-a11y will integrate, not redefine.** When the a11y crate
   moves past the stub state, it consumes the same `TabStopMap` for
   screen-reader traversal order. Two orders (visual / focus and
   accessibility-tree) are not implemented today; one or two ADRs from
   now we may need them, but the default is "screen reader reads in
   focus order".

## Consequences

- Widget libraries can compose without colliding on global tab numbers.
- A future Storybook-style example app can be written around
  `tab_group`-driven navigation without engine changes.
- The SumTree path representation has an inline cap of 6 — deep nesting
  spills. That is documented but not constraining for any UI we
  realistically build.
- Future a11y work has a target shape; it does not need to invent a
  second tab-stop store.

## Out of scope (separate ADRs)

- **Screen-reader / AT-SPI / UIA integration.** flui-a11y stub work.
- **Focus traversal under RTL languages** (does Tab go visually or
  logically?). Decision lives next to the BiDi text-direction ADR
  whenever it is written.
- **Focus restoration after modal close.** Adjacent topic; deserves its
  own ADR; the current contract is silent on it.
- **Roving tab-index inside composite widgets** (e.g. radio groups). A
  widget-layer concern, not engine.

## Action items (tracked; no code lands with this ADR)

1. Surface `cx.tab_group(tab_index, |cx| { … })` as the public sugar.
   Today the engine has `begin_group` / `end_group` `pub fn` on a
   `pub(crate)` struct; widget code reaches it via existing macros, but
   the helper name is not stable.
2. Document the `tab_index` semantics in a comment block at the top of
   [`tab_stop.rs`](../../../crates/flui-core/src/tab_stop.rs) pointing
   to this ADR.
3. Add tests in `tab_stop.rs::tests` that cover the six decision points
   above (nested groups, negative `tab_index`, `tab_stop = false`,
   wrap, etc.).
4. When [`flui-a11y`](../../../crates/flui-a11y/src/lib.rs) gains real
   code, expose a `TabStopMap::iter()` for AT traversal — do not
   duplicate the store.

## References

### Upstream issues
- [zed-industries/zed#34796](https://github.com/zed-industries/zed/issues/34796) — local tab-index API request.

### Internal
- [docs/research/adr/ADR-009-input-ime-contract.md](ADR-009-input-ime-contract.md) — input pipeline sibling.
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — theme #5 (_Input / focus / hit-testing_), continued.
