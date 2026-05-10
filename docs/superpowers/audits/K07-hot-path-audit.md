# K07 Hot-Path Audit

Date: 2026-05-10

## Question

Does the K07 `AppCell::try_borrow_mut` / `AppCell::borrow_mut` path become reachable from per-frame paint, `Window::draw`, `Window::dispatch_event`, `Element::paint`, or observer dispatch hot paths?

## Conclusion

No AppCell `try_borrow_mut` call is reachable from paint or dispatch hot paths.

`Window::draw`, `Window::dispatch_event`, and `Element::paint` receive `&mut App` directly. Their `borrow_mut()` hits are pre-existing local `RefCell` state borrows (`WindowInner`, element state, recognizers, scroll state, input-rate tracker), not `AppCell`.

The remaining production AppCell `borrow_mut()` callsites are startup/configuration, platform callbacks, menu command handlers, and `AsyncApp` entrypoints. They are callback/user-action frequency, not per-frame paint or pointer/key dispatch frequency.

## Commands

```text
rg -n "try_borrow_mut|\.borrow_mut\(\)" crates/flui-core/src/app.rs crates/flui-core/src/app crates/flui-core/src/window.rs crates/flui-core/src/elements crates/flui-core/src/subscription.rs crates/flui-core/src/platform crates/flui-core/src/reentrancy.rs
rg -n "pub fn draw|pub fn dispatch_event|fn paint\(|with_element_state|borrow_mut\(\)" crates/flui-core/src/window.rs crates/flui-core/src/elements
rg -n "app\.borrow_mut\(\)|self\.app\.borrow_mut\(\)|self\.0\.borrow_mut\(\)|try_borrow_mut\(" crates/flui-core/src/app.rs crates/flui-core/src/app/*.rs crates/flui-core/src/platform/app_menu.rs crates/flui-core/src/reentrancy.rs crates/flui-core/src/app/cell.rs
```

Note: the last command's `app/*.rs` glob is not portable under PowerShell `rg`; the same scope was covered by the first command using the `crates/flui-core/src/app` directory.

## Callgraph Table

| File / line | Caller chain | Hot-path reachable? | Expected frequency |
|---|---|---:|---|
| `crates/flui-core/src/app/cell.rs:123` | `AppCell::try_borrow_mut` implementation | No direct caller; primitive only | n/a |
| `crates/flui-core/src/app/async_context.rs:90` | `AsyncApp::update_window` -> upgraded `Weak<AppCell>` -> `try_borrow_mut` | No | async user entrypoint |
| `crates/flui-core/src/app/async_context.rs:39,45,55,65,126,135,152,168,182,204,213,222,236,247` | `AsyncApp` public methods -> upgraded `Weak<AppCell>` -> `borrow_mut` | No | async user entrypoint |
| `crates/flui-core/src/app.rs:195,205,214,249` | `Application` builder/configuration methods | No | app startup/config |
| `crates/flui-core/src/app.rs:251` | platform reopen callback -> upgraded `Weak<AppCell>` -> `borrow_mut` | No | platform lifecycle callback |
| `crates/flui-core/src/app.rs:809,815,829,841` | `App::new_app` platform observer callbacks -> upgraded app -> `borrow_mut` | No | platform lifecycle/layout-change callback |
| `crates/flui-core/src/platform/app_menu.rs:336,346,355` | menu command validation/dispatch -> upgraded app -> `borrow_mut().update(...)` | No | menu action / menu validation |
| `crates/flui-core/src/reentrancy.rs:299` | test-only direct AppCell contention test | No | test-only |
| `crates/flui-core/src/window.rs:2336` | `Window::draw(&mut self, cx: &mut App)` | No AppCell access | per-frame |
| `crates/flui-core/src/window.rs:4210` | `Window::dispatch_event(&mut self, ..., cx: &mut App)` | No AppCell access | input dispatch |
| `crates/flui-core/src/elements/**:paint` | `Element::paint(..., window: &mut Window, cx: &mut App)` | No AppCell access | per-frame |

## Hot-Path `borrow_mut()` Hits That Are Not AppCell

Representative examples:

- `crates/flui-core/src/window.rs:128-156` borrow `WindowHandleInner`.
- `crates/flui-core/src/window.rs:4525`, `:4537`, `:4640` borrow recognizer/input-rate local state during dispatch.
- `crates/flui-core/src/elements/div.rs`, `list.rs`, `text.rs`, and `uniform_list.rs` borrow element-local state handles during layout/paint.
- `crates/flui-core/src/subscription.rs` borrows `SubscriptionMap` local state, not app runtime state.

These remain outside K07's AppCell primitive and are not new per-frame App runtime borrow checks.

## Decision

Key Principle #8 remains satisfied for K07: no new AppCell `try_borrow_mut` / `borrow_mut` contention path is introduced into paint or input dispatch hot paths. No new Known Limitation is needed.
