//! # flui-platform
//!
//! Platform abstraction layer for the flui UI framework.
//!
//! This crate is **intentionally empty** in spec S02a. It is a reserved
//! slot in the workspace dependency graph that will be populated
//! incrementally by subsequent migration specs:
//!
//! - **S02b** — the `Platform` trait family (`Platform`, `PlatformWindow`,
//!   `PlatformDisplay`, `PlatformDispatcher`, `PlatformTextSystem`,
//!   `PlatformKeyboardLayout`, `PlatformKeyboardMapper`,
//!   `PlatformHeadlessRenderer`), supporting value types referenced by
//!   trait signatures (`ClipboardItem`, `WindowParams`, `AnyWindowHandle`,
//!   `CursorStyle`, `Menu`, `Keymap`, `Brightness`, …), and the test
//!   backends (`TestPlatform`, `TestDispatcher`, `TestDisplay`,
//!   `TestWindow`, `VisualTestPlatform`). S02b is the "trait flip" — the
//!   point at which `flui-core` first gains a dependency on
//!   `flui-platform`.
//! - **S03** — wgpu backend and Linux (`x11`, `wayland`, `headless`)
//!   backends.
//! - **S04** — macOS backend (Metal, cbindgen-generated shader ABI).
//! - **S05** — Windows backend (DirectX, FXC shader compilation).
//! - **S06** — web backend, the `keystroke` / `keyboard` / `app_menu` /
//!   `layer_shell` / `scap_screen_capture` top-level modules, and
//!   deletion of `flui-core/src/platform/`. After S06, `flui-core`
//!   re-exports the Platform API from this crate.
//!
//! See `docs/superpowers/specs/2026-04-13-flui-core-roadmap.md` for the
//! full migration plan.

#![warn(missing_docs)]
