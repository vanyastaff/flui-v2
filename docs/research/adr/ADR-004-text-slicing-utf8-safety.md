# ADR-004: Text slicing — UTF-8 boundary safety in `text_system`

**Date:** 2026-05-12
**Status:** Draft — contract only. No code changes land with this ADR.
**Scope:** `flui-core/src/text_system/{line,line_layout,line_wrapper}.rs`.
**Drivers:** [zed-industries/zed#49860](https://github.com/zed-industries/zed/issues/49860).

## Context

GPUI #49860 reports a hard panic (and subsequent `SIGABRT` across the
`CoreText` FFI boundary on macOS) when a text element truncates a string that
mixes single-byte ASCII and multi-byte CJK characters with an ellipsis: as the
container widens just enough to reveal the first CJK code point, the
calculated slice index lands in the middle of a multi-byte sequence and
`str::Index` panics with "byte index N is not a char boundary".

flui-v2 inherited the same `text_system` shape, split across three files
(`line.rs`, `line_layout.rs`, `line_wrapper.rs`), with at least one slice
operation that depends on an externally-supplied byte index being on a
boundary. This ADR audits every slice in `text_system`, classifies each, and
fixes the contract that callers must obey before any new feature is built on
top.

The ADR is intentionally narrow: it is **not** about shaping CJK correctly,
emoji clusters, grapheme handling, or BiDi. It is about preventing a single
class of crash by making the invariant explicit.

## Current behaviour (verified)

References below cite `crates/flui-core/src/text_system` at the commit this
ADR is written against. Each `&str`/`SharedString` slice has been tracked
back to the source of its index.

### Safe — index comes from `char_indices`

[`line_wrapper.rs:205`](../../../crates/flui-core/src/text_system/line_wrapper.rs#L205):

```rust
TruncateFrom::End => {
    SharedString::from(format!("{}{truncation_affix}", &line[..truncate_ix]))
}
```

`truncate_ix` is the byte index returned by `should_truncate_line`, which
walks `line.char_indices()` ([line_wrapper.rs:169](../../../crates/flui-core/src/text_system/line_wrapper.rs#L169)).
Every value `char_indices` yields is a valid char boundary by construction.
**No panic.**

### Safe — boundary explicitly enforced

[`line_wrapper.rs:202`](../../../crates/flui-core/src/text_system/line_wrapper.rs#L202):

```rust
TruncateFrom::Start => SharedString::from(format!(
    "{truncation_affix}{}",
    &line[line.ceil_char_boundary(truncate_ix + 1)..]
)),
```

`truncate_ix + 1` may land mid-codepoint; `str::ceil_char_boundary` walks
forward to the next boundary. **No panic, but the contract relies on
`ceil_char_boundary` being available** — that is `feature(round_char_boundary)`
stable in 1.80+. Our MSRV (K99 raised it to 1.95) covers this.

### Needs audit — index comes from the shaper

[`line_layout.rs:150`](../../../crates/flui-core/src/text_system/line_layout.rs#L150):

```rust
let character = text[glyph.index..].chars().next().unwrap();
```

`glyph.index` is a byte offset that the platform shaper (`CoreText`,
`DirectWrite`, or HarfBuzz on Linux) records for a glyph cluster. The
implicit assumption is that the shaper always reports indices on a char
boundary, which is true for canonical text-run shaping but is not enforced
by any type in our code. The `.unwrap()` here will panic if the assumption
ever breaks — for example, when a future shaper supports grapheme clusters
that span codepoints (regional indicators, emoji ZWJ sequences) and we
forget to update the indexing rule.

### Needs audit — `byte_index` from line splitting

[`line.rs:211-212`](../../../crates/flui-core/src/text_system/line.rs#L211):

```rust
let left_text  = SharedString::new(self.text[..byte_index].to_string());
let right_text = SharedString::new(self.text[byte_index..].to_string());
```

`byte_index` is computed by `ShapedLine::split_at` from a column or x-offset.
The path that produces it walks the layout runs; whether the result lands on
a char boundary in **every** path (mixed-script lines, RTL embeds, soft
hyphens) is not currently asserted anywhere.

## Findings vs upstream issues

| Issue | Symptom | Repro in flui-v2 today |
|-------|---------|-------------------------|
| [zed-industries/zed#49860](https://github.com/zed-industries/zed/issues/49860) | `not a char boundary` panic during CJK truncation under live resize. | **Likely not — for the exact GPUI repro path.** `should_truncate_line` walks `char_indices`, and `TruncateFrom::Start` uses `ceil_char_boundary`. The same crash would not fire on the matching call site. **Two other slice sites (`line_layout.rs:150`, `line.rs:211`) carry an implicit boundary assumption and have not been proven safe**; they are queued for audit. |

The honest claim is: **the exact GPUI #49860 site is fine in flui-v2**, but
the invariant is held by inspection, not by a type. Two other code paths
hold it by external promise (the shaper / the line-splitter) and would
benefit from an explicit assert.

## Decision (contract)

1. **Every `&str` slice index inside `text_system` must originate from one of
   three sources**, and the source must be discoverable from the surrounding
   code:

   - **(a) `str::char_indices`** (or `str::byte_offsets` once that lands) —
     trivially on a boundary.
   - **(b) An explicit `str::is_char_boundary` / `floor_char_boundary` /
     `ceil_char_boundary` call** at or before the slice site.
   - **(c) A typed `ByteOffset` produced by a routine that documents and,
     where cheap, asserts the boundary invariant** (the shaper, the line
     splitter, the layout engine).

2. **Indexing patterns `text[i..]` and `text[..i]` with a raw `usize` are
   forbidden** unless one of (a)–(c) is documented in a comment on the slice
   site. This is a *new* rule and is binding on new code immediately.

3. **`debug_assert!(text.is_char_boundary(i))`** is required at every (c)
   site, immediately before the slice. The cost is zero in release builds
   and turns a future regression into a deterministic test failure instead
   of a CJK-resize-only panic at a user site.

4. **`.unwrap()` after `.chars().next()` is a contract violation indicator,
   not a bug fix.** When a slice starts with `text[i..].chars().next()`, the
   slice itself must have come from (a)–(c) — otherwise the `.unwrap()` is
   the place where the panic surfaces, not the place where it originates.

5. **Truncation affixes are joined with `format!`, never with slice
   concatenation that depends on byte length.** Already the case; this ADR
   only codifies it.

## Consequences

- The single user-visible crash class behind #49860 becomes a deterministic
  test failure in debug builds long before it reaches an end user.
- `line_layout.rs:150` and `line.rs:211` get explicit `debug_assert`s when
  someone touches them next. No urgency: neither site has a known bug today
  in our test corpus.
- New `text_system` code carries the explicit contract — comment + assert —
  for every raw byte slice.
- We avoid the slow drift toward "the shaper *probably* lands on a boundary"
  that hands users panics under future emoji / grapheme-cluster work.

## Out of scope (separate ADRs)

- **Grapheme cluster vs codepoint semantics.** Whether selection moves by
  codepoint or by extended grapheme cluster (Unicode UAX #29) is a UX/
  semantics decision; it does not change the byte-boundary safety
  requirement.
- **Bidirectional text (BiDi) and shaping correctness.** Orthogonal —
  shaping can be wrong without panicking, and slice safety covers panics
  only.
- **CJK metrics and ellipsis width estimation.** Performance, not safety.
- **Text rasterization strategy (metrics hinting, bi-level).** GPUI #55214
  is its own ADR candidate — text-rendering-strategy, not slicing.

## Action items (tracked; no code lands with this ADR)

1. Add `debug_assert!(text.is_char_boundary(byte_index))` immediately before
   [line.rs:211](../../../crates/flui-core/src/text_system/line.rs#L211) and
   [line.rs:212](../../../crates/flui-core/src/text_system/line.rs#L212).
2. Add `debug_assert!(text.is_char_boundary(glyph.index))` immediately before
   [line_layout.rs:150](../../../crates/flui-core/src/text_system/line_layout.rs#L150),
   plus a `// CONTRACT:` comment naming the shaper as the producer.
3. Add a property-style unit test in `line_wrapper.rs::tests` that feeds
   the truncation pipeline a corpus of mixed-script strings (ASCII + CJK +
   Latin + emoji), spanning every break the existing tests miss, and asserts
   no panic for any prefix width.
4. Add a top-of-file `// CONTRACT:` comment to
   [`text_system/line_wrapper.rs`](../../../crates/flui-core/src/text_system/line_wrapper.rs)
   pointing back to this ADR.

## References

### Upstream issues
- [zed-industries/zed#49860](https://github.com/zed-industries/zed/issues/49860) — CJK truncation panic.
- [zed-industries/zed#55214](https://github.com/zed-industries/zed/issues/55214) — text rasterization strategy. Referenced for disambiguation.

### Internal
- [docs/research/adr/ADR-001-invalidation-scope.md](ADR-001-invalidation-scope.md)
- [docs/research/adr/ADR-002-hover-active-invalidation.md](ADR-002-hover-active-invalidation.md)
- [docs/research/adr/ADR-003-color-alpha-pipeline.md](ADR-003-color-alpha-pipeline.md)
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — theme #3 (_Text rendering_), partial coverage by this ADR (slicing only; rasterization is separate).
