use flui_core::{FontWeight, WindowAppearance, px};
use flui_theme::{Brightness, ShapeTheme, SpacingTheme, TextTheme, ThemeData, ThemeTextStyle};

use super::color_roles::{m3_dark_colors, m3_light_colors};

/// Material Design 3 theme builder.
pub struct MaterialTheme;

impl MaterialTheme {
    /// Build a light M3 theme.
    pub fn light() -> ThemeData {
        ThemeData {
            brightness: Brightness::Light,
            color_scheme: m3_light_colors(),
            text: Self::default_text_theme(),
            shape: ShapeTheme::default(),
            spacing: SpacingTheme::default(),
        }
    }

    /// Build a dark M3 theme.
    pub fn dark() -> ThemeData {
        ThemeData {
            brightness: Brightness::Dark,
            color_scheme: m3_dark_colors(),
            text: Self::default_text_theme(),
            shape: ShapeTheme::default(),
            spacing: SpacingTheme::default(),
        }
    }

    /// Resolve theme for the given window appearance.
    pub fn for_appearance(appearance: WindowAppearance) -> ThemeData {
        match appearance {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::dark(),
            _ => Self::light(),
        }
    }

    fn default_text_theme() -> TextTheme {
        TextTheme {
            display_large: ThemeTextStyle {
                size: px(57.),
                weight: FontWeight::NORMAL,
                line_height: px(64.),
                letter_spacing: -0.25,
            },
            display_medium: ThemeTextStyle {
                size: px(45.),
                weight: FontWeight::NORMAL,
                line_height: px(52.),
                letter_spacing: 0.,
            },
            display_small: ThemeTextStyle {
                size: px(36.),
                weight: FontWeight::NORMAL,
                line_height: px(44.),
                letter_spacing: 0.,
            },
            headline_large: ThemeTextStyle {
                size: px(32.),
                weight: FontWeight::NORMAL,
                line_height: px(40.),
                letter_spacing: 0.,
            },
            headline_medium: ThemeTextStyle {
                size: px(28.),
                weight: FontWeight::NORMAL,
                line_height: px(36.),
                letter_spacing: 0.,
            },
            headline_small: ThemeTextStyle {
                size: px(24.),
                weight: FontWeight::NORMAL,
                line_height: px(32.),
                letter_spacing: 0.,
            },
            title_large: ThemeTextStyle {
                size: px(22.),
                weight: FontWeight::NORMAL,
                line_height: px(28.),
                letter_spacing: 0.,
            },
            title_medium: ThemeTextStyle {
                size: px(16.),
                weight: FontWeight::MEDIUM,
                line_height: px(24.),
                letter_spacing: 0.15,
            },
            title_small: ThemeTextStyle {
                size: px(14.),
                weight: FontWeight::MEDIUM,
                line_height: px(20.),
                letter_spacing: 0.1,
            },
            body_large: ThemeTextStyle {
                size: px(16.),
                weight: FontWeight::NORMAL,
                line_height: px(24.),
                letter_spacing: 0.5,
            },
            body_medium: ThemeTextStyle {
                size: px(14.),
                weight: FontWeight::NORMAL,
                line_height: px(20.),
                letter_spacing: 0.25,
            },
            body_small: ThemeTextStyle {
                size: px(12.),
                weight: FontWeight::NORMAL,
                line_height: px(16.),
                letter_spacing: 0.4,
            },
            label_large: ThemeTextStyle {
                size: px(14.),
                weight: FontWeight::MEDIUM,
                line_height: px(20.),
                letter_spacing: 0.1,
            },
            label_medium: ThemeTextStyle {
                size: px(12.),
                weight: FontWeight::MEDIUM,
                line_height: px(16.),
                letter_spacing: 0.5,
            },
            label_small: ThemeTextStyle {
                size: px(11.),
                weight: FontWeight::MEDIUM,
                line_height: px(16.),
                letter_spacing: 0.5,
            },
        }
    }
}
