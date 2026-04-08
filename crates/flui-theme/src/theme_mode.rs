/// Controls which brightness variant is applied.
///
/// - `System`: follows OS preference via `window.appearance()`
/// - `Light`: always light theme
/// - `Dark`: always dark theme
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeMode {
    /// Follow OS dark/light preference.
    #[default]
    System,
    /// Always use light theme.
    Light,
    /// Always use dark theme.
    Dark,
}
