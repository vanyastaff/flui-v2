use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;

use super::inherited::InheritedValue;

thread_local! {
    static PROVIDER_STACKS: RefCell<HashMap<TypeId, Vec<Box<dyn Any>>>> = RefCell::new(HashMap::new());
}

/// Push a value onto the thread-local provider stack for type `T`.
///
/// Must be paired with a corresponding [`pop::<T>()`] call after the child
/// subtree has finished rendering.
pub fn push<T: InheritedValue>(value: T) {
    PROVIDER_STACKS.with(|stacks| {
        stacks
            .borrow_mut()
            .entry(TypeId::of::<T>())
            .or_default()
            .push(Box::new(value));
    });
}

/// Pop the most recent value from the provider stack for type `T`.
///
/// # Panics
///
/// Panics if the stack for `T` is empty (mismatched push/pop).
pub fn pop<T: InheritedValue>() {
    PROVIDER_STACKS.with(|stacks| {
        let mut stacks = stacks.borrow_mut();
        let stack = stacks
            .get_mut(&TypeId::of::<T>())
            .expect("Provider::pop called without matching push");
        stack.pop().expect("Provider stack underflow");
    });
}

/// Read the current value of type `T` from the provider stack.
///
/// Returns `None` if no `Provider<T>` exists in the current render tree.
pub fn try_read<T: InheritedValue>() -> Option<T> {
    PROVIDER_STACKS.with(|stacks| {
        stacks
            .borrow()
            .get(&TypeId::of::<T>())
            .and_then(|stack| stack.last())
            .and_then(|val| val.downcast_ref::<T>())
            .cloned()
    })
}

/// Read the current value of type `T` from the provider stack.
///
/// # Panics
///
/// Panics if no `Provider<T>` exists in the current render tree.
pub fn read<T: InheritedValue>() -> T {
    try_read::<T>().unwrap_or_else(|| {
        panic!(
            "No Provider<{}> found in the current render tree. \
             Wrap a parent widget with Provider::new(value, child).",
            std::any::type_name::<T>()
        )
    })
}
