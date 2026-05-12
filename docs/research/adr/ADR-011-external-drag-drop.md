# ADR-011: External drag-and-drop — payloads beyond file paths

**Date:** 2026-05-12
**Status:** Draft — contract only. No code changes land with this ADR.
**Scope:** `flui-core/src/interactive.rs` (`FileDropEvent`, `ExternalPaths`),
per-platform DnD glue (`platform/{windows,mac,linux/{x11,wayland},web}`).
**Drivers:** [zed-industries/zed#52110](https://github.com/zed-industries/zed/issues/52110).

## Context

GPUI #52110 reports that a URL dragged from a browser onto a GPUI window
does nothing. flui-v2 reproduces the same gap: external drag-and-drop is
modelled as `FileDropEvent`, and the payload is `ExternalPaths
= SmallVec<[PathBuf; 2]>`. A URL is not a path, a text snippet is not a
path, an HTML fragment is not a path. The native drag-and-drop pasteboards
(`NSDraggingInfo`, `IDataObject`, Wayland `wl_data_offer`, X11 XDND, the
HTML5 `DataTransfer` object) all carry MIME-typed payloads. We see only
the subset that already arrives as a list of paths.

This ADR fixes the payload model. It does not redesign the dispatch
pipeline — the existing `FileDropEvent` arrival sequence
(`Entered → Pending → Submit | Exited`) is the right one and stays.

## Current behaviour (verified)

[`crates/flui-core/src/interactive.rs:608`](../../../crates/flui-core/src/interactive.rs#L608):

```rust
pub struct ExternalPaths(pub SmallVec<[PathBuf; 2]>);
```

[`crates/flui-core/src/interactive.rs:626`](../../../crates/flui-core/src/interactive.rs#L626):

```rust
pub enum FileDropEvent {
    Entered { position: Point<Pixels>, paths: ExternalPaths },
    Pending { position: Point<Pixels> },
    Submit  { position: Point<Pixels> },
    Exited,
}
```

The platform glue
([x11/client.rs:817](../../../crates/flui-core/src/platform/linux/x11/client.rs#L817), etc.)
constructs `FileDropEvent` from each platform's native source — but only
when the source advertises a paths payload (`text/uri-list`,
`CF_HDROP`, ...). A URL drop from a browser advertises `text/uri-list`
in some browsers (in which case it sometimes reaches us as a *file*
URL) and `text/x-moz-url` / `text/plain` in others (in which case the
event is dropped silently).

`ClipboardEntry::ExternalPaths` at
[`platform.rs:1837`](../../../crates/flui-core/src/platform.rs#L1837) is
the matching shape on the clipboard side; it has the same limitation.

## Findings vs upstream issues

| Issue | Symptom | Repro in flui-v2 today |
|-------|---------|-------------------------|
| [zed-industries/zed#52110](https://github.com/zed-industries/zed/issues/52110) | URL dragged from a browser is ignored by GPUI apps. | **yes**. `FileDropEvent` is the only entry point; URLs without a file path are dropped on the platform side. |

## Decision (contract)

1. **Rename `FileDropEvent` → `ExternalDropEvent`.** The name change is
   load-bearing: callers must stop assuming "file path" is the only
   payload. The variants stay; only the type name changes.

2. **Replace `paths: ExternalPaths` with `payload: ExternalDropPayload`.**
   The payload is a typed enum:

   ```rust
   pub enum ExternalDropPayload {
       Paths(SmallVec<[PathBuf; 2]>),
       Urls(SmallVec<[Url; 2]>),
       Text(String),
       Html { html: String, text: Option<String> },
       Mime { kind: String, bytes: Bytes },
       Mixed(SmallVec<[ExternalDropPayload; 4]>),
   }
   ```

   The exact variant list is the action item below; the contract is the
   shape: a sealed enum with file paths kept as a first-class variant so
   the existing call sites compile after a renaming pass.

3. **`Mixed` is the cross-platform truth.** Every native drag carries
   *one set of advertised MIME types*; we expose this faithfully and let
   the receiver choose. A drop from a browser may arrive as
   `Mixed[Urls(...), Text("https://..."), Html { ... }]`; the consumer
   takes the variant that matches its widget.

4. **`ClipboardEntry` and `ExternalDropPayload` share the same backing
   enum.** Paste and drop carry the same shape; one ADR, one enum.

5. **Per-platform glue is the integration point.** The platform layer
   advertises *which* MIME types it supports both for drops and clipboard;
   flui-core trusts the platform. No platform is expected to provide all
   variants — Wayland's data-device protocol is the worst case and we
   fall through gracefully.

6. **Drop targets advertise what they accept.** A `Div::on_drop`
   listener carries a `DropAcceptFilter`. A drop event is only
   delivered if at least one filter matches. This is the equivalent of
   `dropEffect` on the web; it gives the user the right cursor.

## Consequences

- Apps that accept URL drops work the day after the type rename plus
  variant addition. The implementation cost per platform is "wire up
  one more MIME type per supported variant".
- Existing `FileDropEvent::Entered { paths }` callers become
  `ExternalDropEvent::Entered { payload: ExternalDropPayload::Paths(...) }`
  — mechanical change, no behaviour drift.
- The `ClipboardEntry::ExternalPaths` site updates to the same enum.
- `DropAcceptFilter` is a new public type; the design is action item 3.

## Out of scope (separate ADRs)

- **In-process drag-and-drop** (`cx.active_drag`, the `Div::on_drag` /
  `on_drop` API). The internal drag carries a Rust value, not a MIME
  payload; that pipeline is independent.
- **Drag *out* of the app** (drag a node from our app into Finder /
  Explorer / a browser). Separate ADR; the contract there is the
  inverse (we *produce* a typed payload).
- **Clipboard `read_text` ergonomic helpers**. Cosmetic; not a contract.

## Action items (tracked; no code lands with this ADR)

1. Pick the final variant list for `ExternalDropPayload`. Start from
   the set Apple, Microsoft, X11 XDND, and the HTML5 `DataTransfer`
   spec all support: paths, URLs, plain text, HTML. Add `Mime { kind,
   bytes }` as the escape hatch.
2. Rename `FileDropEvent` → `ExternalDropEvent` and refactor the field
   `paths: ExternalPaths` to `payload: ExternalDropPayload`. Update
   `ClipboardEntry::ExternalPaths` to share the enum.
3. Specify `DropAcceptFilter`. Probable shape: a `Fn(&ExternalDropPayload)
   -> bool` per `Div::on_drop`; the macro `accepts_paths!() /
   accepts_urls!()` sugar can come later.
4. Wire the macOS path through `NSDraggingInfo::pasteboard.types`,
   the Windows path through `IDataObject::EnumFormatEtc`, the X11
   path through `XdndAware`, and the Wayland path through
   `wl_data_offer::accept`. Each is a separate PR keyed off the trait.
5. Add a manual test in the platform sandbox app: a div that prints
   the payload kind, used with URL drags from Firefox, Chrome, and
   Safari on macOS; from Firefox and Edge on Windows; from Firefox on
   Linux X11 and Wayland.

## References

### Upstream issues
- [zed-industries/zed#52110](https://github.com/zed-industries/zed/issues/52110) — external drag-and-drop not available for standalone GPUI apps; URL drops ignored.

### Internal
- [docs/research/adr/ADR-009-input-ime-contract.md](ADR-009-input-ime-contract.md) — sibling input contract.
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — theme #6 (_Drag-and-drop / custom paint_), partial coverage by this ADR (external DnD; custom paint is ADR-012).
