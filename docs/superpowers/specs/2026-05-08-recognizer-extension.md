# Recognizer extension guide (S07.5)

**Audience:** contributors adding a new gesture recognizer to `flui-core`
or to a downstream crate.

**Prerequisites:**

- Read `docs/superpowers/specs/2026-05-06-S07-gesture-arena-design.md`
  for the arena, sanitizer, and dispatch architecture.
- Skim `crates/flui-core/src/gesture/recognizer.rs` for the
  `GestureRecognizer` and `RecognizerLifecycle` traits.
- Skim `crates/flui-core/src/gesture/binding.rs` for how registration
  is wired (`GestureBinding::register_recognizer`).

This guide is the canonical recipe for adding a new recognizer such
that it integrates with the per-window settings, the arena
back-channel, and the `arena.hold` semantics — all without touching
`Window::dispatch_event` again. It supersedes the ad-hoc patterns
that existed in the merged S07 PR and codifies the seam introduced by
S07.5 (`RecognizerLifecycle` + `register_recognizer`).

---

## Adding a new recognizer step-by-step

1. **Create a new module** under
   `crates/flui-core/src/gesture/recognizers/<name>.rs` and add a
   `pub mod <name>;` line in `recognizers/mod.rs`.

2. **Define the recognizer struct** with `#[non_exhaustive]` and
   public threshold fields:

   ```rust
   #[non_exhaustive]
   pub struct MyRecognizer {
       pub on_my_gesture: Option<Box<dyn FnMut(MyDetails, &mut crate::Window, &mut crate::App)>>,
       pub button: PointerButtons,
       /// Read from `GestureSettings::touch_slop` at construction.
       pub slop: Pixels,
       // private state ...
   }
   ```

3. **Implement `GestureRecognizer`** — at minimum:

   - `as_any_mut(&mut self) -> &mut dyn Any` returning `self` (object-safety).
   - `name(&self) -> &'static str` for the `kv` log schema.
   - `add_pointer(&mut self, pid, event)` — initial state setup.
     `event` is a `DeliveredEvent<'_>` (S07.5b); read
     `event.local_position` for `down_position`-style state and
     `event.kind()`/`event.buttons()`/etc. for non-position fields.
   - `handle_event(&mut self, event: DeliveredEvent<'_>, window, cx) ->
     GestureDisposition` — the per-event state machine. Same
     conventions as `add_pointer`: `event.local_position` for
     slop/distance/drag-delta computation,
     `event.global_position()` for callback `global_position`
     payloads. **Never** read the underlying event's `position`
     field directly — the verification grep
     `grep "event\.position" crates/flui-core/src/gesture/recognizers/`
     must return zero hits.
   - `sweep_accepted(&mut self, ...)` — what to do when sweep declares
     this recognizer the captain.
   - `rejected(&mut self, ...)` — clean up; **must reset to a fresh
     state**, never leak partial state.
   - Override `allowed_buttons_filter(&self) -> Option<&AllowedButtonsFilter>`
     to return `self.allowed_buttons_filter.as_ref()` if you ship a
     filter field. The default body returns `None`.
   - Override `lifecycle(&mut self) -> Option<&mut dyn RecognizerLifecycle>`
     to return `Some(self)` if any lifecycle hook applies (see below).

4. **Implement `RecognizerLifecycle`** for the lifecycle hooks you
   need (see "When to use RecognizerLifecycle" below). Default bodies
   are no-ops, so opting in to one method does not require
   implementing the others.

5. **Add a fluent builder** to `crates/flui-core/src/gesture/mod.rs`
   following the `__internal_on_*` pattern:

   ```rust
   #[doc(hidden)]
   pub fn __internal_on_my_gesture(
       iv: &mut crate::elements::Interactivity,
       f: impl FnMut(recognizers::MyDetails, &mut crate::Window, &mut crate::App) + 'static,
   ) {
       let r = find_or_push(__recognizers_mut(iv), || {
           recognizers::MyRecognizer::new(&GestureSettings::default())
       });
       r.on_my_gesture = Some(Box::new(f));
   }
   ```

6. **Surface the builder** on `InteractiveElement` in
   `crates/flui-core/src/elements/div.rs`, calling the
   `__internal_on_my_gesture` helper.

7. **Re-export public types** from `gesture/mod.rs` (`pub use
   recognizers::MyRecognizer;` etc.) and from `lib.rs` if the type
   should be reachable via the canonical flat path
   `flui_core::MyRecognizer`.

8. **Write tests:** unit tests in the recognizer module, an arena
   property test in `arena.rs::tests` if the recognizer changes the
   arena's invariants, and an end-to-end integration test in
   `crates/flui-core/tests/gesture_dispatch_integration.rs` that
   drives `simulate_*` through `Window::dispatch_event`.

---

## When to use `RecognizerLifecycle`

`RecognizerLifecycle` is a sibling trait to `GestureRecognizer`,
reachable only through the optional `GestureRecognizer::lifecycle()`
accessor. Each method has a default no-op body. Override one of:

| Hook                          | Override when …                                                                                                               | Reference impl                  |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------- | ------------------------------- |
| `needs_back_channel`          | the recognizer must declare itself winner from outside `handle_event` (timer fire, async I/O completion).                     | `LongPressGestureRecognizer`    |
| `set_arena_back_channel(pid, bc, idx)` | (always paired with `needs_back_channel = true`) store the supplied [`ArenaBackChannel`] + per-pointer `(pid, idx)` slot for later. Multi-pointer recognizers must keep one slot per pointer (see LongPress migration in S07.5b). | `LongPressGestureRecognizer` |
| `needs_arena_hold`            | the gesture spans multiple Down/Up sequences and the arena must stay open past the first sweep.                               | `DoubleTapGestureRecognizer`    |
| `configure_settings`          | the recognizer's thresholds come from per-window `GestureSettings`. Override to copy from `settings` into the recognizer.     | every recognizer                |
| `allowed_buttons_filter` (on `GestureRecognizer`) | the recognizer ships a `pub allowed_buttons_filter: Option<AllowedButtonsFilter>` field. Surface it through the trait method so `register_recognizer` can gate admission (Decision D10). The fluent `with_allowed_buttons_filter(closure)` builder lives on the recognizer struct itself, not on `RecognizerLifecycle`. | every recognizer with the field |

If none of these apply, leave `lifecycle()` returning `None` (the
trait default). Tests, mocks, and recognizers that build their own
state from constructor arguments can opt out entirely.

**Why this shape rather than a single supertrait?** Adding methods
to `GestureRecognizer` would break every downstream impl. A separate
trait reachable through a default-`None` accessor is purely additive.

---

## Worked examples

### Tap (simple — settings only)

```rust
impl GestureRecognizer for TapGestureRecognizer {
    // ... add_pointer / handle_event / sweep_accepted / rejected ...

    fn lifecycle(&mut self) -> Option<&mut dyn RecognizerLifecycle> {
        Some(self)
    }
}

impl RecognizerLifecycle for TapGestureRecognizer {
    fn configure_settings(&mut self, settings: &GestureSettings) {
        self.touch_slop = settings.touch_slop;
    }
}
```

### LongPress (back-channel — async timer-driven acceptance)

```rust
impl RecognizerLifecycle for LongPressGestureRecognizer {
    fn needs_back_channel(&self) -> bool {
        true
    }

    fn set_arena_back_channel(
        &mut self,
        pointer_id: PointerId,
        back_channel: ArenaBackChannel,
        entry_index: usize,
    ) {
        self.arena_back_channel = back_channel;
        // S07.5b: per-pointer storage. Single-shot LongPress only
        // ever holds one in-flight pointer in practice, so the
        // SmallVec inline budget of 1 covers the common case
        // without a heap allocation. Multi-pointer recognizers
        // (S07.6 MultiTap) inherit the same shape.
        self.pointer_indexes
            .retain(|(pid, _)| *pid != pointer_id);
        self.pointer_indexes.push((pointer_id, entry_index));
    }

    fn configure_settings(&mut self, settings: &GestureSettings) {
        self.timeout = settings.long_press_timeout;
        self.slop = settings.long_press_slop;
        self.timer_budget = settings.long_press_timer_budget;
    }
}
```

The async timer pattern (inside `handle_event` on `Down`):

```rust
let timeout = self.timeout;
let back_channel = self.arena_back_channel.clone();
// Look up this pointer's entry slot recorded during
// `set_arena_back_channel`.
let entry_index = self
    .pointer_indexes
    .iter()
    .find(|(pid, _)| *pid == event.pointer_id())
    .map(|(_, idx)| *idx);
let window_handle = window.window_handle();
// Use BackgroundExecutor::timer (not smol::Timer::after) so test
// schedulers' virtual-clock advance_clock wakes the timer.
let timer_future = cx.background_executor().timer(timeout);
self.timer = Some(cx.spawn(async move |async_cx| {
    timer_future.await;
    let Some(idx) = entry_index else { return; };
    let _ = async_cx.update_window(window_handle, |_, window, cx| {
        // ... mark accepted, fire user callback ...
        back_channel.declare_winner(pid, idx, window, cx);
    });
}));
```

The `Task<()>` is stored on the recognizer; dropping the recognizer
drops the task, cancelling the future. This is the **drop-cancel**
contract: every lifecycle path that resolves the recognizer
(`Move > slop`, `Up`, `Cancel`, `rejected`) must clear the timer
field.

### DoubleTap (hold — sweep deferred until second tap)

```rust
impl RecognizerLifecycle for DoubleTapGestureRecognizer {
    fn needs_arena_hold(&self) -> bool {
        true
    }

    fn configure_settings(&mut self, settings: &GestureSettings) {
        self.touch_slop = settings.touch_slop;
        self.double_tap_timeout = settings.double_tap_timeout;
        self.double_tap_min_time = settings.double_tap_min_time;
    }
}
```

When `needs_arena_hold` returns `true`, the dispatcher in
`Window::dispatch_event` calls `arena.hold(pointer_id)` after
registration and schedules an `arena.release` timer keyed by
`double_tap_timeout`. The recognizer's state machine must handle
the `(AwaitSecond, Down)` transition inside `handle_event`, since
`add_pointer` only runs at registration time and the second `Down`
is dispatched through the existing held arena entry.

---

## Threshold-field conventions

- All threshold fields are `pub` and live directly on the recognizer
  struct (or on a private `inner: DragImpl`-style helper for
  multi-recognizer families). Downstream crates can read/write them
  post-construction, including through fluent `with_*` builders.
- Defaults come from `GestureSettings` at construction time
  (`Self::new(settings)`). The values stay sticky until either:
  - The user mutates them post-construction (idiomatic for tuning).
  - `RecognizerLifecycle::configure_settings` runs at registration
    time (called by `GestureBinding::register_recognizer`), copying
    the per-window overrides into the recognizer's fields.
- The fluent `__internal_on_*` helpers always pass
  `GestureSettings::default()` to `Self::new`. The real per-window
  values are applied later via `configure_settings`. Do not try to
  thread settings into the construction call directly — `render()`
  has no `&Window` reference at the right moment.
- **Pressure thresholds operate on `PressureSample::normalize()`,
  never raw `value`** (S07.5b). A Wacom pen reports
  `PressureSample { value: 4096.0, min: 0.0, max: 8192.0 }`, while
  Force Touch reports `PressureSample { value: 0.5, min: 0.0,
  max: 1.0 }`. Both produce the same `0.5` after `normalize()`, so
  a recognizer threshold of `0.4` means the same physical effort
  on every device. Comparing raw `value` against a fixed constant
  silently makes the threshold device-dependent.

---

## Test discipline

Three kinds of tests, in increasing scope:

1. **Unit tests** in the recognizer module:
   - Cover every `handle_event` arm (`Down`, `Move > slop`, `Move <
     slop`, `Up`, `Cancel`).
   - Lock the threshold fields stay public via a
     `*_threshold_fields_are_settable` compile-time canary.
   - Use synthetic `PointerEvent` constructors and a
     `TestAppContext` for the `&mut Window` / `&mut App` arguments.

2. **Property tests** in `arena.rs::tests` — only needed if the
   recognizer changes the arena's invariants (new `is_held` /
   `winner` interactions, new merge semantics). The S07.5 baseline
   tests (`prop_merge_by_pointer_id_no_duplicates`,
   `prop_active_pointer_count_upper_bound`,
   `prop_hold_release_symmetry`) cover the canonical cases; new
   recognizers usually do not need to add to this set.

3. **End-to-end integration tests** in
   `crates/flui-core/tests/gesture_dispatch_integration.rs`:
   - Paint a `div().on_my_gesture(...)` view.
   - Drive raw `MouseDownEvent` / `MouseUpEvent` / `MouseMoveEvent`
     through `simulate_*`.
   - Assert the user-visible callback fires.
   - For async timer paths, use
     `cx.executor().advance_clock(Duration::...)` plus
     `cx.run_until_parked()` to drive the virtual clock.

The end-to-end test is the single most important regression lock:
it covers the entire `paint → pending_recognizers →
register_recognizer → arena.dispatch → callback` chain, so a
breakage anywhere in that path fails CI.

---

## Common pitfalls

- **Calling `cx.stop_propagation()` from inside `handle_event`.**
  Forbidden by the trait contract — the arena declares winners via
  `GestureDisposition::Accepted`, not via propagation control. The
  dispatcher resets `cx.propagate_event = true` between the arena
  pass and the raw `on_mouse_*` chain.
- **Storing the arena `Rc` directly instead of the `Weak`-backed
  `ArenaBackChannel`.** A strong `Rc` would form a cycle through
  the `arena → entries → recognizer → arena_back_channel → arena`
  path and leak the binding forever. `ArenaBackChannel` is `Weak`
  internally for exactly this reason.
- **Holding the recognizer's `Rc` from inside its own state.** Same
  cycle problem — recognizers must not store an `Rc` to themselves.
  If a timer needs to call back into the recognizer, look it up via
  the arena (`back_channel.upgrade()` →
  `arena.borrow().arenas[…].entries[idx].recognizer`).
- **`smol::Timer::after` instead of
  `BackgroundExecutor::timer`.** `smol::Timer` observes the wall
  clock and never fires under `TestAppContext::executor().advance_clock`.
  Always go through `cx.background_executor().timer(duration)`.
- **`add_pointer` doing state-machine work that `handle_event`
  cannot redo.** The dispatcher only calls `add_pointer` at
  registration time. Any state transition that needs to happen on
  subsequent `Down`/`Move`/`Up` events must live in `handle_event`.

---

## Cross-references

- `crates/flui-core/src/gesture/recognizer.rs` — `GestureRecognizer`
  + `RecognizerLifecycle` traits.
- `crates/flui-core/src/gesture/binding.rs` —
  `GestureBinding::register_recognizer` + `schedule_arena_release`.
- `crates/flui-core/src/gesture/arena.rs` — `ArenaBackChannel`,
  `GestureArenaManager::declare_winner`, `hold`, `release`,
  `merge_by_pointer_id`.
- `crates/flui-core/src/gesture/recognizers/long_press.rs` — full
  back-channel reference impl.
- `crates/flui-core/src/gesture/recognizers/double_tap.rs` —
  arena-hold reference impl with the `(AwaitSecond, Down)`
  transition.
- `crates/flui-core/tests/gesture_dispatch_integration.rs` —
  end-to-end test template.
