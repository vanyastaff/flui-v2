//! # flui-material
//!
//! Material Design 3 components for flui.
//!
//! Provides `MaterialApp`, themed buttons, text fields, scaffold, cards,
//! and other M3 components built on `flui-widgets` headless primitives.
//! Current components use flui-core engine recipes; the final Framework-tier
//! `Widget` API is deferred to `flui-framework`.
//!
//! ## Quick Start
//!
//! ```ignore
//! use flui_material::*;
//!
//! MaterialApp::new()
//!     .theme_mode(ThemeMode::Dark)
//!     .child(
//!         Scaffold::new()
//!             .app_bar(AppBar::new("My App"))
//!             .body(FilledButton::new("btn", "Click me"))
//!     )
//! ```

pub mod app;
pub mod buttons;
pub mod dialog;
pub mod scaffold;
pub mod theme;

mod card;
mod divider;
mod list_view;
mod text_field;

pub use app::MaterialApp;
pub use buttons::{ElevatedButton, FilledButton, IconButton, OutlinedButton, TextButton};
pub use card::{Card, CardVariant};
pub use dialog::AlertDialog;
pub use divider::Divider;
pub use list_view::ListView;
pub use scaffold::{AppBar, Scaffold};
pub use text_field::{InputDecoration, TextField};
pub use theme::MaterialTheme;

// Re-export theme primitives for convenience
pub use flui_theme::{ActiveTheme, Brightness, ColorScheme, ThemeData, ThemeMode};
pub use flui_widgets::{Expanded, Flexible, Padding, SizedBox, Stack, column, row};
