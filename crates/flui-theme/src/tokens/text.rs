use flui_core::{FontWeight, Pixels};

/// A text style definition for a specific typography role.
#[derive(Clone, Debug)]
pub struct ThemeTextStyle {
    pub size: Pixels,
    pub weight: FontWeight,
    pub line_height: Pixels,
    pub letter_spacing: f32,
}

/// Complete set of typography roles.
///
/// Follows Material Design 3 type scale with 15 roles.
#[derive(Clone, Debug)]
pub struct TextTheme {
    pub display_large: ThemeTextStyle,
    pub display_medium: ThemeTextStyle,
    pub display_small: ThemeTextStyle,
    pub headline_large: ThemeTextStyle,
    pub headline_medium: ThemeTextStyle,
    pub headline_small: ThemeTextStyle,
    pub title_large: ThemeTextStyle,
    pub title_medium: ThemeTextStyle,
    pub title_small: ThemeTextStyle,
    pub body_large: ThemeTextStyle,
    pub body_medium: ThemeTextStyle,
    pub body_small: ThemeTextStyle,
    pub label_large: ThemeTextStyle,
    pub label_medium: ThemeTextStyle,
    pub label_small: ThemeTextStyle,
}
