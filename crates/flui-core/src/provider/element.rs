use crate::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, RenderOnce, Window,
};

use super::{InheritedValue, stack};

/// Provides a value of type `T` to all descendant widgets during rendering.
///
/// Children can read the value with [`read::<T>()`](super::read) or
/// [`try_read::<T>()`](super::try_read).
///
/// Nesting: inner `Provider<T>` overrides outer `Provider<T>` for the subtree.
///
/// # Example
///
/// ```ignore
/// Provider::new(my_theme_data, my_child_widget)
/// ```
pub struct Provider<T: InheritedValue> {
    value: T,
    child: AnyElement,
}

impl<T: InheritedValue> Provider<T> {
    /// Create a new Provider that makes `value` available to all descendants.
    pub fn new(value: T, child: impl IntoElement) -> Self {
        Self {
            value,
            child: child.into_any_element(),
        }
    }
}

impl<T: InheritedValue> RenderOnce for Provider<T> {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        ProviderElement::<T> {
            value: self.value,
            child: self.child,
        }
    }
}

impl<T: InheritedValue> IntoElement for Provider<T> {
    type Element = crate::element::Component<Self>;

    fn into_element(self) -> Self::Element {
        crate::element::Component::new(self)
    }
}

/// The actual Element implementation that manages the push/pop lifecycle.
struct ProviderElement<T: InheritedValue> {
    value: T,
    child: AnyElement,
}

impl<T: InheritedValue> Element for ProviderElement<T> {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        stack::push(self.value.clone());
        let layout_id = self.child.request_layout(window, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
        stack::pop::<T>();
    }
}

impl<T: InheritedValue> IntoElement for ProviderElement<T> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
