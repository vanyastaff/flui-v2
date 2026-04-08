use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;

use super::InheritedValue;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_read_pop() {
        push(42i32);
        assert_eq!(read::<i32>(), 42);
        pop::<i32>();
    }

    #[test]
    fn test_try_read_empty() {
        assert_eq!(try_read::<f64>(), None);
    }

    #[test]
    fn test_nested_override() {
        push(1i32);
        assert_eq!(read::<i32>(), 1);
        push(2i32);
        assert_eq!(read::<i32>(), 2);
        pop::<i32>();
        assert_eq!(read::<i32>(), 1);
        pop::<i32>();
    }

    #[test]
    fn test_multiple_types() {
        push(42i32);
        push("hello".to_string());
        assert_eq!(read::<i32>(), 42);
        assert_eq!(read::<String>(), "hello");
        pop::<String>();
        pop::<i32>();
    }

    #[test]
    #[should_panic(expected = "No Provider<i32>")]
    fn test_read_panics_when_empty() {
        let _ = read::<i32>();
    }
}
