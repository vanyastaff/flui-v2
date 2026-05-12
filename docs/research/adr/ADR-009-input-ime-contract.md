# ADR-009: Input / IME pipeline — `doCommandBySelector` must honour selectors

**Date:** 2026-05-12
**Status:** Draft — contract only. No code changes land with this ADR.
**Scope:** `flui-core/src/platform.rs` (`InputHandler` trait),
`flui-core/src/platform/mac/window.rs` (Objective-C bridge to
`NSTextInputClient`).
**Drivers:** [zed-industries/zed#52550](https://github.com/zed-industries/zed/issues/52550).

## Context

The macOS text-input pipeline is a five-step contract:

1. Key event arrives at `keyDown:`.
2. The view forwards it through `inputContext.handleEvent:` (or
   `interpretKeyEvents:`).
3. The macOS key-binding manager consults `DefaultKeyBinding.dict` and the
   text-system's standard bindings, translating the key combo into a
   selector — e.g. `ctrl-W` → `deleteWordBackward:`.
4. The manager calls `doCommandBySelector:` on the input client with that
   selector.
5. The input client performs the action.

Every Apple-supplied text widget (`NSTextView`, `NSTextField`, the WebKit
text fields, etc.) implements step 5 properly. GPUI #52550 reports that
the GPUI's `doCommandBySelector:` implementation **drops the selector
parameter on the floor** and synthesises a key-down event back to the
flui-side keymap. The standard Cocoa bindings — `ctrl-W` for word delete,
`ctrl-A` for line start, etc., plus everything the user added to
`~/Library/KeyBindings/DefaultKeyBinding.dict` — are silently lost. Every
flui-v2 app inherits this bug because we ported the same handler verbatim.

This is not a fringe issue: power users routinely customise
`DefaultKeyBinding.dict` for Emacs-style or Vim-style editing in every
text field on the system. A text widget that ignores it is *not* a
native-feeling text widget.

## Current behaviour (verified)

References cite the commit this ADR is written against.

[`crates/flui-core/src/platform/mac/window.rs:2423`](../../../crates/flui-core/src/platform/mac/window.rs#L2423):

```rust
extern "C" fn do_command_by_selector(this: &Object, _: Sel, _: Sel) {
    let state = unsafe { get_window_state(this) };
    let mut lock = state.as_ref().lock();
    let keystroke = lock.keystroke_for_do_command.take();
    let mut event_callback = lock.event_callback.take();
    drop(lock);

    if let Some((keystroke, callback)) = keystroke.zip(event_callback.as_mut()) {
        let handled = (callback)(PlatformInput::KeyDown(KeyDownEvent {
            keystroke,
            is_held: false,
            prefer_character_input: false,
        }));
        state.as_ref().lock().do_command_handled = Some(!handled.propagate);
    }
}
```

Note the signature: the second-but-last `_: Sel` is the selector the
key-binding manager asked us to execute. The function name is the only
remaining clue — `do_command_by_selector` — that the selector ever
existed. The handler simply re-fires the original keystroke through the
flui-side keymap.

The contract `InputHandler` (`platform.rs:1293`) mirrors `NSTextInputClient`
for **insertion** (`replace_text_in_range`, `replace_and_mark_text_in_range`,
`marked_text_range`, etc.) but has no symmetric method for commands.

## Findings vs upstream issues

| Issue | Symptom | Repro in flui-v2 today |
|-------|---------|-------------------------|
| [zed-industries/zed#52550](https://github.com/zed-industries/zed/issues/52550) | macOS `DefaultKeyBinding.dict` entries and standard Cocoa text bindings are dropped because `doCommandBySelector:` ignores the selector. | **yes**. The function in `mac/window.rs:2423` is the same shape; the selector parameter is `_`. |

## Decision (contract)

1. **`doCommandBySelector:` must carry the selector through to the flui
   side.** The handler in `mac/window.rs` reads the selector argument and
   dispatches a typed `PlatformInput::EditorCommand(EditorCommand::…)`
   event, distinct from `PlatformInput::KeyDown`. Re-firing the original
   keystroke is acceptable as a **fallback** when no flui-side handler
   claims the selector.

2. **`InputHandler` grows a `handle_editor_command` method.** The Cocoa
   selectors (`moveLeft:`, `moveWordLeft:`, `selectLineStart:`,
   `deleteWordBackward:`, ...) translate to a flui enum
   `EditorCommand { MoveLeft, MoveWordLeft, SelectLineStart,
   DeleteWordBackward, ... }`. The enum is the platform-agnostic shape;
   the macOS bridge fills it from `NSSelector` names; future Windows /
   Linux IME bridges fill it from their equivalents (`WM_KEYDOWN` +
   accel tables, `xdg_input_method_v2` actions).

3. **The keymap is a fallback, not a bypass.** If a selector arrives and
   the input handler does not handle it, the bridge re-emits the
   keystroke as a key-down event. If the keymap also does not handle it,
   the macOS default of beep (or `NSSystemBeep`) is correct — we do not
   silently swallow.

4. **`InputHandler` semantics stay text-centric.** Window-level shortcuts
   (`cmd-Q`, `cmd-Tab`, `cmd-,`) are NOT routed through
   `doCommandBySelector:`; they continue to use the keymap path. The
   distinction is whose key-binding-manager owned the keystroke at the
   moment of dispatch.

5. **Cross-platform symmetry.** The Windows path (`WM_CHAR` /
   `WM_IME_COMPOSITION`) and the Wayland path
   (`text-input-unstable-v3` / `xdg_input_method_v2`) bind into the
   same `EditorCommand` enum. Linux IME is currently a `TODO`; the
   contract pre-empts the divergence.

## Consequences

- macOS text widgets built on flui-v2 honour `DefaultKeyBinding.dict`,
  matching every native Cocoa text widget on the system.
- Widgets that *want* the legacy "re-route through keymap" behaviour
  opt in by returning `false` from `handle_editor_command`.
- The Windows and Linux IME bridges, when written, have a target shape;
  they do not need to be designed twice.
- The `EditorCommand` enum is a new public type; once it is published it
  is semver-stable. The action items below propose seeding it from the
  Cocoa selector list to anchor it in something real.

## Out of scope (separate ADRs)

- **Marked text / composition rendering**. The `replace_and_mark_text_in_range`
  side of the contract is independent and already shaped correctly.
- **Right-to-left and bidi cursor motion semantics** (visual vs logical
  movement). UX/IME semantics; orthogonal.
- **Voice control / accessibility input**. Routed through the a11y
  pipeline, not `InputHandler`.
- **Window-level shortcuts** — explicitly excluded by decision point 4.

## Action items (tracked; no code lands with this ADR)

1. Define `EditorCommand` enum in `flui-core/src/platform.rs`. Seed it
   from the standard Cocoa text bindings (`StandardKeyBindingResponding`
   protocol): `moveForward`, `moveBackward`, `moveLeft`, `moveRight`,
   `moveWordForward`, `moveWordBackward`, `moveToBeginningOfLine`,
   `moveToEndOfLine`, `moveUp`, `moveDown`, `moveWordLeft`,
   `moveWordRight`, `moveToBeginningOfDocument`, `moveToEndOfDocument`,
   `pageUp`, `pageDown`, plus the select-variants and the delete-variants.
2. Add `InputHandler::handle_editor_command(&mut self, command:
   EditorCommand, window: &mut Window, cx: &mut App) -> bool` with a
   default `false` so existing implementors compile unchanged.
3. Rewrite [`mac/window.rs:2423`](../../../crates/flui-core/src/platform/mac/window.rs#L2423)
   to read the selector argument, look it up in a static table, and
   dispatch through `handle_editor_command`. Fallback to the current
   key-down path on unknown selectors or on `handle_editor_command`
   returning `false`.
4. Add tests that drive `ctrl-W` and `ctrl-A` through a mock input
   handler and assert `EditorCommand::DeleteWordBackward` /
   `EditorCommand::MoveToBeginningOfLine` is observed.

## References

### Upstream issues
- [zed-industries/zed#52550](https://github.com/zed-industries/zed/issues/52550) — `doCommandBySelector` ignores selectors.

### Internal
- [docs/research/adr/ADR-001-invalidation-scope.md](ADR-001-invalidation-scope.md)
- [docs/research/adr/ADR-008-window-chrome-contract.md](ADR-008-window-chrome-contract.md) — sibling on "platform-side flag is first line, flui-side enforcement is second" pattern.
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — theme #5 (_Input / focus / hit-testing_), partial coverage by this ADR.
