---
spec_id: S01c
title: lock-behavior-non-rendering
phase: I
depends_on: [S01a.1]
blocks: [S02]
status: draft
date: 2026-04-13
---

# S01c — lock-behavior-non-rendering

## Context

Third branch of the lock phase, in parallel with S01b and S01d. S01b
covers rendering output via golden tests; S01c covers everything else
that the migration steps (S02-S06) might silently regress.

The adversarial review of the original S01a draft enumerated nine
high-risk areas where existing functionality could vanish during the
platform extraction without any test catching it: input event routing
across nine `PlatformInput` variants, focus traversal, keyboard layout
mapping, clipboard read/write (including primary selection on Linux and
find-pasteboard on mac), window lifecycle (minimize/maximize/fullscreen),
IME composition, drag-and-drop, custom cursors, and animation frame
scheduling.

S01c pins the behaviors that can be exercised through `TestPlatform`
and `TestWindow` (i.e. without a real display server). IME composition,
drag-and-drop, and display-link timing require platform-specific
machinery that the test platform doesn't provide; those are explicitly
**deferred** to per-platform integration tests in S03/S04/S05 with the
risk documented here.

S01c is **pure test additions** — no production code changes, no API
changes. The only file outside `tests/` it touches is
`.github/workflows/ci.yml` if S01a.1's benchmark didn't already enable
`--features test-support` in the test job.

## Goals

1. Add a `tests/behavior/` directory with one test file per behavior
   category, each gated on `feature = "test-support"`.
2. Pin event dispatch behavior per `PlatformInput` variant on
   `TestPlatform`: synthetic input → `on_input` callback receives the
   right event, modifier state correct, button state correct.
3. Pin focus/tab-stop traversal: at least one positive case (focus
   moves forward across two stops) and one negative case (focus does
   not move past a barrier).
4. Pin keyboard layout selection through `PlatformKeyboardMapper`:
   synthetic scancode → `Keystroke` matches expected.
5. Pin clipboard read/write round-trip on `TestPlatform`: write a
   string, read it back, identity holds.
6. Pin window lifecycle methods: `minimize`, `is_minimized`, `maximize`,
   `is_maximized`, `toggle_fullscreen`, `is_fullscreen`, `close`. Each
   exercised once with assertion.
7. Pin scheduler determinism in a way the existing
   `scheduler/tests.rs` does not already cover (S01c first reads the
   existing tests and only adds what's missing).
8. Add a real example smoke job to CI that runs ~5 examples on Linux
   and macOS and verifies they actually opened a window and rendered a
   first frame — not just exit code 0.
9. Document explicitly which behaviors are NOT pinned by S01c and why
   (IME, drag-and-drop, display-link), so the deferred risk is auditable.

## Non-goals

- Not adding any production code or changing any API.
- Not running golden image comparisons. That's S01b.
- Not pinning IME composition behavior. The test platform has no IME
  hookup; pinning it requires real platform integration tests under
  S03/S04/S05.
- Not pinning drag-and-drop. Same reason.
- Not pinning vsync/display-link timing. Same reason.
- Not pinning any non-existent functionality (e.g. gesture arena
  doesn't exist yet — S07 will add it).
- Not adding new feature flags.
- Not running tests on Windows. S01a.4 is repairing the debug Windows
  build; once it lands, a follow-up adds Windows to CI. S01c verifies
  on Linux + mac only.
- Not running `examples/legacy/layer_shell` — it requires a Wayland
  compositor that GitHub runners don't provide. Documented as a known
  gap.

## Current state

### Test platform infrastructure

- `TestPlatform` at
  [`crates/flui-core/src/platform/test/platform.rs`](../../crates/flui-core/src/platform/test/platform.rs).
  Already implements `Platform` with deterministic dispatcher
  (`TestDispatcher`), in-memory clipboard, in-memory display, and
  in-memory windows.
- `TestWindow` at
  [`crates/flui-core/src/platform/test/window.rs`](../../crates/flui-core/src/platform/test/window.rs).
  Implements `PlatformWindow`. Has 6-8 `unimplemented!()` stubs (test
  windows are not backed by real platform windows) but supports the
  callback registration methods that S01c needs:
  `set_input_handler`, `take_input_handler`, `on_request_frame`,
  `on_input`, `sprite_atlas`.
- `TestDispatcher` at
  [`crates/flui-core/src/platform/test/dispatcher.rs`](../../crates/flui-core/src/platform/test/dispatcher.rs).
  Already has determinism support via `TestScheduler` and seeded RNG.

### Existing scheduler tests

[`crates/flui-core/src/scheduler/tests.rs`](../../crates/flui-core/src/scheduler/tests.rs)
already contains many determinism-focused tests including
`test_block_does_not_progress_same_session_foreground`,
`test_randomize_order`, and others. S01c's job is to **not duplicate**
these — the implementer reads the existing tests, identifies any gap
S01c needs, and adds only the delta.

### CI test step

`.github/workflows/ci.yml:118` runs `cargo test --workspace`. After
S01a.1's `test-support` benchmark, this either becomes
`cargo test --workspace --features test-support` (if benchmark
approved) or stays with a separate test step using
`--features test-support` (if benchmark deferred). S01c assumes the
former for simplicity but works either way.

### Examples

`crates/flui-core/examples/legacy/` contains 17 examples. S01c picks 5
representative ones for the smoke test:

- `hello_world.rs` — minimum viable app
- `window.rs` — explicit window creation
- `window_shadow.rs` — exercises shadow rendering (smoke for the
  rendering pipeline)
- `opacity.rs` — exercises layer opacity
- `tab_stop.rs` — exercises focus traversal

`layer_shell.rs` is excluded because it requires a Wayland compositor.
Documented in the spec.

## Design

### Test directory layout

```
crates/flui-core/tests/
├── behavior/
│   ├── common/
│   │   ├── mod.rs          # build TestPlatform + TestWindow harness
│   │   └── synthetic.rs    # synthetic PlatformInput constructors
│   ├── event_dispatch.rs   # one #[test] per PlatformInput variant
│   ├── focus_traversal.rs  # tab-stop tests
│   ├── keyboard_layout.rs  # PlatformKeyboardMapper round-trip
│   ├── clipboard.rs        # write/read round-trip
│   ├── window_lifecycle.rs # minimize/maximize/fullscreen
│   └── scheduler_extra.rs  # only what isn't in scheduler/tests.rs
└── golden/                 # (created by S01b, mentioned for context)
```

All files start with `#![cfg(feature = "test-support")]` and use
`flui_core::*;` to access the test platform.

### 1. Event dispatch per `PlatformInput` variant

[`crates/flui-core/src/platform/input.rs`](../../crates/flui-core/src/platform/input.rs)
defines `PlatformInput` (search for `pub enum PlatformInput`).
Variants (verify via grep at implementation time, expected list):

- `MouseDown { position, button, click_count, modifiers, ... }`
- `MouseUp { position, button, modifiers, ... }`
- `MouseMove { position, modifiers, ... }`
- `MouseExit { position, ... }`
- `KeyDown(KeyDownEvent)` with `keystroke` and `is_held`
- `KeyUp(KeyUpEvent)` with `keystroke`
- `ModifiersChanged(ModifiersChangedEvent)` with `modifiers`
- `ScrollWheel { position, delta, modifiers, phase, ... }`
- `FileDrop(FileDropEvent)` with paths

**Test pattern** (one per variant):

```rust
#[test]
fn dispatch_mouse_down() {
    let mut cx = TestAppContext::new();
    let window = cx.add_window(/* ... */);
    let received = Rc::new(RefCell::new(Vec::new()));

    window.update(&mut cx, |_, window, _| {
        let received = received.clone();
        window.platform_window.on_input(Box::new(move |input| {
            received.borrow_mut().push(input.clone());
            DispatchEventResult::Propagate
        }));
    });

    let event = synthetic::mouse_down(
        Point::new(px(10.), px(20.)),
        MouseButton::Left,
        Modifiers { control: true, ..Default::default() },
    );
    window.platform_window.simulate_input(event.clone());

    let received = received.borrow();
    assert_eq!(received.len(), 1);
    match &received[0] {
        PlatformInput::MouseDown(e) => {
            assert_eq!(e.position, Point::new(px(10.), px(20.)));
            assert_eq!(e.button, MouseButton::Left);
            assert!(e.modifiers.control);
        }
        other => panic!("expected MouseDown, got {other:?}"),
    }
}
```

Repeated for every variant. **Open question**: does
`TestWindow::simulate_input` exist? If not, S01c either adds it to
`platform/test/window.rs` (ONE small production-code change) or uses
the `on_input` callback directly without a separate "simulate" entry
point. Resolution at implementation time.

### 2. Focus / tab-stop traversal

[`crates/flui-core/src/tab_stop.rs`](../../crates/flui-core/src/tab_stop.rs)
is `pub(crate)` — but its types are reachable through `Window` /
`Element` APIs that S01c uses through the test platform.

**Tests:**

- `tab_forward_two_stops` — create a window with two focusable
  elements, call `Window::focus_next`, verify focus moved to the second.
- `tab_backward` — same but with `focus_previous`.
- `tab_does_not_escape_focus_scope` — create a focus scope with one
  element, call `focus_next`, verify focus stays on the same element.
- `focus_initial` — verify the first focusable element receives focus
  by default.

These tests verify the `tab_stop.rs` invariants without exposing it
publicly.

### 3. Keyboard layout / mapper round-trip

[`crates/flui-core/src/platform/keyboard.rs`](../../crates/flui-core/src/platform/keyboard.rs)
defines `PlatformKeyboardLayout` and `PlatformKeyboardMapper` traits.
The test platform provides `DummyKeyboardMapper` at
`platform/keyboard.rs:27`.

**Tests:**

- `dummy_mapper_letter_key` — synthetic scancode for 'A' produces
  `Keystroke { key: "a", ... }`.
- `dummy_mapper_modifier_combo` — synthetic scancode + modifier mask
  produces the expected `Keystroke` with modifiers.
- `dummy_mapper_keystroke_parse_round_trip` — `Keystroke::parse("ctrl-shift-k")`
  followed by `to_string()` returns the same text.

### 4. Clipboard round-trip

`TestPlatform` has in-memory clipboard via `read_from_clipboard` /
`write_to_clipboard`. Trait at `platform.rs:296` (verify line at impl
time).

**Tests:**

- `clipboard_string_round_trip` — write `"hello world"`, read back,
  assert identity.
- `clipboard_overwrite` — write A, write B, read, assert B.
- `clipboard_empty` — fresh platform, read returns
  `ClipboardItem::default()` or `None`.
- `clipboard_with_metadata` — write a `ClipboardItem` with text + a
  metadata blob; read back; assert both fields preserved.

Linux primary selection and macOS find-pasteboard are NOT pinned by
TestPlatform (those are real OS APIs). Documented as deferred to
S03/S04 platform-specific tests.

### 5. Window lifecycle

`TestWindow` exposes `is_active`, `is_hovered`, `is_maximized`,
`is_fullscreen`, `is_minimized` and the `minimize`, `maximize`,
`toggle_fullscreen`, `close` setters via the `PlatformWindow` trait.
Most are `unimplemented!()` in the test impl today (4 of the 8
known stubs).

**Tests:**

- `lifecycle_initial_state` — fresh window: not maximized, not
  fullscreen, not minimized.
- `lifecycle_maximize_toggle` — call `maximize`; assert
  `is_maximized()` returns true.
- `lifecycle_minimize_toggle` — same for minimize.
- `lifecycle_fullscreen_toggle` — same for fullscreen.
- `lifecycle_close_marks_window` — call `close`; verify the window's
  `should_close` callback fires and the window is removed from the
  platform's window stack.

**Open question (and known production-code change candidate):** if the
`is_maximized` / `is_minimized` / `is_fullscreen` getters are
`unimplemented!()` in `TestWindow` today, S01c either:

- (a) adds minimal stub implementations that track state in a
  `RefCell<TestWindowState>` (1 small production change), or
- (b) marks these tests as `#[ignore]` with a TODO, accepts the gap,
  and the lifecycle pinning is deferred.

Recommendation: **(a)** — a few dozen lines of production change to
make the test platform actually testable is a fair price. It's purely
in-memory state tracking.

If (a) is taken, those stubs are removed from the
`docs/fixtures/platform-expected-stubs.toml` from S01a.1 in the same
commit.

### 6. Scheduler extras

Read `scheduler/tests.rs` first. List existing test names. Identify
gaps:

- Determinism across `spawn_realtime` boundary?
- Determinism with mixed foreground/background priorities?
- Determinism after `Scheduler::advance` to a specific instant?

Add ONLY the missing tests. If `scheduler/tests.rs` already covers
everything S01c would add, this category becomes a 1-line "no new
tests, existing coverage validated" entry in the test log.

### 7. Real example smoke

New CI step (`.github/workflows/ci.yml` test job, both Linux and macOS
matrix entries):

```yaml
- name: Example smoke
  env:
    VK_ICD_FILENAMES: /usr/share/vulkan/icd.d/lvp_icd.x86_64.json
    WGPU_POWER_PREF: low
    FLUI_SMOKE_TEST: "1"   # signals examples to exit after first frame
  run: |
    set -e
    for example in hello_world window window_shadow opacity tab_stop; do
      timeout 30s cargo run --example "$example" --features test-support
    done
```

The `FLUI_SMOKE_TEST=1` env var is read by a small helper in
`examples/legacy/common.rs` (or each example individually) and causes
the example to:

1. Verify the window opened (i.e. `Window::content_size()` is non-zero).
2. Schedule one frame draw.
3. After the first frame's `on_request_frame` callback fires, exit 0.

This is **not** a "did the binary not crash" check — it actively
verifies the rendering loop ran one cycle. Without it, a regression
that prevents windows from opening at all could land green (the binary
would exit 0 after `Application::run` returns due to no events).

Implementation note: this requires a small change to one or more
example files to read the env var. That's the only S01c production-code
change outside the optional `TestWindow` lifecycle-state addition.

The `layer_shell` example is excluded; documented as a known CI gap.

### 8. Deferred behaviors (documented gaps)

S01c does NOT pin:

- **IME composition.** Requires real platform IME hooks. Deferred to
  per-platform tests in S03 (Wayland/X11), S04 (mac), S05 (Windows).
- **Drag-and-drop.** Same.
- **Custom cursors at runtime.** Cursor style propagation through
  TestPlatform exists but rendering the cursor doesn't.
- **Display-link timing.** Vsync / refresh rate is platform hardware.
  Deferred.
- **Mac NSServices menu integration.** Requires real `NSApplication`
  delegate. Deferred to S04.
- **Linux primary selection / macOS find-pasteboard.** Real OS APIs
  not in TestPlatform. Deferred to S03/S04.
- **Wayland xdg-activation / session-lock / fractional scaling
  protocols.** Deferred to S03 Wayland integration tests.

Each gap is recorded in `docs/lock-coverage-gaps.md` (new file
created by S01c) so future work can audit what's covered and what
isn't.

## API surface

**Zero new public items in `flui-core`.**

If the optional `TestWindow` lifecycle-state addition is taken (Step 5
option a), the `pub fn` signatures on `TestWindow` are unchanged — the
state tracking is in private fields.

`docs/lock-coverage-gaps.md` is documentation, not API.

## Migration / Compatibility

Zero breakage. All changes are test additions.

If `TestWindow` gains lifecycle state tracking, existing tests that
already construct `TestWindow` will see slightly different behavior on
methods that were previously `unimplemented!()` — but those methods
were panicking before, so no test could have depended on the panic
behavior.

## Testing strategy

The spec is itself the test addition; the testing strategy is meta:

1. **All new tests pass** on Linux + macOS in CI.
2. **All new tests fail loudly** if a relevant production-code change
   breaks the pinned behavior. The implementer verifies this manually
   for at least 3 categories (e.g. break the clipboard read, break
   focus traversal, break event dispatch — confirm each new test
   panics with the expected diff).
3. **CI run time delta** is measured before/after S01c and recorded.
   If S01c adds >2 minutes to the CI runtime per matrix entry, the
   test selection is too aggressive; trim and re-measure.
4. **Stub inventory delta** from S01a.1 reflects any `unimplemented!()`
   sites that were filled in (lifecycle state).
5. **Example smoke step** runs and is green; manual log check for the
   "first frame rendered" message.

## Open questions

- **`TestWindow::simulate_input` existence** — verify at impl time.
  If it exists, use it; if not, the on_input callback is the entry
  point.
- **Lifecycle state in `TestWindow`** — option (a) adds in-memory
  tracking, option (b) defers. Recommendation (a).
- **Existing `scheduler/tests.rs` coverage** — read first, add only
  the delta. May be zero.
- **`FLUI_SMOKE_TEST` env var name** — pick a project-consistent
  prefix. `FLUI_*` is the natural choice; verify no existing
  conflicts.
- **Example helper file location** — `examples/legacy/common.rs`
  doesn't exist today. Either create it (one new file, shared by all
  smoke-tested examples) or duplicate the env-var check in each
  example file. Recommendation: shared helper.
- **Mac `find-pasteboard` and Linux primary selection** — could in
  principle be tested through `TestPlatform` if the test platform
  added a separate in-memory primary selection. Worth doing? For
  S01c, no — that's an extension to TestPlatform that's its own work.

## Done criteria

- [ ] `tests/behavior/event_dispatch.rs` exists with one `#[test]` per
      `PlatformInput` variant (verified via grep on the variant list).
- [ ] `tests/behavior/focus_traversal.rs` exists with the four named
      tab-stop tests.
- [ ] `tests/behavior/keyboard_layout.rs` exists with the keyboard
      mapper round-trip tests.
- [ ] `tests/behavior/clipboard.rs` exists with the four clipboard
      tests.
- [ ] `tests/behavior/window_lifecycle.rs` exists with the lifecycle
      tests.
- [ ] `tests/behavior/scheduler_extra.rs` exists, even if its only
      content is a one-line `// All scheduler determinism is covered
      by scheduler/tests.rs:N..M` comment plus a no-op test.
- [ ] All new tests pass on Linux and macOS CI with
      `--features test-support`.
- [ ] CI test step has the new "Example smoke" step running 5 examples
      with first-frame verification on Linux + macOS.
- [ ] `examples/legacy/{hello_world,window,window_shadow,opacity,tab_stop}.rs`
      respect `FLUI_SMOKE_TEST=1` and exit 0 after first frame.
- [ ] `docs/lock-coverage-gaps.md` exists and lists every behavior the
      spec deferred (IME, drag-and-drop, display-link, find-pasteboard,
      primary selection, mac NSServices, Wayland special protocols,
      `layer_shell` example).
- [ ] If `TestWindow` lifecycle state was added, those
      `unimplemented!()` sites are removed from
      `docs/fixtures/platform-expected-stubs.toml` in the same PR.
- [ ] `cargo xtask check-stubs` from S01a.1 green.
- [ ] CI runtime delta recorded in test log; under +2 minutes per
      matrix entry.
- [ ] Manual break-and-verify done for at least 3 test categories;
      results in test log.

## Test log

To be filled during implementation.

### Test counts added

| Category | New tests | Files |
|---|---|---|
| event_dispatch | TBD (one per variant) | tests/behavior/event_dispatch.rs |
| focus_traversal | 4 | tests/behavior/focus_traversal.rs |
| keyboard_layout | 3 | tests/behavior/keyboard_layout.rs |
| clipboard | 4 | tests/behavior/clipboard.rs |
| window_lifecycle | 5 | tests/behavior/window_lifecycle.rs |
| scheduler_extra | 0-N | tests/behavior/scheduler_extra.rs |

### CI runtime delta

| Job | Before | After | Delta |
|---|---|---|---|
| test (Linux) | TBD | TBD | TBD |
| test (macOS) | TBD | TBD | TBD |

### Break-and-verify

| Category | Mutation introduced | Test caught it? |
|---|---|---|
| clipboard | broke `write_to_clipboard` to no-op | TBD |
| focus | broke `focus_next` to noop | TBD |
| event_dispatch | dropped MouseDown variant from match | TBD |

### Stub inventory delta

```
$ cargo xtask check-stubs --bless
# Expected delta: TestWindow lifecycle-state methods removed from inventory
# Captured: TBD
```

## Follow-ups after S01c lands

- **S02 unblocked** on the behavior front for everything that goes
  through `TestPlatform`.
- **Per-platform integration tests** for the deferred categories — to
  be folded into S03, S04, S05 as part of each migration's "this still
  works on the real platform" verification.
- **`docs/lock-coverage-gaps.md` is the canonical list** of what's NOT
  pinned. Future specs should append to it whenever they add coverage
  or discover new gaps.
- **TestPlatform extensions** for primary selection and find-pasteboard
  could be a small cleanup spec at any point.
