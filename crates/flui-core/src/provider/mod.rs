mod element;
pub(crate) mod stack;

use std::any::Any;

/// Marker trait for values that can be propagated down the widget tree
/// via [`Provider<T>`](Provider).
///
/// Any type that is `Clone + Send + Sync + 'static` can be an inherited value.
pub trait InheritedValue: Any + Clone + Send + Sync + 'static {}

// Blanket impl: any compatible type is automatically InheritedValue.
impl<T: Any + Clone + Send + Sync + 'static> InheritedValue for T {}

pub use element::Provider;
pub use stack::{read, try_read};
