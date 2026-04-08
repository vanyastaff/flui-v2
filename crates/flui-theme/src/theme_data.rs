use flui_core::Global;

use crate::{Brightness, ColorScheme, ShapeTheme, SpacingTheme, TextTheme};

/// The complete token set for a theme.
///
/// Stored as a `Global` and accessed via `Theme::of(cx)` or `cx.theme()`.
/// Design systems (Material, Fluent, etc.) populate this with their values.
///
/// # Example
///
/// ```ignore
/// // In MaterialApp::render():
/// let theme = ThemeData {
///     brightness: Brightness::Dark,
///     color_scheme: ColorScheme::m3_dark(),
///     text: TextTheme::m3_defaults(),
///     shape: ShapeTheme::default(),
///     spacing: SpacingTheme::default(),
/// };
/// cx.set_global(theme);
/// ```
#[derive(Clone, Debug)]
pub struct ThemeData {
    pub brightness: Brightness,
    pub color_scheme: ColorScheme,
    pub text: TextTheme,
    pub shape: ShapeTheme,
    pub spacing: SpacingTheme,
}

impl Global for ThemeData {}

/// Flutter-style `Theme.of(context)` — reads the current theme from Global state.
///
/// # Panics
///
/// Panics if no theme has been set (e.g., no `MaterialApp` or `ThemeProvider` in the tree).
impl ThemeData {
    /// Read the current theme. Flutter equivalent: `Theme.of(context)`.
    pub fn of(cx: &flui_core::App) -> &Self {
        cx.global::<Self>()
    }
}
