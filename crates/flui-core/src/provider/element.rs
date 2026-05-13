use std::{any::type_name, panic::Location};

use crate::{
    AnyElement, App, Element, ElementId, IntoElement, LayoutCx, LayoutId, PaintCx, PrepaintCx,
    RenderOnce, Window,
};

use super::InheritedValue;

/// Provides a value of type `T` to all descendant widgets during rendering.
///
/// Children can read the value with scoped inherited-value APIs:
/// `read_inherited::<T>()` for a non-subscribing lookup or `inherit::<T>()`
/// for a subscribing lookup on `LayoutCx`, `PrepaintCx`, or `PaintCx`.
///
/// Nesting: inner `Provider<T>` overrides outer `Provider<T>` for the subtree.
///
/// # Example
///
/// ```ignore
/// Provider::new(my_theme_data, my_child_widget)
/// ```
pub struct Provider<T: InheritedValue> {
    id: ElementId,
    source_location: Option<&'static Location<'static>>,
    value: T,
    child: AnyElement,
}

impl<T: InheritedValue> Provider<T> {
    /// Create a new Provider that makes `value` available to all descendants.
    #[track_caller]
    pub fn new(value: T, child: impl IntoElement) -> Self {
        let source_location = Location::caller();
        Self::new_with_id(
            provider_scope_id::<T>(ElementId::CodeLocation(*source_location)),
            Some(source_location),
            value,
            child,
        )
    }

    /// Create a new Provider with an explicit identity key.
    ///
    /// Use this when constructing repeated same-type providers from the same source location,
    /// such as inside loops. Prefer [`crate::Key::value`] for reorder-stable provider identity.
    #[track_caller]
    pub fn new_keyed(key: impl Into<ElementId>, value: T, child: impl IntoElement) -> Self {
        Self::new_with_id(
            provider_scope_id::<T>(key.into()),
            Some(Location::caller()),
            value,
            child,
        )
    }

    #[track_caller]
    fn new_with_id(
        id: ElementId,
        source_location: Option<&'static Location<'static>>,
        value: T,
        child: impl IntoElement,
    ) -> Self {
        Self {
            id,
            source_location,
            value,
            child: child.into_any_element(),
        }
    }
}

impl<T: InheritedValue> RenderOnce for Provider<T> {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        ProviderElement::<T> {
            id: self.id,
            source_location: self.source_location,
            value: self.value,
            child: self.child,
        }
    }
}

impl<T: InheritedValue> IntoElement for Provider<T> {
    type Element = ProviderElement<T>;

    fn into_element(self) -> Self::Element {
        ProviderElement::<T> {
            id: self.id,
            source_location: self.source_location,
            value: self.value,
            child: self.child,
        }
    }
}

/// The actual Element implementation that manages the push/pop lifecycle.
#[doc(hidden)]
pub struct ProviderElement<T: InheritedValue> {
    id: ElementId,
    source_location: Option<&'static Location<'static>>,
    value: T,
    child: AnyElement,
}

impl<T: InheritedValue> Element for ProviderElement<T> {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.source_location
    }

    fn request_layout(&mut self, cx: &mut LayoutCx<'_>) -> (LayoutId, Self::RequestLayoutState) {
        let scope_id = cx
            .global_id()
            .cloned()
            .expect("ProviderElement invariant: missing global_id during request_layout");

        let inspector_id = cx.inspector_id().cloned();
        let value = &self.value;
        let child = &mut self.child;
        let layout_id = cx.with_window_app(|window, app| {
            window.with_inherited_provider(&scope_id, value, app, |window, app| {
                let mut child_cx =
                    LayoutCx::new(window, app, Some(&scope_id), inspector_id.as_ref());
                child.request_layout(&mut child_cx)
            })
        });

        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        cx: &mut PrepaintCx<'_>,
        _request_layout: &mut Self::RequestLayoutState,
    ) {
        let scope_id = cx
            .global_id()
            .cloned()
            .expect("ProviderElement invariant: missing global_id during prepaint");

        let inspector_id = cx.inspector_id().cloned();
        let bounds = cx.bounds();
        let value = &self.value;
        let child = &mut self.child;
        cx.with_window_app(|window, app| {
            window.with_inherited_provider(&scope_id, value, app, |window, app| {
                let mut child_cx =
                    PrepaintCx::new(window, app, Some(&scope_id), inspector_id.as_ref(), bounds);
                child.prepaint(&mut child_cx);
            });
        });
    }

    fn paint(
        &mut self,
        cx: &mut PaintCx<'_>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
    ) {
        let scope_id = cx
            .global_id()
            .cloned()
            .expect("ProviderElement invariant: missing global_id during paint");

        let inspector_id = cx.inspector_id().cloned();
        let bounds = cx.bounds();
        let value = &self.value;
        let child = &mut self.child;
        cx.with_window_app(|window, app| {
            window.with_inherited_provider(&scope_id, value, app, |window, app| {
                let mut child_cx =
                    PaintCx::new(window, app, Some(&scope_id), inspector_id.as_ref(), bounds);
                child.paint(&mut child_cx);
            });
        });
    }
}

impl<T: InheritedValue> IntoElement for ProviderElement<T> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

fn provider_scope_id<T: InheritedValue>(base: ElementId) -> ElementId {
    (base, type_name::<T>()).into()
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::{
        cell::RefCell,
        panic::{self, AssertUnwindSafe},
        sync::Arc,
    };

    use crate::{
        AppContext, AvailableSpace, Context, DrawPhase, Empty, Entity, Key, Render, Style,
        StyleRefinement, TestAppContext, point, px, size,
    };

    use super::*;

    #[test]
    fn explicit_provider_key_is_part_of_scope_identity() {
        let provider = Provider::new_keyed("theme", 1i32, Empty);

        assert_eq!(
            provider.id,
            provider_scope_id::<i32>(ElementId::from("theme"))
        );
    }

    #[test]
    fn provider_keyed_identity_accepts_key_api() {
        let provider = Provider::new_keyed(Key::value("theme"), 1i32, Empty);

        assert_eq!(
            provider.id,
            provider_scope_id::<i32>(ElementId::from("theme"))
        );
    }

    #[test]
    fn provider_scope_identity_is_salted_by_value_type() {
        let base = ElementId::from("shared");

        assert_ne!(
            provider_scope_id::<i32>(base.clone()),
            provider_scope_id::<String>(base)
        );
    }

    #[test]
    fn source_location_provider_uses_code_location_fallback() {
        let provider = Provider::new(1i32, Empty);

        let ElementId::NamedChild(base, label) = provider.id else {
            panic!("provider id must be a named child");
        };

        assert!(matches!(&*base, ElementId::CodeLocation(_)));
        assert_eq!(label.as_ref(), type_name::<i32>());
    }

    #[test]
    fn source_location_provider_scope_normalizes_through_identity_stack() {
        let provider = Provider::new(1i32, Empty);
        let mut stack = crate::element::ElementIdStack::default();

        stack.push(provider.id);

        let ElementId::NamedChild(base, label) = &stack[0] else {
            panic!("provider id must remain type-salted after stack normalization");
        };

        assert!(matches!(&**base, ElementId::Local(_)));
        assert_eq!(label.as_ref(), type_name::<i32>());
    }

    #[test]
    fn nested_global_scope_ids_remain_distinct() {
        let outer = provider_scope_id::<i32>(ElementId::from("outer"));
        let inner = provider_scope_id::<i32>(ElementId::from("inner"));

        let outer_global = crate::GlobalElementId(Arc::from(vec![outer].into_boxed_slice()));
        let inner_global = crate::GlobalElementId(Arc::from(
            vec![outer_global[0].clone(), inner].into_boxed_slice(),
        ));

        assert_ne!(outer_global, inner_global);
    }

    #[derive(Default)]
    struct ReadRecord {
        layout: Option<i32>,
        prepaint: Option<i32>,
        paint: Option<i32>,
    }

    #[derive(Clone)]
    struct ReadProbe {
        record: Rc<RefCell<ReadRecord>>,
    }

    impl IntoElement for ReadProbe {
        type Element = Self;

        fn into_element(self) -> Self::Element {
            self
        }
    }

    impl Element for ReadProbe {
        type RequestLayoutState = ();
        type PrepaintState = ();

        fn id(&self) -> Option<ElementId> {
            Some("read-probe".into())
        }

        fn source_location(&self) -> Option<&'static Location<'static>> {
            Some(Location::caller())
        }

        fn request_layout(
            &mut self,
            cx: &mut LayoutCx<'_>,
        ) -> (LayoutId, Self::RequestLayoutState) {
            self.record.borrow_mut().layout = cx.inherit::<i32>();
            cx.with_window_app(|window, cx| (window.request_layout(Style::default(), [], cx), ()))
        }

        fn prepaint(
            &mut self,
            cx: &mut PrepaintCx<'_>,
            _request_layout: &mut Self::RequestLayoutState,
        ) -> Self::PrepaintState {
            self.record.borrow_mut().prepaint = cx.inherit::<i32>();
        }

        fn paint(
            &mut self,
            cx: &mut PaintCx<'_>,
            _request_layout: &mut Self::RequestLayoutState,
            _prepaint: &mut Self::PrepaintState,
        ) {
            self.record.borrow_mut().paint = cx.inherit::<i32>();
        }
    }

    struct ReadRoot {
        record: Rc<RefCell<ReadRecord>>,
        provider_value: Option<i32>,
    }

    impl Render for ReadRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let child = ReadProbe {
                record: self.record.clone(),
            }
            .into_any_element();

            if let Some(value) = self.provider_value {
                Provider::new(value, child).into_any_element()
            } else {
                child
            }
        }
    }

    #[crate::test]
    fn provider_reads_work_in_all_lifecycle_phases(cx: &mut TestAppContext) {
        let record = Rc::new(RefCell::new(ReadRecord::default()));
        let record_for_root = record.clone();
        let (_root, cx) = cx.add_window_view(move |_window, _cx| ReadRoot {
            record: record_for_root,
            provider_value: Some(11),
        });

        draw_window(cx);

        let record = record.borrow();
        assert_eq!(record.layout, Some(11));
        assert_eq!(record.prepaint, Some(11));
        assert_eq!(record.paint, Some(11));
    }

    #[crate::test]
    fn missing_provider_returns_none_in_lifecycle_phases(cx: &mut TestAppContext) {
        let record = Rc::new(RefCell::new(ReadRecord::default()));
        let record_for_root = record.clone();
        let (_root, cx) = cx.add_window_view(move |_window, _cx| ReadRoot {
            record: record_for_root,
            provider_value: None,
        });

        draw_window(cx);

        let record = record.borrow();
        assert_eq!(record.layout, None);
        assert_eq!(record.prepaint, None);
        assert_eq!(record.paint, None);
    }

    struct CachedReaderView {
        record: Rc<RefCell<ReadRecord>>,
    }

    impl Render for CachedReaderView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            ReadProbe {
                record: self.record.clone(),
            }
        }
    }

    struct ProviderToggleRoot {
        include_provider: bool,
        provider_value: i32,
        child: Entity<CachedReaderView>,
    }

    impl Render for ProviderToggleRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let child = self
                .child
                .clone()
                .into_element()
                .cached(StyleRefinement::default())
                .into_any_element();

            if self.include_provider {
                Provider::new_keyed("theme", self.provider_value, child).into_any_element()
            } else {
                child
            }
        }
    }

    fn draw_window(cx: &mut crate::VisualTestContext) {
        cx.update(|window, cx| {
            window.draw(cx).clear();
        });
    }

    #[crate::test]
    fn cached_view_does_not_reuse_value_after_ancestor_provider_is_removed(
        cx: &mut TestAppContext,
    ) {
        let record = Rc::new(RefCell::new(ReadRecord::default()));
        let record_for_child = record.clone();
        let (root, cx) = cx.add_window_view(move |_window, cx| {
            let child = cx.new(|_| CachedReaderView {
                record: record_for_child,
            });
            ProviderToggleRoot {
                include_provider: true,
                provider_value: 11,
                child,
            }
        });

        draw_window(cx);
        assert_eq!(record.borrow().layout, Some(11));

        root.update(cx, |root, cx| {
            root.include_provider = false;
            cx.notify();
        });

        draw_window(cx);
        assert_eq!(record.borrow().layout, None);
    }

    #[crate::test]
    fn cached_view_rerenders_same_frame_when_provider_value_changes(cx: &mut TestAppContext) {
        let record = Rc::new(RefCell::new(ReadRecord::default()));
        let record_for_child = record.clone();
        let (root, cx) = cx.add_window_view(move |_window, cx| {
            let child = cx.new(|_| CachedReaderView {
                record: record_for_child,
            });
            ProviderToggleRoot {
                include_provider: true,
                provider_value: 11,
                child,
            }
        });

        draw_window(cx);
        assert_eq!(record.borrow().layout, Some(11));

        root.update(cx, |root, cx| {
            root.provider_value = 12;
            cx.notify();
        });

        draw_window(cx);
        assert_eq!(record.borrow().layout, Some(12));
    }

    #[derive(Clone, Copy)]
    enum PanicPhase {
        Layout,
        Prepaint,
        Paint,
    }

    struct PanicProbe {
        phase: PanicPhase,
    }

    impl IntoElement for PanicProbe {
        type Element = Self;

        fn into_element(self) -> Self::Element {
            self
        }
    }

    impl Element for PanicProbe {
        type RequestLayoutState = ();
        type PrepaintState = ();

        fn id(&self) -> Option<ElementId> {
            Some("panic-probe".into())
        }

        fn source_location(&self) -> Option<&'static Location<'static>> {
            Some(Location::caller())
        }

        fn request_layout(
            &mut self,
            cx: &mut LayoutCx<'_>,
        ) -> (LayoutId, Self::RequestLayoutState) {
            if matches!(self.phase, PanicPhase::Layout) {
                panic!("layout panic");
            }

            cx.with_window_app(|window, cx| (window.request_layout(Style::default(), [], cx), ()))
        }

        fn prepaint(
            &mut self,
            _cx: &mut PrepaintCx<'_>,
            _request_layout: &mut Self::RequestLayoutState,
        ) -> Self::PrepaintState {
            if matches!(self.phase, PanicPhase::Prepaint) {
                panic!("prepaint panic");
            }
        }

        fn paint(
            &mut self,
            _cx: &mut PaintCx<'_>,
            _request_layout: &mut Self::RequestLayoutState,
            _prepaint: &mut Self::PrepaintState,
        ) {
            if matches!(self.phase, PanicPhase::Paint) {
                panic!("paint panic");
            }
        }
    }

    fn assert_provider_scope_restored_after_panic(
        cx: &mut crate::VisualTestContext,
        phase: PanicPhase,
    ) {
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            cx.draw(
                point(px(0.), px(0.)),
                size(
                    AvailableSpace::Definite(px(100.)),
                    AvailableSpace::Definite(px(100.)),
                ),
                move |_, _| Provider::new(11, PanicProbe { phase }).into_element(),
            );
        }));

        assert!(result.is_err());

        cx.update(|window, _| {
            assert_eq!(window.core.inherited_registry.active_scope_count::<i32>(), 0);
            window.core.invalidator.set_phase(DrawPhase::None);
        });
    }

    #[crate::test]
    fn provider_scope_is_restored_after_layout_panic(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        assert_provider_scope_restored_after_panic(cx, PanicPhase::Layout);
    }

    #[crate::test]
    fn provider_scope_is_restored_after_prepaint_panic(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        assert_provider_scope_restored_after_panic(cx, PanicPhase::Prepaint);
    }

    #[crate::test]
    fn provider_scope_is_restored_after_paint_panic(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        assert_provider_scope_restored_after_panic(cx, PanicPhase::Paint);
    }
}
