//! # Widget Patterns
//!
//! flui uses the same patterns as Flutter, mapped to Rust traits:
//!
//! | Flutter             | flui                                         |
//! |---------------------|----------------------------------------------|
//! | `StatelessWidget`   | `#[derive(IntoElement)]` + `impl RenderOnce` |
//! | `StatefulWidget`    | `impl Render` + `Entity<T>`                  |
//! | `Widget.build()`    | `RenderOnce::render()` / `Render::render()`  |
//! | `BuildContext`      | `&mut Window` + `&mut App`                   |
//! | `State<T>`          | `&mut Context<Self>` in `Render::render()`   |
//! | `setState()`        | `cx.notify()`                                |
//!
//! ## StatelessWidget (consumed on render, no persistent state)
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
//! ## StatefulWidget (persistent state across frames)
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
