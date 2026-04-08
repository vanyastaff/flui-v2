//! # flui-theme
//!
//! Universal theme token system for flui — design-system agnostic.
//!
//! Provides `ThemeData` (stored as a `Global`), `ColorScheme`, `TextTheme`,
//! `ShapeTheme`, `SpacingTheme`, and the `ActiveTheme` extension trait.
//!
//! Design systems like `flui-material` populate `ThemeData` with their values.
//! Custom themes can be created without any design system dependency.
//!
//! ## Usage
//!
//! ```ignore
//! use flui_theme::{ThemeData, ActiveTheme};
//!
//! // In any render method:
//! let theme = cx.theme();  // or ThemeData::of(cx)
//! div().bg(theme.color_scheme.primary)
//! ```

mod active_theme;
mod brightness;
mod theme_data;
mod theme_mode;
pub mod tokens;

pub use active_theme::ActiveTheme;
pub use brightness::Brightness;
pub use theme_data::ThemeData;
pub use theme_mode::ThemeMode;
pub use tokens::{ColorScheme, ShapeTheme, SpacingTheme, TextTheme, ThemeTextStyle};

// Re-export flui-widgets for convenience
pub use flui_widgets;
