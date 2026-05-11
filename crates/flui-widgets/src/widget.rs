//! # Engine Component Patterns
//!
//! `flui-widgets` currently builds on engine-level flui-core recipes. These
//! are intentionally different from the future `flui-framework::Widget` API.
//!
//! | Current flui-core concept | Use for |
//! |---------------------------|---------|
//! | `RenderOnce` + `#[derive(IntoElement)]` | Consuming stateless engine recipes |
//! | `ElementBuilder` + `build_element(...)` | Immutable recipes built from `&self` |
//! | `Render` + `Entity<T>` | Mutable, entity-backed engine views and roots |
//! | `Window::use_state` / `use_keyed_state` | Element-scoped state in current engine code |
//! | `Context<Self>::notify()` | Invalidating mutable `Render` views |
//!
//! The final Flutter-style `Widget`, `State`, `BuildCx`, reconciliation, and
//! `setState` surface belongs to the planned `flui-framework` crate.
//!
//! ## Consuming engine recipe
//!
//! ```ignore
//! #[derive(IntoElement)]
//! struct Greeting { name: SharedString }
//!
//! impl RenderOnce for Greeting {
//!     fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
//!         div().child(format!("Hello, {}!", self.name))
//!     }
//! }
//! ```
//!
//! ## Immutable engine recipe
//!
//! ```ignore
//! struct Greeting { name: SharedString }
//!
//! impl ElementBuilder for Greeting {
//!     fn build(&self, _cx: &mut ElementBuildCx<'_>) -> impl IntoElement {
//!         div().child(format!("Hello, {}!", self.name))
//!     }
//! }
//!
//! let element = build_element(Greeting { name: "Ada".into() });
//! ```
//!
//! ## Mutable engine view
//!
//! ```ignore
//! struct Counter { count: i32 }
//!
//! impl Render for Counter {
//!     fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
//!         div()
//!             .on_click(cx.listener(|this, _, _, cx| {
//!                 this.count += 1;
//!                 cx.notify();
//!             }))
//!             .child(format!("Count: {}", self.count))
//!     }
//! }
//! ```
