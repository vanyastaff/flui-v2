use flui_core::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window, WindowAppearance, div,
};
use flui_theme::{ActiveTheme, Brightness, ThemeData, ThemeMode};

use crate::MaterialTheme;

/// Root widget for Material Design 3 applications.
///
/// Sets up the theme as a Global and wraps children in a
/// full-size container with background/text colors from the theme.
///
/// Equivalent to Flutter's `MaterialApp`.
///
/// # Example
///
/// ```ignore
/// MaterialApp::new()
///     .theme_mode(ThemeMode::Dark)
///     .child(Scaffold::new().body(my_content))
/// ```
#[derive(flui_core::IntoElement)]
pub struct MaterialApp {
    theme_mode: ThemeMode,
    light_theme: Option<ThemeData>,
    dark_theme: Option<ThemeData>,
    child: Option<AnyElement>,
}

impl MaterialApp {
    /// Create a new MaterialApp with default M3 themes.
    pub fn new() -> Self {
        Self {
            theme_mode: ThemeMode::System,
            light_theme: None,
            dark_theme: None,
            child: None,
        }
    }

    /// Set the theme mode.
    pub fn theme_mode(mut self, mode: ThemeMode) -> Self {
        self.theme_mode = mode;
        self
    }

    /// Override the light theme.
    pub fn light_theme(mut self, theme: ThemeData) -> Self {
        self.light_theme = Some(theme);
        self
    }

    /// Override the dark theme.
    pub fn dark_theme(mut self, theme: ThemeData) -> Self {
        self.dark_theme = Some(theme);
        self
    }

    /// Set the child widget (typically a `Scaffold`).
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }
}

impl RenderOnce for MaterialApp {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Resolve brightness from ThemeMode
        let brightness = match self.theme_mode {
            ThemeMode::Light => Brightness::Light,
            ThemeMode::Dark => Brightness::Dark,
            ThemeMode::System => match window.appearance() {
                WindowAppearance::Dark | WindowAppearance::VibrantDark => Brightness::Dark,
                _ => Brightness::Light,
            },
        };

        // Select the right theme
        let theme = match brightness {
            Brightness::Light => self.light_theme.unwrap_or_else(MaterialTheme::light),
            Brightness::Dark => self.dark_theme.unwrap_or_else(MaterialTheme::dark),
        };

        // Install as Global — every cx.theme() call downstream reads this
        cx.set_global(theme);

        // Render with theme-aware background and text color
        let t = cx.theme();
        let bg = t.color_scheme.background;
        let fg = t.color_scheme.on_background;

        let mut container = div().size_full().bg(bg).text_color(fg);

        if let Some(child) = self.child {
            container = container.child(child);
        }

        container
    }
}
