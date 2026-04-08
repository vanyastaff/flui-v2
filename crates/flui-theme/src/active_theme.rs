use flui_core::App;

use crate::ThemeData;

/// Extension trait providing `cx.theme()` shortcut.
///
/// Implemented for `App` — since `Context<T>` derefs to `App`,
/// this is available everywhere.
pub trait ActiveTheme {
    /// Get the current theme. Equivalent to `ThemeData::of(cx)`.
    fn theme(&self) -> &ThemeData;
}

impl ActiveTheme for App {
    fn theme(&self) -> &ThemeData {
        ThemeData::of(self)
    }
}
