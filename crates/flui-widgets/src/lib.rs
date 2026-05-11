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

// Re-export core provider construction types at crate root. Inherited reads are
// scoped to `Window` / lifecycle contexts in K01 and are not global functions.
pub use flui_core::{InheritedValue, Provider};
pub use layout::{EdgeInsets, Expanded, Flexible, Padding, SizedBox, Stack, column, row};
pub use primitives::{
    ButtonBase, CheckboxBase, DialogBase, RadioBase, ScrollBase, SelectBase, SliderBase,
    SwitchBase, TextFieldBase, TextFieldVisuals, VirtualListBase,
};
pub use state::InteractionState;
