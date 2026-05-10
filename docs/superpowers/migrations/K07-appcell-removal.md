# K07 AppCell Removal Migration Guide

K07 replaces the inherited `AppCell = RefCell<App>` primitive with
`flui_core::app::cell::AppCell`, a K15-aware borrow cell backed by
`UnsafeCell<App>` and `BorrowState`. Most application code keeps compiling
because the doc-hidden `AppCell` / `AppRef` / `AppRefMut` names and
`borrow()` / `borrow_mut()` spelling are preserved. The important migration is
error shape: App borrow contention now surfaces as `ReentryError` directly.

Design spec:
[`2026-05-09-K07-appcell-removal-design.md`](../specs/2026-05-09-K07-appcell-removal-design.md).

## AsyncApp Borrow Contention

Result-returning `AsyncApp` APIs now propagate `ReentryError::AppBorrowed`
directly when they attempt to touch the app during an active mutable borrow.

Before:

```rust
let mut app = app.try_borrow_mut().map_err(ReentryError::from)?;
```

After:

```rust
let mut app = app.try_borrow_mut()?;
```

For public methods that already return `anyhow::Result<T>`, callers can inspect
the source with `downcast_ref::<ReentryError>()`.

```rust
if let Err(error) = async_app.open_window(options, build_root_view) {
    if matches!(error.downcast_ref::<ReentryError>(), Some(ReentryError::AppBorrowed)) {
        cx.defer(|cx| {
            // Retry after the current update finishes.
        });
    }
}
```

## AsyncApp Gone-Away Panics

`AsyncApp` still contains a weak app handle. If the underlying app has already
been released, Result-returning methods now return `ReentryError::AppGoneAway`.
Non-Result `AsyncApp` methods preserve their old panic shape, but the panic
payload is now typed with `std::panic::panic_any(ReentryError::AppGoneAway)`.

Before K07, `catch_unwind` callers saw an unspecified panic payload from the
weak upgrade failure:

```rust
let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    async_app.update_entity(&entity, |state, cx| {
        // ...
    });
}));
```

After K07, callers that recover from panics can identify the exact
`ReentryError::AppGoneAway` payload:

```rust
let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    async_app.update_entity(&entity, |state, cx| {
        // ...
    });
}));

if let Err(payload) = result {
    if matches!(
        payload.downcast_ref::<ReentryError>(),
        Some(ReentryError::AppGoneAway)
    ) {
        // The AsyncApp outlived the App; stop retrying or rebuild the handle.
    }
}
```

## Window::prompt

`Window::prompt` was already migrated by K15 and its post-K07 shape is
unchanged: a second prompt returns `Err(ReentryError::PromptInProgress)`.

Before K15:

```rust
let rx = window.prompt(level, message, detail, answers, cx);
```

After K15/K07:

```rust
let rx = window.prompt(level, message, detail, answers, cx)?;
```

## AsyncWindowContext::prompt

`AsyncWindowContext::prompt` continues to flatten the nested result into
`anyhow::Result<oneshot::Receiver<usize>>`. K07 does not change this call
shape; it only ensures app-borrow failures underneath are `ReentryError`.

```rust
let rx = async_window.prompt(level, message, detail, answers)?;
```

## AppContext::as_mut

`AsyncApp::as_mut`, `AsyncWindowContext::as_mut`, and the async test/headless
contexts still cannot return `Result` because the trait signature returns
`GpuiBorrow<'_, T>`. They now panic with typed
`ReentryError::AsyncContextAsMut` instead of an unstructured string.

Before:

```rust
let result = std::panic::catch_unwind(|| {
    async_app.as_mut(&entity);
});
```

After:

```rust
let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    async_app.as_mut(&entity);
}));

let payload = result.expect_err("as_mut must panic for async contexts");
assert!(matches!(
    payload.downcast_ref::<ReentryError>(),
    Some(ReentryError::AsyncContextAsMut)
));
```

## Direct AppCell Callers

Direct `AppCell` use is rare because the type is `#[doc(hidden)]`, but test
contexts expose `Rc<AppCell>` for low-level harnesses. The spelling remains the
same:

```rust
let mut app = test_context.app.borrow_mut();
```

The behavior changes on contention:

```rust
match test_context.app.try_borrow_mut() {
    Ok(app) => {
        // use app
    }
    Err(ReentryError::AppBorrowed) => {
        // defer or retry later
    }
    Err(error) => return Err(error.into()),
}
```

## BorrowMutError Pattern Matchers

Code that matched `std::cell::BorrowMutError` from flui-core APIs must switch
to `ReentryError::AppBorrowed`.

Before:

```rust
match app.try_borrow_mut() {
    Ok(app) => use_app(app),
    Err(_borrow_error) => defer_work(),
}
```

After:

```rust
match app.try_borrow_mut() {
    Ok(app) => use_app(app),
    Err(ReentryError::AppBorrowed) => defer_work(),
    Err(error) => return Err(error.into()),
}
```

## What Stayed the Same

- `Application::new()` and `Application::run(...)`.
- `Application(Rc<AppCell>)` topology.
- `App::this: Weak<AppCell>` topology.
- `AppCell`, `AppRef`, and `AppRefMut` names.
- `AppCell::borrow()` and `AppCell::borrow_mut()` compatibility spelling.
- `cx.defer(...)` and `Window::defer(...)` as the escape hatches for work that
  must run after the current update returns.
