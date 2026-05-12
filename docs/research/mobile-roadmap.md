# Mobile targets — Android and iOS roadmap

**Date:** 2026-05-12
**Status:** Strategic roadmap, not an ADR. Captures what would be
involved and what depends on what, without committing to a timeline.
**Drivers:**
[zed-industries/zed#43207](https://github.com/zed-industries/zed/issues/43207),
[zed-industries/zed#43206](https://github.com/zed-industries/zed/issues/43206).

## Why this is a roadmap and not an ADR

Mobile support is not a contract decision. It is a platform-expansion
plan: most of the underlying contracts (rendering, input, text,
device-loss) are already covered by ADRs 001–018 and apply on mobile
unchanged. What this document captures is the **sequence and the
non-obvious dependencies**, so when someone signs up to do the work
they do not rediscover them from scratch.

## What flui-v2 has today that mobile re-uses

By inspection of `crates/flui-core/src/platform/`:

- `platform/test/` — synthetic platform for unit tests; reusable.
- `platform/web/` — wasm32 path through wgpu's `BROWSER_WEBGPU | GL`
  backends and `Closure::wrap`-based event registration.
- `platform/wgpu/` — the rendering core; iOS/Android can reuse the
  Metal/Vulkan paths wgpu already supports.
- The cross-platform abstractions (`Platform` trait,
  `PlatformWindow`, `InputHandler`, `WindowOptions`) are
  desktop-shaped today (multiple windows, drag-region, draggable
  title bars). Decisions for mobile need to map them down to one
  full-screen window, a virtual keyboard, and a system-back gesture.

## Per-target sketch

### iOS — [zed-industries/zed#43206](https://github.com/zed-industries/zed/issues/43206)

| Surface | Status today | Mobile work |
|---|---|---|
| Adapter | wgpu's Metal backend works on iOS via wgpu's existing iOS support | reuse |
| Event loop | `platform/mac/` uses `NSApplication` / `CADisplayLink` | a `platform/ios/` sibling needs `UIApplicationDelegate`, `CADisplayLink` |
| Window | `NSWindow` | `UIWindow` (single window, full screen) |
| Input | `NSTextInputClient` | `UITextInput` (similar but not identical; ADR-009 contract holds) |
| Display | `NSScreen` | `UIScreen` (no multi-display by default) |
| GPU device loss | works via wgpu | check `MTLDevice::removed` notifications |
| Distribution | dmg / pkg | App Store / TestFlight; signing & entitlements |

Out of band: Apple's lifecycle (`applicationDidEnterBackground`,
`applicationWillResignActive`) needs explicit hooks — desktop apps
have no analogue. Likely a small extension to the `Platform` trait.

### Android — [zed-industries/zed#43207](https://github.com/zed-industries/zed/issues/43207)

| Surface | Status today | Mobile work |
|---|---|---|
| Adapter | wgpu's Vulkan backend works on Android | reuse |
| Event loop | `platform/linux/{x11,wayland}/` | `Activity` / `Looper` via `ndk` + `android-activity` crates |
| Window | per-platform | `ANativeWindow` (single, full screen) |
| Input | Wayland / X11 IME | `InputMethodManager` + `KeyEvent` (ADR-009 enum maps) |
| Display | per-platform | `Display` API; high-DPI is the default |
| GPU device loss | works via wgpu | Android pauses GPU during background; integrate with `onPause`/`onResume` |
| Distribution | tarball | `.aab` / Play Store; signing |

Out of band: Android's `onPause` is closer to "the GPU surface might
disappear" than to "the window is no longer focused". Plug that into
ADR-005's `recover()` flow.

## Mobile-specific contracts that are not yet ADRs

These show up only when mobile lands; they are not worth writing now,
but the implementer should expect to author them:

1. **Single-window platforms.** What `App::displays()` means when
   there is exactly one. ADR-007 contract continues to hold but the
   observers fire only on connect/disconnect of external displays.
2. **Virtual keyboard / IME visibility.** The keyboard occludes part
   of the viewport; `Window::viewport_size` should reflect that, and
   layout should respond. New trait method
   `PlatformWindow::keyboard_insets()`.
3. **System back gesture.** Android's `onBackPressed` does not exist
   on iOS (gesture instead). A `Platform::on_system_navigation`
   abstraction unifies them.
4. **App lifecycle states.** `Active | Inactive | Background |
   Suspended`. Composes with the frame budget from ADR-014 (software
   rendering): a backgrounded app may skip frames or be killed.
5. **Touch as the primary pointer.** Hit-test already works for
   touch in `gesture/`; verify the gesture recognizers cope with
   multi-touch (`pan`, `scale`, `long_press` exist; verify they
   work without a hover state).
6. **Accessibility integration.** Both platforms have rich a11y
   APIs (`UIAccessibility`, Android `AccessibilityNode`); the
   flui-a11y crate (currently a 4-line stub — see ADR-010) is the
   integration point.

## What we should *not* do before mobile lands

- Add per-platform `cfg(target_os = "ios")` to the cross-target
  `[dependencies]` block. Mobile crates sit under
  `[target.'cfg(target_os = "ios")'.dependencies]` (ADR-016
  applies).
- Bake desktop assumptions into new APIs. Multi-window is a
  desktop concept; mobile pretends to have one window and never
  asks `App::all_windows()` to return more than one.
- Pre-implement a "phone mode" toggle in widget libraries. The
  layout system already has media queries (`MediaQuery`); use them.

## Per-issue notes

- **#43207 (Android)** — Touchpoint with [`android-activity`](https://crates.io/crates/android-activity)
  crate for the event loop; wgpu has a `Backends::VULKAN` path that
  works there.
- **#43206 (iOS)** — Touchpoint with [`objc2`](https://crates.io/crates/objc2)
  successor of `objc`; macOS path in `platform/mac/` is the closest
  analogue. Be careful with `UIScene` (iOS 13+) vs `UIApplication`
  multi-tasking.

## References

### Upstream issues
- [zed-industries/zed#43207](https://github.com/zed-industries/zed/issues/43207) — Android.
- [zed-industries/zed#43206](https://github.com/zed-industries/zed/issues/43206) — iOS.
- [zed-industries/zed#12039](https://github.com/zed-industries/zed/issues/12039) — the harder "Zed on iOS" umbrella; out of scope here.

### Internal ADRs that already apply
- [ADR-005](adr/ADR-005-gpu-device-loss.md) — device-loss path integrates with Android onPause.
- [ADR-007](adr/ADR-007-display-lifecycle.md) — single-display semantics.
- [ADR-009](adr/ADR-009-input-ime-contract.md) — `EditorCommand` enum extends to mobile IMEs.
- [ADR-014](adr/ADR-014-software-rendering-fallback.md) — frame budget tightens under battery / background.
- [ADR-016](adr/ADR-016-wasm-target-gating.md) — per-target dependency-gating policy.
