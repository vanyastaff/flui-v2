# Spec A: BuildContext + Platform + Provider

**Date:** 2026-04-07
**Status:** Approved
**Scope:** flui-core improvements — platform brightness, locale, MediaQueryData, Provider migration

---

## Goals

Add foundational platform APIs to flui-core and clean up the dependency graph by moving Provider into core.

## Non-Goals

- Safe area / padding (mobile-only, Phase 3)
- Subscription to locale changes at runtime
- Platform text_scale_factor detection (hardcoded 1.0 for MVP)
- BuildContext unified trait (methods added directly to App/Window)
- Animations, gestures, layout extensions (separate specs)

---

## 1. Platform Brightness

### Problem

Dark mode detection is currently per-window (`window.appearance()`). There is no app-level API to read OS brightness preference before a window exists or across the entire application.

### Design

**New Global:** `SystemBrightness` stored in `App`, initialized at startup from platform API.

**New Platform trait methods:**
```rust
fn brightness(&self) -> Brightness;
fn on_brightness_changed(&self, callback: Box<dyn Fn(Brightness) + Send>);
```

**Platform implementations:**
- macOS: `NSApp.effectiveAppearance` → check `bestMatch` against `NSAppearanceNameDarkAqua`
- Linux: D-Bus `org.freedesktop.appearance` `color-scheme` property (already used in `xdg_desktop_portal.rs` for window appearance)
- Windows: registry `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`

**App API:**
```rust
impl App {
    /// Read current OS brightness preference.
    fn platform_brightness(&self) -> Brightness;

    /// Subscribe to OS brightness changes.
    fn observe_platform_brightness(
        &mut self,
        callback: impl Fn(Brightness, &mut App) + 'static,
    ) -> Subscription;
}
```

**Initialization:** `App::new()` calls platform `brightness()` and stores result in `SystemBrightness` Global. Registers `on_brightness_changed` callback to update the Global and notify observers.

**File:** `src/platform_brightness.rs`

---

## 2. Locale and TextDirection

### Problem

No system locale detection. Flutter developers expect `locale()` and `text_direction()` on the context.

### Design

**New types:**
```rust
pub struct Locale {
    pub language: String,       // ISO 639-1: "en", "ru", "ar"
    pub country: Option<String>, // ISO 3166-1: "US", "RU"
}

pub enum TextDirection {
    Ltr,
    Rtl,
}
```

**Storage:** `SystemLocale` as Global in App. Initialized once at startup.

**Platform detection:**
- macOS: `NSLocale.currentLocale` → `languageCode` + `countryCode`
- Linux: `LC_ALL` or `LANG` environment variable, parse `lang_COUNTRY.encoding`
- Windows: `GetUserDefaultLocaleName` → parse BCP 47 tag

**TextDirection:** Computed from `Locale.language`. RTL languages: `ar`, `he`, `fa`, `ur`, `ps`, `sd`, `yi`. All others: LTR. Simple lookup table, no ICU dependency.

**App API:**
```rust
impl App {
    fn locale(&self) -> &Locale;
    fn text_direction(&self) -> TextDirection;
}
```

No runtime locale change subscription (YAGNI — users rarely change locale mid-session).

**File:** `src/locale.rs`

---

## 3. MediaQueryData

### Problem

Window-level data (size, DPR, brightness) is scattered across different methods. Flutter developers expect a single `MediaQuery` access point.

### Design

**Convenience struct:**
```rust
pub struct MediaQueryData {
    pub size: Size<Pixels>,      // window content area size
    pub scale_factor: f32,       // device pixel ratio (1.0, 2.0, etc.)
    pub brightness: Brightness,  // from platform (app-level)
    pub text_scale_factor: f32,  // OS text scaling (hardcoded 1.0 for MVP)
}
```

**Window API:**
```rust
impl Window {
    fn media_query(&self, cx: &App) -> MediaQueryData;
}
```

Data sources:
- `size` ← `self.content_size()` (existing)
- `scale_factor` ← `self.scale_factor()` (existing)
- `brightness` ← `cx.platform_brightness()` (from section 1)
- `text_scale_factor` ← `1.0` (platform detection deferred)

No InheritedWidget/Provider wrapper — purely a convenience struct with zero overhead. Computed on each call, not cached.

**File:** `src/media_query.rs`

---

## 4. Provider Migration to flui-core

### Problem

`Provider<T>` lives in flui-widgets. This forces flui-theme to depend on flui-widgets, creating an inverted dependency (theme should be lower than widgets).

### Design

**Move to flui-core:**
- `src/provider.rs` (or `src/provider/` directory) containing:
  - Thread-local stack: `HashMap<TypeId, Vec<Box<dyn Any>>>` with `push`/`pop`
  - `Provider<T>` component implementing `RenderOnce` → `ProviderElement<T>` implementing `Element`
  - `InheritedValue` trait (blanket impl for `Any + Clone + Send + Sync + 'static`)
  - Public functions: `read::<T>()`, `try_read::<T>()`

**Public API from flui-core:**
```rust
pub use provider::{Provider, InheritedValue, read, try_read};
```

**flui-widgets changes:**
- Delete `src/provider/` directory
- Re-export from flui-core: `pub use flui_core::{Provider, InheritedValue, read, try_read};`
- Keeps backward compatibility — downstream code doesn't change

**flui-theme Cargo.toml change:**
- Remove `flui-widgets` dependency
- Depends only on `flui-core` (which now has Provider)

**Resulting dependency graph:**
```
flui-material → flui-theme + flui-widgets
flui-theme    → flui-core
flui-widgets  → flui-core
flui-navigator → flui-core
```

---

## 5. Files Changed

### New files in flui-core:
| File | Contents |
|------|----------|
| `src/brightness.rs` | `Brightness` enum (Light/Dark) — canonical definition, replaces flui-theme's copy |
| `src/platform_brightness.rs` | SystemBrightness Global, App methods |
| `src/locale.rs` | Locale, TextDirection, SystemLocale Global, App methods |
| `src/media_query.rs` | MediaQueryData struct, Window method |
| `src/provider.rs` | Provider<T>, ProviderElement<T>, InheritedValue, read/try_read |

### Modified files in flui-core:
| File | Change |
|------|--------|
| `src/platform.rs` | Add `brightness()`, `on_brightness_changed()`, `locale()` to Platform trait |
| `src/platform/mac/platform.rs` | macOS implementations |
| `src/platform/linux/platform.rs` | Linux implementations |
| `src/platform/windows/platform.rs` | Windows implementations |
| `src/app.rs` | Init SystemBrightness/SystemLocale, add platform_brightness/locale/text_direction methods |
| `src/window.rs` | Add `media_query()` method |
| `src/lib.rs` | pub mod + re-exports for new modules |

### Modified files in other crates:
| File | Change |
|------|--------|
| `crates/flui-widgets/src/provider/` | Delete directory |
| `crates/flui-widgets/src/lib.rs` | Re-export Provider from flui-core |
| `crates/flui-theme/Cargo.toml` | Remove flui-widgets dependency |
| `crates/flui-theme/src/lib.rs` | Remove flui-widgets re-export |

---

## 6. Testing

- Unit test: `SystemBrightness` initialization and observer notification
- Unit test: `Locale` parsing from env/platform strings
- Unit test: `TextDirection` lookup for RTL languages
- Unit test: `MediaQueryData` construction from mock window/app
- Unit test: Provider push/pop/read/try_read stack behavior
- Unit test: Nested Provider override correctness
- Integration: Provider<ThemeData> used by flui-theme without flui-widgets dependency
