# Core Context + Platform + Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add platform brightness/locale/MediaQueryData APIs to flui-core and migrate Provider from flui-widgets to flui-core, cleaning up the dependency graph.

**Architecture:** New modules added to flui-core (`brightness`, `locale`, `media_query`, `provider`). Platform trait extended with `brightness()` and `locale()` methods. Each platform backend implements them. flui-theme drops flui-widgets dependency and re-exports Brightness from flui-core.

**Tech Stack:** Rust, flui-core (gpui-ce fork), platform APIs (macOS Cocoa, Linux env/D-Bus, Windows registry)

---

### Task 1: Brightness enum in flui-core

**Files:**
- Create: `crates/flui-core/src/brightness.rs`
- Modify: `crates/flui-core/src/lib.rs`

- [ ] **Step 1: Create `brightness.rs`**

```rust
// crates/flui-core/src/brightness.rs

/// Whether the system or theme uses light or dark mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Brightness {
    /// Light theme variant.
    #[default]
    Light,
    /// Dark theme variant.
    Dark,
}
```

- [ ] **Step 2: Register module and re-export in `lib.rs`**

Add after the existing `mod color;` line in `crates/flui-core/src/lib.rs`:
```rust
mod brightness;
```

Add to the re-exports section:
```rust
pub use brightness::*;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p flui-core 2>&1 | grep '^error'`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add crates/flui-core/src/brightness.rs crates/flui-core/src/lib.rs
git commit -m "feat(flui-core): add Brightness enum (Light/Dark)"
```

---

### Task 2: Platform trait — brightness methods

**Files:**
- Modify: `crates/flui-core/src/platform.rs` (Platform trait, ~line 204)
- Modify: `crates/flui-core/src/platform/test/platform.rs`
- Modify: `crates/flui-core/src/platform/linux/platform.rs`

- [ ] **Step 1: Add methods to Platform trait**

In `crates/flui-core/src/platform.rs`, inside `pub trait Platform: 'static {`, add after the existing `fn window_appearance(&self) -> WindowAppearance;` method (around line 247):

```rust
    /// Returns the current OS-level brightness preference (light or dark).
    /// Unlike `window_appearance()` which is per-window, this is app-level.
    fn brightness(&self) -> crate::Brightness {
        // Default: derive from window_appearance
        match self.window_appearance() {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => crate::Brightness::Dark,
            _ => crate::Brightness::Light,
        }
    }

    /// Register a callback to be notified when OS brightness preference changes.
    fn on_brightness_changed(&self, _callback: Box<dyn Fn(crate::Brightness) + Send>) {
        // Default: no-op. Platforms override as needed.
    }
```

Default implementations derive from `window_appearance()` — this means all existing platform backends (mac, linux, windows, test) get a working implementation immediately without modification.

- [ ] **Step 2: Override for test platform with explicit control**

In `crates/flui-core/src/platform/test/platform.rs`, add a field to `TestPlatform`:
```rust
pub(crate) brightness: Mutex<crate::Brightness>,
```

Initialize it in the constructor with `Mutex::new(crate::Brightness::Light)`.

Override the trait method:
```rust
fn brightness(&self) -> crate::Brightness {
    *self.brightness.lock()
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p flui-core 2>&1 | grep '^error'`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add crates/flui-core/src/platform.rs crates/flui-core/src/platform/test/platform.rs
git commit -m "feat(flui-core): add brightness() to Platform trait with defaults"
```

---

### Task 3: SystemBrightness Global and App methods

**Files:**
- Create: `crates/flui-core/src/platform_brightness.rs`
- Modify: `crates/flui-core/src/app.rs`
- Modify: `crates/flui-core/src/lib.rs`

- [ ] **Step 1: Create `platform_brightness.rs`**

```rust
// crates/flui-core/src/platform_brightness.rs

use crate::{Brightness, Global};

/// App-level global storing the current OS brightness preference.
///
/// Initialized at app startup from the platform API.
/// Updated automatically when the OS preference changes.
pub struct SystemBrightness(pub Brightness);

impl Global for SystemBrightness {}
```

- [ ] **Step 2: Register module in `lib.rs`**

Add `mod platform_brightness;` and `pub use platform_brightness::*;` in `lib.rs`.

- [ ] **Step 3: Add App methods**

In `crates/flui-core/src/app.rs`, inside `impl App {` (at line 666), add:

```rust
    /// Read the current OS brightness preference (Light or Dark).
    ///
    /// This is app-level — independent of any specific window.
    pub fn platform_brightness(&self) -> Brightness {
        self.try_global::<SystemBrightness>()
            .map(|sb| sb.0)
            .unwrap_or(Brightness::Light)
    }
```

- [ ] **Step 4: Initialize SystemBrightness at app startup**

Find the `App::new()` or initialization code in `app.rs`. After globals are set up, add:

```rust
let brightness = platform.brightness();
// set_global requires BorrowAppContext which App implements via self
this.set_global(SystemBrightness(brightness));
```

Note: the exact location depends on the App construction flow. Search for `set_global` calls in `App::new()` to find the right place.

- [ ] **Step 5: Verify it compiles**

Run: `cargo check -p flui-core 2>&1 | grep '^error'`
Expected: no errors

- [ ] **Step 6: Commit**

```bash
git add crates/flui-core/src/platform_brightness.rs crates/flui-core/src/app.rs crates/flui-core/src/lib.rs
git commit -m "feat(flui-core): SystemBrightness Global + App::platform_brightness()"
```

---

### Task 4: Locale and TextDirection

**Files:**
- Create: `crates/flui-core/src/locale.rs`
- Modify: `crates/flui-core/src/platform.rs` (Platform trait)
- Modify: `crates/flui-core/src/platform/test/platform.rs`
- Modify: `crates/flui-core/src/app.rs`
- Modify: `crates/flui-core/src/lib.rs`

- [ ] **Step 1: Create `locale.rs`**

```rust
// crates/flui-core/src/locale.rs

use crate::Global;

/// System locale — language and optional country code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Locale {
    /// ISO 639-1 language code: "en", "ru", "ar", "he"
    pub language: String,
    /// ISO 3166-1 country code: "US", "RU", "SA"
    pub country: Option<String>,
}

impl Default for Locale {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            country: None,
        }
    }
}

impl Locale {
    /// Create a new Locale.
    pub fn new(language: impl Into<String>, country: Option<impl Into<String>>) -> Self {
        Self {
            language: language.into(),
            country: country.map(|c| c.into()),
        }
    }

    /// Parse a POSIX locale string like "en_US.UTF-8" or "ru_RU" or "en".
    pub fn from_posix(s: &str) -> Self {
        // Strip encoding: "en_US.UTF-8" → "en_US"
        let without_encoding = s.split('.').next().unwrap_or("en");
        // Strip modifier: "sr_RS@latin" → "sr_RS"
        let without_modifier = without_encoding.split('@').next().unwrap_or("en");

        if let Some((lang, country)) = without_modifier.split_once('_') {
            Self::new(lang.to_lowercase(), Some(country.to_uppercase()))
        } else {
            Self::new(without_modifier.to_lowercase(), None::<String>)
        }
    }

    /// Parse a BCP 47 tag like "en-US" or "zh-Hans-CN".
    pub fn from_bcp47(s: &str) -> Self {
        let parts: Vec<&str> = s.split('-').collect();
        let language = parts.first().unwrap_or(&"en").to_lowercase();
        // Country is the first 2-letter uppercase part after language
        let country = parts.iter().skip(1).find(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_uppercase()));
        Self::new(language, country.map(|c| c.to_string()))
    }
}

/// Text direction — left-to-right or right-to-left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextDirection {
    /// Left-to-right (default for most languages).
    #[default]
    Ltr,
    /// Right-to-left (Arabic, Hebrew, Persian, Urdu, etc).
    Rtl,
}

impl TextDirection {
    /// Determine text direction from a language code.
    pub fn from_language(language: &str) -> Self {
        match language {
            "ar" | "he" | "fa" | "ur" | "ps" | "sd" | "yi" | "ckb" | "ug" => Self::Rtl,
            _ => Self::Ltr,
        }
    }
}

/// App-level global storing the system locale.
pub(crate) struct SystemLocale(pub Locale);

impl Global for SystemLocale {}
```

- [ ] **Step 2: Add `locale()` to Platform trait**

In `crates/flui-core/src/platform.rs`, inside `pub trait Platform`, add:

```rust
    /// Returns the system locale.
    fn locale(&self) -> Locale {
        // Default: try POSIX env vars, fallback to "en"
        if let Ok(val) = std::env::var("LC_ALL").or_else(|_| std::env::var("LANG")).or_else(|_| std::env::var("LC_MESSAGES")) {
            if val != "C" && val != "POSIX" && !val.is_empty() {
                return Locale::from_posix(&val);
            }
        }
        Locale::default()
    }
```

Add `use crate::Locale;` at the top of `platform.rs` if needed.

- [ ] **Step 3: Add App methods**

In `crates/flui-core/src/app.rs`, inside `impl App {`, add:

```rust
    /// Get the system locale.
    pub fn locale(&self) -> &Locale {
        &self.global::<SystemLocale>().0
    }

    /// Get the text direction for the current locale.
    pub fn text_direction(&self) -> TextDirection {
        TextDirection::from_language(&self.locale().language)
    }
```

- [ ] **Step 4: Initialize SystemLocale at app startup**

In the same location where `SystemBrightness` was initialized (Task 3, Step 4), add:

```rust
let locale = platform.locale();
this.set_global(SystemLocale(locale));
```

- [ ] **Step 5: Register module and re-export**

In `lib.rs`, add `mod locale;` and `pub use locale::*;`.

- [ ] **Step 6: Verify it compiles**

Run: `cargo check -p flui-core 2>&1 | grep '^error'`
Expected: no errors

- [ ] **Step 7: Commit**

```bash
git add crates/flui-core/src/locale.rs crates/flui-core/src/platform.rs crates/flui-core/src/app.rs crates/flui-core/src/lib.rs
git commit -m "feat(flui-core): Locale, TextDirection, App::locale()/text_direction()"
```

---

### Task 5: MediaQueryData

**Files:**
- Create: `crates/flui-core/src/media_query.rs`
- Modify: `crates/flui-core/src/window.rs`
- Modify: `crates/flui-core/src/lib.rs`

- [ ] **Step 1: Create `media_query.rs`**

```rust
// crates/flui-core/src/media_query.rs

use crate::{Brightness, Pixels, Size};

/// Aggregated window + platform data — convenience struct.
///
/// Equivalent to Flutter's `MediaQueryData`.
#[derive(Clone, Debug)]
pub struct MediaQueryData {
    /// Window content area size in logical pixels.
    pub size: Size<Pixels>,
    /// Device pixel ratio (1.0 on standard displays, 2.0 on Retina, etc).
    pub scale_factor: f32,
    /// OS-level brightness preference (Light or Dark).
    pub brightness: Brightness,
    /// OS text scaling factor.
    // TODO: detect from OS (macOS accessibility, GNOME text-scaling-factor, Windows SystemParametersInfo)
    pub text_scale_factor: f32,
}
```

- [ ] **Step 2: Add `media_query()` to Window**

In `crates/flui-core/src/window.rs`, inside `impl Window {`, add:

```rust
    /// Get aggregated media query data for this window.
    ///
    /// Combines window-level info (size, DPR) with app-level info (brightness).
    pub fn media_query(&self, cx: &App) -> MediaQueryData {
        MediaQueryData {
            size: self.bounds().size,
            scale_factor: self.scale_factor(),
            brightness: cx.platform_brightness(),
            text_scale_factor: 1.0, // TODO: detect from OS
        }
    }
```

Add `use crate::MediaQueryData;` at the top of `window.rs` if needed.

- [ ] **Step 3: Register module and re-export**

In `lib.rs`, add `mod media_query;` and `pub use media_query::*;`.

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p flui-core 2>&1 | grep '^error'`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add crates/flui-core/src/media_query.rs crates/flui-core/src/window.rs crates/flui-core/src/lib.rs
git commit -m "feat(flui-core): MediaQueryData + Window::media_query()"
```

---

### Task 6: Migrate Provider to flui-core

**Files:**
- Create: `crates/flui-core/src/provider/mod.rs`
- Create: `crates/flui-core/src/provider/stack.rs`
- Create: `crates/flui-core/src/provider/element.rs`
- Modify: `crates/flui-core/src/lib.rs`

- [ ] **Step 1: Create `provider/stack.rs`**

Copy from `crates/flui-widgets/src/provider/stack.rs` but update the import path:

```rust
// crates/flui-core/src/provider/stack.rs

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;

use super::InheritedValue;

thread_local! {
    static PROVIDER_STACKS: RefCell<HashMap<TypeId, Vec<Box<dyn Any>>>> = RefCell::new(HashMap::new());
}

/// Push a value onto the thread-local provider stack for type `T`.
pub fn push<T: InheritedValue>(value: T) {
    PROVIDER_STACKS.with(|stacks| {
        stacks
            .borrow_mut()
            .entry(TypeId::of::<T>())
            .or_default()
            .push(Box::new(value));
    });
}

/// Pop the most recent value from the provider stack for type `T`.
pub fn pop<T: InheritedValue>() {
    PROVIDER_STACKS.with(|stacks| {
        let mut stacks = stacks.borrow_mut();
        let stack = stacks
            .get_mut(&TypeId::of::<T>())
            .expect("Provider::pop called without matching push");
        stack.pop().expect("Provider stack underflow");
    });
}

/// Read the current value of type `T`. Returns `None` if no Provider exists.
pub fn try_read<T: InheritedValue>() -> Option<T> {
    PROVIDER_STACKS.with(|stacks| {
        stacks
            .borrow()
            .get(&TypeId::of::<T>())
            .and_then(|stack| stack.last())
            .and_then(|val| val.downcast_ref::<T>())
            .cloned()
    })
}

/// Read the current value of type `T`. Panics if no Provider exists.
pub fn read<T: InheritedValue>() -> T {
    try_read::<T>().unwrap_or_else(|| {
        panic!(
            "No Provider<{}> found in the current render tree. \
             Wrap a parent widget with Provider::new(value, child).",
            std::any::type_name::<T>()
        )
    })
}
```

- [ ] **Step 2: Create `provider/element.rs`**

```rust
// crates/flui-core/src/provider/element.rs

use crate::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, RenderOnce, Window,
};

use super::{InheritedValue, stack};

/// Provides a value of type `T` to all descendant widgets during rendering.
///
/// Children read the value with `read::<T>()` or `try_read::<T>()`.
/// Nesting: inner Provider overrides outer for the subtree.
#[derive(crate::IntoElement)]
pub struct Provider<T: InheritedValue> {
    value: T,
    child: AnyElement,
}

impl<T: InheritedValue> Provider<T> {
    /// Create a Provider wrapping a child subtree.
    pub fn new(value: T, child: impl IntoElement) -> Self {
        Self {
            value,
            child: child.into_any_element(),
        }
    }
}

impl<T: InheritedValue> RenderOnce for Provider<T> {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        ProviderElement::<T> {
            value: self.value,
            child: self.child,
        }
    }
}

/// Internal Element that manages push/pop lifecycle.
struct ProviderElement<T: InheritedValue> {
    value: T,
    child: AnyElement,
}

impl<T: InheritedValue> Element for ProviderElement<T> {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        stack::push(self.value.clone());
        let layout_id = self.child.request_layout(window, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
        stack::pop::<T>();
    }
}

impl<T: InheritedValue> IntoElement for ProviderElement<T> {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}
```

- [ ] **Step 3: Create `provider/mod.rs`**

```rust
// crates/flui-core/src/provider/mod.rs

mod element;
pub(crate) mod stack;

use std::any::Any;

/// Marker trait for values that can be propagated via Provider.
/// Blanket-implemented for any `Clone + Send + Sync + 'static` type.
pub trait InheritedValue: Any + Clone + Send + Sync + 'static {}
impl<T: Any + Clone + Send + Sync + 'static> InheritedValue for T {}

pub use element::Provider;
pub use stack::{read, try_read};
```

- [ ] **Step 4: Register module and re-export in `lib.rs`**

In `crates/flui-core/src/lib.rs`, add:
```rust
mod provider;
```

And in the re-exports:
```rust
pub use provider::{InheritedValue, Provider, read, try_read};
```

- [ ] **Step 5: Verify flui-core compiles**

Run: `cargo check -p flui-core 2>&1 | grep '^error'`
Expected: no errors

- [ ] **Step 6: Commit**

```bash
git add crates/flui-core/src/provider/
git add crates/flui-core/src/lib.rs
git commit -m "feat(flui-core): migrate Provider<T> from flui-widgets to core"
```

---

### Task 7: Update flui-widgets — remove provider, re-export from core

**Files:**
- Delete: `crates/flui-widgets/src/provider/inherited.rs`
- Delete: `crates/flui-widgets/src/provider/provider.rs`
- Delete: `crates/flui-widgets/src/provider/stack.rs`
- Delete: `crates/flui-widgets/src/provider/mod.rs`
- Modify: `crates/flui-widgets/src/lib.rs`

- [ ] **Step 1: Delete provider directory**

```bash
rm -rf crates/flui-widgets/src/provider/
```

- [ ] **Step 2: Update `lib.rs` — replace provider module with re-exports**

In `crates/flui-widgets/src/lib.rs`, remove:
```rust
pub mod provider;
```
and:
```rust
pub use provider::{InheritedValue, Provider, read, try_read};
```

Replace with:
```rust
// Re-export Provider from flui-core for backward compatibility
pub use flui_core::{InheritedValue, Provider, read, try_read};
```

- [ ] **Step 3: Verify flui-widgets compiles**

Run: `cargo check -p flui-widgets 2>&1 | grep '^error'`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor(flui-widgets): remove provider, re-export from flui-core"
```

---

### Task 8: Update flui-theme — depend on flui-core only

**Files:**
- Modify: `crates/flui-theme/Cargo.toml`
- Modify: `crates/flui-theme/src/lib.rs`
- Delete: `crates/flui-theme/src/brightness.rs`
- Modify: `crates/flui-theme/src/theme_data.rs`

- [ ] **Step 1: Remove flui-widgets dependency from Cargo.toml**

In `crates/flui-theme/Cargo.toml`, change:
```toml
[dependencies]
flui-core = { path = "../flui-core" }
flui-widgets = { path = "../flui-widgets" }
```

To:
```toml
[dependencies]
flui-core = { path = "../flui-core" }
```

- [ ] **Step 2: Delete `brightness.rs` — now comes from flui-core**

```bash
rm crates/flui-theme/src/brightness.rs
```

- [ ] **Step 3: Update `lib.rs`**

Remove `mod brightness;` and `pub use brightness::Brightness;`.
Remove `pub use flui_widgets;`.

Add:
```rust
pub use flui_core::Brightness;
```

- [ ] **Step 4: Update `theme_data.rs` — use flui_core::Brightness**

In `crates/flui-theme/src/theme_data.rs`, change:
```rust
use crate::{Brightness, ColorScheme, ShapeTheme, SpacingTheme, TextTheme};
```
This should still work since `crate::Brightness` now re-exports from `flui_core::Brightness`.

If it was importing `crate::brightness::Brightness` directly, update to `flui_core::Brightness` or `crate::Brightness`.

- [ ] **Step 5: Verify flui-theme compiles**

Run: `cargo check -p flui-theme 2>&1 | grep '^error'`
Expected: no errors

- [ ] **Step 6: Verify full workspace compiles**

Run: `cargo check --workspace 2>&1 | tail -5`
Expected: `Finished` with no errors

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(flui-theme): depend on flui-core only, re-export Brightness from core"
```

---

### Task 9: Unit tests

**Files:**
- Create: `crates/flui-core/src/locale.rs` (add tests at bottom)
- Create: `crates/flui-core/src/provider/stack.rs` (add tests at bottom)

- [ ] **Step 1: Add locale parsing tests**

Append to `crates/flui-core/src/locale.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locale_from_posix_full() {
        let locale = Locale::from_posix("en_US.UTF-8");
        assert_eq!(locale.language, "en");
        assert_eq!(locale.country, Some("US".to_string()));
    }

    #[test]
    fn test_locale_from_posix_no_encoding() {
        let locale = Locale::from_posix("ru_RU");
        assert_eq!(locale.language, "ru");
        assert_eq!(locale.country, Some("RU".to_string()));
    }

    #[test]
    fn test_locale_from_posix_language_only() {
        let locale = Locale::from_posix("en");
        assert_eq!(locale.language, "en");
        assert_eq!(locale.country, None);
    }

    #[test]
    fn test_locale_from_posix_with_modifier() {
        let locale = Locale::from_posix("sr_RS@latin");
        assert_eq!(locale.language, "sr");
        assert_eq!(locale.country, Some("RS".to_string()));
    }

    #[test]
    fn test_locale_from_bcp47_simple() {
        let locale = Locale::from_bcp47("en-US");
        assert_eq!(locale.language, "en");
        assert_eq!(locale.country, Some("US".to_string()));
    }

    #[test]
    fn test_locale_from_bcp47_with_script() {
        let locale = Locale::from_bcp47("zh-Hans-CN");
        assert_eq!(locale.language, "zh");
        assert_eq!(locale.country, Some("CN".to_string()));
    }

    #[test]
    fn test_text_direction_ltr() {
        assert_eq!(TextDirection::from_language("en"), TextDirection::Ltr);
        assert_eq!(TextDirection::from_language("ru"), TextDirection::Ltr);
        assert_eq!(TextDirection::from_language("zh"), TextDirection::Ltr);
    }

    #[test]
    fn test_text_direction_rtl() {
        assert_eq!(TextDirection::from_language("ar"), TextDirection::Rtl);
        assert_eq!(TextDirection::from_language("he"), TextDirection::Rtl);
        assert_eq!(TextDirection::from_language("fa"), TextDirection::Rtl);
        assert_eq!(TextDirection::from_language("ur"), TextDirection::Rtl);
    }
}
```

- [ ] **Step 2: Add provider stack tests**

Append to `crates/flui-core/src/provider/stack.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_read_pop() {
        push(42i32);
        assert_eq!(read::<i32>(), 42);
        pop::<i32>();
    }

    #[test]
    fn test_try_read_empty() {
        assert_eq!(try_read::<f64>(), None);
    }

    #[test]
    fn test_nested_override() {
        push(1i32);
        assert_eq!(read::<i32>(), 1);

        push(2i32);
        assert_eq!(read::<i32>(), 2);

        pop::<i32>();
        assert_eq!(read::<i32>(), 1);

        pop::<i32>();
    }

    #[test]
    fn test_multiple_types() {
        push(42i32);
        push("hello".to_string());

        assert_eq!(read::<i32>(), 42);
        assert_eq!(read::<String>(), "hello");

        pop::<String>();
        pop::<i32>();
    }

    #[test]
    #[should_panic(expected = "No Provider<i32>")]
    fn test_read_panics_when_empty() {
        let _ = read::<i32>();
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p flui-core -- locale::tests provider::stack::tests 2>&1 | tail -15`
Expected: all tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/flui-core/src/locale.rs crates/flui-core/src/provider/stack.rs
git commit -m "test(flui-core): unit tests for Locale parsing, TextDirection, Provider stack"
```

---

### Deferred Items

- `observe_platform_brightness()` subscription — spec mentions it but for MVP `platform_brightness()` read-only is sufficient. `MaterialApp::render()` re-reads brightness each frame. Subscription can be added when needed.
- Platform-specific locale detection (macOS `NSLocale`, Windows `GetUserDefaultLocaleName`) — the default env-var approach works cross-platform. Platform overrides are a future optimization.

---

### Task 10: Final verification

**Files:** None (verification only)

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace 2>&1 | tail -5`
Expected: `Finished` with no errors (warnings OK)

- [ ] **Step 2: Run all tests**

Run: `cargo test -p flui-core 2>&1 | tail -10`
Expected: tests pass

- [ ] **Step 3: Verify dependency graph**

Run: `grep 'flui-widgets' crates/flui-theme/Cargo.toml`
Expected: no output (dependency removed)

Run: `grep 'Provider' crates/flui-core/src/lib.rs`
Expected: shows re-export

- [ ] **Step 4: Push**

```bash
git push origin main
```
