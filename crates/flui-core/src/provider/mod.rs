mod element;
pub(crate) mod registry;

use std::any::Any;

/// Marker trait for values that can be propagated down the element tree via
/// [`Provider<T>`](Provider).
///
/// `PartialEq` drives K01 provider invalidation: unchanged values keep their
/// provider version and do not dirty inherited dependents.
pub trait InheritedValue: Any + Clone + PartialEq + Send + Sync + 'static {}

// Blanket impl: any compatible type is automatically InheritedValue.
impl<T: Any + Clone + PartialEq + Send + Sync + 'static> InheritedValue for T {}

pub use element::Provider;
