# ADR-017: Window background blur — X11/KDE/Deepin xprops fill in an existing API

**Date:** 2026-05-12
**Status:** Draft — contract only. No code changes land with this ADR.
**Scope:** `flui-core/src/platform.rs` (`WindowBackgroundAppearance::Blurred`),
`flui-core/src/platform/linux/{x11,wayland}/window.rs`.
**Drivers:** [zed-industries/zed#14590](https://github.com/zed-industries/zed/issues/14590).
**Related:** [zed-industries/zed#5040](https://github.com/zed-industries/zed/issues/5040)
(window transparency umbrella, now closed) — same neighbourhood.

## Context

GPUI #14590 asks for background blur on X11. The original transparency
issue (#5040) covered cross-platform `WindowBackgroundAppearance`;
flui-v2 inherits that public API. What is missing on Linux is the
**implementation** for the `Blurred` variant: KDE's KWin compositor
honours the `_KDE_NET_WM_BLUR_BEHIND_REGION` window property, and the
Deepin compositor honours `_NET_WM_DEEPIN_BLUR_REGION_ROUNDED`. Setting
those properties is enough to get a compositor-side blur for free.

The Wayland side has its own protocol (`org.kde.kwin.blur` on KDE,
nothing portable yet) and is independent.

## Current behaviour (verified)

[`crates/flui-core/src/platform.rs:1618`](../../../crates/flui-core/src/platform.rs#L1618):

```rust
pub enum WindowBackgroundAppearance {
    #[default]
    Opaque,
    Transparent,
    Blurred,
    MicaBackdrop,
    MicaAltBackdrop,
}
```

— the `Blurred` variant exists in the public enum with a doc note that
says "Not always supported".

[`crates/flui-core/src/platform.rs:651`](../../../crates/flui-core/src/platform.rs#L651):

```rust
fn background_appearance(&self) -> WindowBackgroundAppearance;
fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance);
```

— each platform impl maps the value to its own backend. A grep for
`_KDE_NET_WM_BLUR_BEHIND_REGION` / `_NET_WM_DEEPIN_BLUR` /
`set_background_blur` in the codebase returns nothing — the X11 path
ignores `Blurred`.

Wayland likewise has no `org.kde.kwin.blur` binding. macOS and
Windows have working implementations through their native
backdrop-effect surfaces.

## Findings vs upstream

| Issue | Symptom | Repro in flui-v2 today |
|-------|---------|-------------------------|
| [zed-industries/zed#14590](https://github.com/zed-industries/zed/issues/14590) | Background blur not available on X11. | **yes — by omission**. API is present; X11 implementation is not. |

## Decision (contract)

1. **`WindowBackgroundAppearance::Blurred` is best-effort, not
   guaranteed.** The doc note "Not always supported" is binding —
   callers must not assume the value they set is the value they
   get. The visual result on an unsupported compositor falls
   through to `Transparent` (or `Opaque`, if the surface is opaque
   by config).

2. **Each platform has a clear capability matrix.** Documented at
   the call site:

   | Platform | Mechanism | Status |
   |----------|-----------|--------|
   | macOS    | `NSVisualEffectView` | implemented |
   | Windows 10+ | `DwmEnableBlurBehindWindow` / Mica | implemented |
   | Wayland (KDE) | `org.kde.kwin.blur` protocol | not implemented |
   | Wayland (GNOME / wlroots) | no portable protocol | not supported |
   | X11 + KDE | `_KDE_NET_WM_BLUR_BEHIND_REGION` xprop | not implemented (this ADR) |
   | X11 + Deepin | `_NET_WM_DEEPIN_BLUR_REGION_ROUNDED` xprop | not implemented (this ADR) |
   | X11 + other (i3, GNOME on X) | no standard | not supported |
   | Web | CSS `backdrop-filter: blur()` | not implemented (separate ADR) |

3. **`set_background_appearance(Blurred)` followed by
   `background_appearance()` is allowed to return `Transparent` (or
   `Opaque`) on unsupported platforms.** The getter reflects the
   *actually applied* state, not the requested one. Callers that
   need to know whether blur was honoured query the getter.

4. **X11 implementation sets the property on the window for the
   whole client area.** The blur region is the entire window — we
   do not expose per-region blur, even though the X11 protocol
   technically allows it. A future ADR may introduce
   `WindowBackgroundAppearance::PartialBlur { region }`; until
   then, single-region is the contract.

5. **Wayland KDE binding lives in the same `set_background_appearance`
   call.** The implementation switches on the compositor's
   advertised globals; on KDE we get blur, on Mutter we silently
   fall through.

## Consequences

- Apps that use `Blurred` get blur on KDE X11 today, and on Wayland-KDE
  when the binding lands. They get nothing on other compositors —
  this matches every other modern desktop framework.
- The contract makes "blur is a hint" explicit; no caller writes
  code that assumes it succeeded without consulting the getter.
- A future ADR for `org.kde.kwin.blur` reuses the same enum value;
  no API split between X11 and Wayland for the same compositor
  family.

## Out of scope (separate ADRs)

- **Per-region blur** (`PartialBlur { region }`). Currently
  unmotivated; can be added when a real use case shows up.
- **Backdrop tint / vibrancy** beyond plain blur (NSVisualEffectView
  materials, Windows 11 acrylic). Add variants as we implement
  them.
- **Wayland portable blur protocol.** Does not exist; will land
  when xdg-shell or wlr-protocols adopt one.
- **Web `backdrop-filter: blur()`** for the flui web target —
  small CSS shim; deferred to whenever the web target gets real
  use.

## Action items (tracked; no code lands with this ADR)

1. Implement the `Blurred` branch in
   [`platform/linux/x11/window.rs`](../../../crates/flui-core/src/platform/linux/x11/window.rs):
   set `_KDE_NET_WM_BLUR_BEHIND_REGION` and
   `_NET_WM_DEEPIN_BLUR_REGION_ROUNDED` on `set_background_appearance(Blurred)`.
   Clear them on any other variant.
2. Document the capability matrix as a `// CONTRACT:` table at the
   top of [`platform.rs:1615`](../../../crates/flui-core/src/platform.rs#L1615)
   pointing at this ADR.
3. Add a manual test on a KDE X11 session — verify the blur is
   visible.
4. Open a separate ADR for `org.kde.kwin.blur` on Wayland when
   the Wayland binding crate gets used.

## References

### Upstream issues
- [zed-industries/zed#14590](https://github.com/zed-industries/zed/issues/14590) — X11 background blur.
- [zed-industries/zed#5040](https://github.com/zed-industries/zed/issues/5040) — transparency umbrella (closed).

### Internal
- [docs/research/adr/ADR-008-window-chrome-contract.md](ADR-008-window-chrome-contract.md) — neighbouring chrome contract.
- [docs/research/gpui-adr-candidates.md](../gpui-adr-candidates.md) — theme #4 (_Window / display lifecycle_), feature side.
