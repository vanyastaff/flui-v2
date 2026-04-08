//! # flui-widgets
//!
//! Headless, unstyled widget primitives for flui.
//!
//! This crate provides behavioral logic (interaction states, keyboard handling,
//! focus management) without any visual styling. Design systems like
//! `flui-material` layer visual styling on top using the `build()` pattern.
//!
//! ## Architecture Rules
//!
//! **Forbidden** in this crate:
//! - Colors (except `transparent`)
//! - Border radii, shadows, visual borders
//! - Any import from design system crates
//!
//! **Allowed**:
//! - Interaction state tracking (hover, press, focus, disabled)
//! - Keyboard navigation and shortcuts
//! - Layout constraints
//! - Event handling
//! - Animation hooks (without visual implementation)

pub mod layout;
pub mod primitives;
pub mod state;
/// Widget pattern documentation — maps Flutter concepts to flui equivalents.
pub mod widget;

// Re-export core types at crate root
pub use layout::{column, row, EdgeInsets, Expanded, Flexible, Padding, SizedBox, Stack};
pub use primitives::{
    ButtonBase, CheckboxBase, DialogBase, RadioBase, ScrollBase, SelectBase, SliderBase,
    SwitchBase, TextFieldBase, TextFieldVisuals, VirtualListBase,
};
pub use flui_core::{InheritedValue, Provider, read, try_read};
pub use state::InteractionState;
