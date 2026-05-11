//! Engine-level immutable element recipes.
//!
//! This module is the K03 bridge between mutable engine views and the future
//! Framework-tier widget model. It lets immutable recipe values build the
//! existing [`Element`](crate::Element) tree without introducing
//! `flui-framework::Widget`, reconciliation, or a second runtime tree.

use crate::{
    AnyElement, App, Element, ElementId, GlobalElementId, InheritedValue, InspectorElementId,
    IntoElement, LayoutCx, LayoutId, PaintCx, PrepaintCx, SharedString, Window,
};
use std::{any::type_name, panic::Location};

/// Build-time access for [`ElementBuilder`] values.
///
/// `ElementBuildCx` is an engine context, not the final Framework-tier
/// `BuildCx`. It exposes the same low-level runtime handles and inherited
/// value substrate that engine elements already use, while leaving
/// reconciliation, state maps, and `setState` to `flui-framework`.
pub struct ElementBuildCx<'a> {
    window: &'a mut Window,
    app: &'a mut App,
    global_id: Option<&'a GlobalElementId>,
    inspector_id: Option<&'a InspectorElementId>,
}

impl<'a> ElementBuildCx<'a> {
    pub(crate) fn new(
        window: &'a mut Window,
        app: &'a mut App,
        global_id: Option<&'a GlobalElementId>,
        inspector_id: Option<&'a InspectorElementId>,
    ) -> Self {
        Self {
            window,
            app,
            global_id,
            inspector_id,
        }
    }

    /// Returns the globally unique id for this build boundary, if available.
    pub fn global_id(&self) -> Option<&GlobalElementId> {
        self.global_id
    }

    /// Returns the inspector id associated with this build boundary, if any.
    pub fn inspector_id(&self) -> Option<&InspectorElementId> {
        self.inspector_id
    }

    /// Reborrow the current window.
    pub fn window(&mut self) -> &mut Window {
        &mut *self.window
    }

    /// Reborrow the current application state.
    pub fn app(&mut self) -> &mut App {
        &mut *self.app
    }

    /// Reborrow both window and app for existing engine APIs.
    pub fn with_window_app<R>(&mut self, f: impl FnOnce(&mut Window, &mut App) -> R) -> R {
        f(&mut *self.window, &mut *self.app)
    }

    /// Reads the nearest inherited value of type `T` without subscribing this
    /// build boundary to future provider changes.
    pub fn read_inherited<T: InheritedValue>(&self) -> Option<T> {
        self.window.read_inherited::<T>()
    }

    /// Reads the nearest inherited value of type `T` and subscribes the
    /// current view through this build boundary's stable element id.
    pub fn inherit<T: InheritedValue>(&mut self) -> Option<T> {
        debug_assert!(
            self.global_id.is_some(),
            "`inherit()` requires a stable build element id; use `read_inherited()` for non-subscribing lookups",
        );
        let Some(global_id) = self.global_id.cloned() else {
            return None;
        };
        debug_assert!(
            self.window.try_current_view().is_some(),
            "`inherit()` requires an active view; use `read_inherited()` for non-subscribing lookups",
        );
        let Some(view_id) = self.window.try_current_view() else {
            return None;
        };

        self.window.inherit_inherited::<T>(&global_id, view_id)
    }
}

/// An immutable engine recipe that builds an element tree.
///
/// This is not the Framework-tier `Widget` trait. `ElementBuilder` has no
/// reconciliation hooks, state object, or object-safe erased form. It is a
/// generic bridge for values that can build existing engine elements from
/// `&self`.
pub trait ElementBuilder: 'static {
    /// Builds this recipe into an element tree.
    fn build(&self, cx: &mut ElementBuildCx<'_>) -> impl IntoElement;
}

/// Wraps an [`ElementBuilder`] so it can participate in the existing element tree.
///
/// Construct this with [`build_element`] or [`BuildElement::new`]. Use
/// [`BuildElement::key`] for repeated or reordered builder boundaries whose
/// inner state or provider identity should follow application data.
pub struct BuildElement<B: ElementBuilder> {
    builder: B,
    key: Option<ElementId>,
    source: &'static Location<'static>,
}

impl<B: ElementBuilder> BuildElement<B> {
    /// Create a new element-builder adapter.
    #[track_caller]
    pub fn new(builder: B) -> Self {
        Self {
            builder,
            key: None,
            source: Location::caller(),
        }
    }

    /// Assign an explicit identity key to this build boundary.
    pub fn key(mut self, key: impl Into<ElementId>) -> Self {
        self.key = Some(key.into());
        self
    }
}

/// Convert an [`ElementBuilder`] value into an element-tree adapter.
#[track_caller]
pub fn build_element<B: ElementBuilder>(builder: B) -> BuildElement<B> {
    BuildElement::new(builder)
}

fn type_namespace<T: 'static>() -> ElementId {
    ElementId::Name(SharedString::new_static(type_name::<T>()))
}

fn prepaint_build_element(
    (element, name): &mut (AnyElement, &'static str),
    cx: &mut PrepaintCx<'_>,
) {
    let global_id = cx.global_id().cloned();
    let inspector_id = cx.inspector_id().cloned();
    let bounds = cx.bounds();
    cx.with_window_app(|window, app| {
        window.with_id(ElementId::Name(SharedString::new_static(name)), |window| {
            let mut cx = PrepaintCx::new(
                window,
                app,
                global_id.as_ref(),
                inspector_id.as_ref(),
                bounds,
            );
            element.prepaint(&mut cx);
        });
    })
}

fn paint_build_element((element, name): &mut (AnyElement, &'static str), cx: &mut PaintCx<'_>) {
    let global_id = cx.global_id().cloned();
    let inspector_id = cx.inspector_id().cloned();
    let bounds = cx.bounds();
    cx.with_window_app(|window, app| {
        window.with_id(ElementId::Name(SharedString::new_static(name)), |window| {
            let mut cx = PaintCx::new(
                window,
                app,
                global_id.as_ref(),
                inspector_id.as_ref(),
                bounds,
            );
            element.paint(&mut cx);
        });
    })
}

impl<B: ElementBuilder> Element for BuildElement<B> {
    type RequestLayoutState = (AnyElement, &'static str);
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(
            self.key
                .clone()
                .unwrap_or(ElementId::CodeLocation(*self.source)),
        )
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        Some(self.source)
    }

    fn request_layout(&mut self, cx: &mut LayoutCx<'_>) -> (LayoutId, Self::RequestLayoutState) {
        let inspector_id = cx.inspector_id().cloned();
        cx.with_window_app(|window, app| {
            window.with_global_id(type_namespace::<B>(), |global_id, window| {
                let mut element = {
                    let mut build_cx =
                        ElementBuildCx::new(window, app, Some(global_id), inspector_id.as_ref());
                    self.builder.build(&mut build_cx).into_any_element()
                };

                let mut child_cx = LayoutCx::new(window, app, None, None);
                let layout_id = element.request_layout(&mut child_cx);
                (layout_id, (element, type_name::<B>()))
            })
        })
    }

    fn prepaint(&mut self, cx: &mut PrepaintCx<'_>, state: &mut Self::RequestLayoutState) {
        prepaint_build_element(state, cx);
    }

    fn paint(
        &mut self,
        cx: &mut PaintCx<'_>,
        state: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
    ) {
        paint_build_element(state, cx);
    }
}

impl<B: ElementBuilder> IntoElement for BuildElement<B> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AppContext as _, AvailableSpace, Component, Context, Empty, Entity, Key, ParentElement,
        Provider, Render, RenderOnce, Style, StyleRefinement, TestAppContext, VisualTestContext,
        deferred, div, point, px, size,
    };
    use std::{cell::RefCell, rc::Rc};

    #[derive(Clone)]
    struct ProbeBuilder {
        label: usize,
        record: Rc<RefCell<Vec<(usize, GlobalElementId)>>>,
    }

    impl ElementBuilder for ProbeBuilder {
        fn build(&self, _cx: &mut ElementBuildCx<'_>) -> impl IntoElement {
            ProbeElement {
                label: self.label,
                record: self.record.clone(),
            }
        }
    }

    struct ProbeElement {
        label: usize,
        record: Rc<RefCell<Vec<(usize, GlobalElementId)>>>,
    }

    impl IntoElement for ProbeElement {
        type Element = Self;

        fn into_element(self) -> Self::Element {
            self
        }
    }

    impl Element for ProbeElement {
        type RequestLayoutState = ();
        type PrepaintState = ();

        fn id(&self) -> Option<ElementId> {
            Some(ElementId::Name(SharedString::new_static("build-probe")))
        }

        fn source_location(&self) -> Option<&'static Location<'static>> {
            None
        }

        fn request_layout(
            &mut self,
            cx: &mut LayoutCx<'_>,
        ) -> (LayoutId, Self::RequestLayoutState) {
            cx.with_window_app(|window, cx| {
                let mut style = Style::default();
                style.size = size(px(1.).into(), px(1.).into());
                (window.request_layout(style, [], cx), ())
            })
        }

        fn prepaint(
            &mut self,
            cx: &mut PrepaintCx<'_>,
            _request_layout: &mut Self::RequestLayoutState,
        ) -> Self::PrepaintState {
            let global_id = cx
                .global_id()
                .cloned()
                .expect("build child should inherit build boundary global id");
            self.record.borrow_mut().push((self.label, global_id));
        }

        fn paint(
            &mut self,
            _cx: &mut PaintCx<'_>,
            _request_layout: &mut Self::RequestLayoutState,
            _prepaint: &mut Self::PrepaintState,
        ) {
        }
    }

    fn draw_element(cx: &mut VisualTestContext, element: impl Element) {
        cx.draw(
            point(px(0.), px(0.)),
            size(
                AvailableSpace::Definite(px(10.)),
                AvailableSpace::Definite(px(10.)),
            ),
            move |_, _| element,
        );
    }

    fn first_local_source_line(global_id: &GlobalElementId) -> u32 {
        global_id
            .iter()
            .find_map(|segment| match segment {
                ElementId::Local(local) => Some(local.source_location().line()),
                _ => None,
            })
            .expect("build boundary global id should contain a local source segment")
    }

    #[crate::test]
    fn element_builder_builds_from_shared_reference(cx: &mut TestAppContext) {
        let record = Rc::new(RefCell::new(Vec::new()));
        let cx = cx.add_empty_window();

        draw_element(
            cx,
            build_element(ProbeBuilder {
                label: 1,
                record: record.clone(),
            }),
        );

        let record = record.borrow();
        assert_eq!(record.len(), 1);
        assert_eq!(record[0].0, 1);
    }

    #[crate::test]
    fn repeated_build_elements_from_same_callsite_get_local_identity(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();

        cx.draw(
            point(px(0.), px(0.)),
            size(
                AvailableSpace::Definite(px(10.)),
                AvailableSpace::Definite(px(10.)),
            ),
            move |_, _| {
                let children = (0..2).map(|_| build_element(EmptyBuilder).into_any_element());
                div().children(children)
            },
        );
    }

    #[crate::test]
    fn parent_child_preserves_build_element_callsite_identity(cx: &mut TestAppContext) {
        let record = Rc::new(RefCell::new(Vec::new()));
        let record_for_draw = record.clone();
        let cx = cx.add_empty_window();

        cx.draw(
            point(px(0.), px(0.)),
            size(
                AvailableSpace::Definite(px(10.)),
                AvailableSpace::Definite(px(10.)),
            ),
            move |_, _| {
                div()
                    .child(build_element(ProbeBuilder {
                        label: 1,
                        record: record_for_draw.clone(),
                    }))
                    .child(build_element(ProbeBuilder {
                        label: 2,
                        record: record_for_draw,
                    }))
            },
        );

        let record = record.borrow();
        let first = record
            .iter()
            .find_map(|(label, global_id)| (*label == 1).then_some(global_id))
            .expect("first build element should record its global id");
        let second = record
            .iter()
            .find_map(|(label, global_id)| (*label == 2).then_some(global_id))
            .expect("second build element should record its global id");

        assert_ne!(
            first_local_source_line(first),
            first_local_source_line(second),
            "ParentElement::child must preserve the user's build_element callsite"
        );
    }

    struct RenderOnceProbe {
        label: usize,
        record: Rc<RefCell<Vec<(usize, GlobalElementId)>>>,
    }

    impl RenderOnce for RenderOnceProbe {
        fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
            ProbeElement {
                label: self.label,
                record: self.record,
            }
        }
    }

    #[crate::test]
    fn render_once_components_still_work_next_to_build_elements(cx: &mut TestAppContext) {
        let record = Rc::new(RefCell::new(Vec::new()));
        let record_for_draw = record.clone();
        let cx = cx.add_empty_window();

        cx.draw(
            point(px(0.), px(0.)),
            size(
                AvailableSpace::Definite(px(10.)),
                AvailableSpace::Definite(px(10.)),
            ),
            move |_, _| {
                div()
                    .child(
                        Component::new(RenderOnceProbe {
                            label: 1,
                            record: record_for_draw.clone(),
                        })
                        .key(Key::value("render-once")),
                    )
                    .child(build_element(ProbeBuilder {
                        label: 2,
                        record: record_for_draw,
                    }))
            },
        );

        let labels = record
            .borrow()
            .iter()
            .map(|(label, _)| *label)
            .collect::<Vec<_>>();
        assert_eq!(labels, vec![1, 2]);
    }

    #[derive(Clone, Copy)]
    struct EmptyBuilder;

    impl ElementBuilder for EmptyBuilder {
        fn build(&self, _cx: &mut ElementBuildCx<'_>) -> impl IntoElement {
            Empty
        }
    }

    #[derive(Clone)]
    struct ProviderReadBuilder {
        read_record: Rc<RefCell<Option<i32>>>,
        inherit_record: Rc<RefCell<Option<i32>>>,
    }

    impl ElementBuilder for ProviderReadBuilder {
        fn build(&self, cx: &mut ElementBuildCx<'_>) -> impl IntoElement {
            *self.read_record.borrow_mut() = cx.read_inherited::<i32>();
            *self.inherit_record.borrow_mut() = cx.inherit::<i32>();
            Empty
        }
    }

    struct ProviderReadRoot {
        read_record: Rc<RefCell<Option<i32>>>,
        inherit_record: Rc<RefCell<Option<i32>>>,
    }

    impl Render for ProviderReadRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            Provider::new(
                11,
                build_element(ProviderReadBuilder {
                    read_record: self.read_record.clone(),
                    inherit_record: self.inherit_record.clone(),
                }),
            )
        }
    }

    #[crate::test]
    fn build_context_reads_inherited_values(cx: &mut TestAppContext) {
        let read_record = Rc::new(RefCell::new(None));
        let inherit_record = Rc::new(RefCell::new(None));
        let read_record_for_root = read_record.clone();
        let inherit_record_for_root = inherit_record.clone();
        let (_root, cx) = cx.add_window_view(move |_window, _cx| ProviderReadRoot {
            read_record: read_record_for_root,
            inherit_record: inherit_record_for_root,
        });

        draw_window(cx);

        assert_eq!(*read_record.borrow(), Some(11));
        assert_eq!(*inherit_record.borrow(), Some(11));
    }

    struct BuildStateProbe {
        label: usize,
        record: Rc<RefCell<Vec<(usize, crate::EntityId)>>>,
    }

    impl IntoElement for BuildStateProbe {
        type Element = Self;

        fn into_element(self) -> Self::Element {
            self
        }
    }

    impl Element for BuildStateProbe {
        type RequestLayoutState = ();
        type PrepaintState = ();

        fn id(&self) -> Option<ElementId> {
            None
        }

        fn source_location(&self) -> Option<&'static Location<'static>> {
            None
        }

        fn request_layout(
            &mut self,
            cx: &mut LayoutCx<'_>,
        ) -> (LayoutId, Self::RequestLayoutState) {
            cx.with_window_app(|window, cx| {
                let mut style = Style::default();
                style.size = size(px(1.).into(), px(1.).into());
                (window.request_layout(style, [], cx), ())
            })
        }

        fn prepaint(
            &mut self,
            cx: &mut PrepaintCx<'_>,
            _request_layout: &mut Self::RequestLayoutState,
        ) -> Self::PrepaintState {
            let state =
                cx.with_window_app(|window, cx| window.use_state(cx, |_, _| BuildStateProbeState));
            self.record
                .borrow_mut()
                .push((self.label, state.entity_id()));
        }

        fn paint(
            &mut self,
            _cx: &mut PaintCx<'_>,
            _request_layout: &mut Self::RequestLayoutState,
            _prepaint: &mut Self::PrepaintState,
        ) {
        }
    }

    struct BuildStateProbeState;

    struct StateBuilder {
        label: usize,
        record: Rc<RefCell<Vec<(usize, crate::EntityId)>>>,
    }

    impl ElementBuilder for StateBuilder {
        fn build(&self, _cx: &mut ElementBuildCx<'_>) -> impl IntoElement {
            BuildStateProbe {
                label: self.label,
                record: self.record.clone(),
            }
        }
    }

    struct BuildStateRoot {
        reversed: bool,
        record: Rc<RefCell<Vec<(usize, crate::EntityId)>>>,
    }

    impl Render for BuildStateRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let order = if self.reversed { [2, 1] } else { [1, 2] };
            let children = order.into_iter().map(|label| {
                build_element(StateBuilder {
                    label,
                    record: self.record.clone(),
                })
                .key(Key::value(("state-builder", label)))
                .into_any_element()
            });

            div().children(children)
        }
    }

    fn draw_window(cx: &mut VisualTestContext) {
        cx.update(|window, cx| {
            window.draw(cx).clear();
        });
    }

    struct MutableRenderRoot {
        render_count: usize,
        record: Rc<RefCell<Vec<usize>>>,
    }

    impl Render for MutableRenderRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            self.render_count += 1;
            self.record.borrow_mut().push(self.render_count);
            build_element(EmptyBuilder)
        }
    }

    #[crate::test]
    fn mutable_render_roots_still_render_build_elements(cx: &mut TestAppContext) {
        let record = Rc::new(RefCell::new(Vec::new()));
        let record_for_root = record.clone();
        let (root, cx) = cx.add_window_view(move |_window, _cx| MutableRenderRoot {
            render_count: 0,
            record: record_for_root,
        });

        record.borrow_mut().clear();
        root.update(cx, |root, cx| {
            root.render_count = 40;
            cx.notify();
        });
        draw_window(cx);

        assert!(
            record.borrow().iter().any(|count| *count >= 41),
            "mutable Render root should keep rendering after state changes"
        );
    }

    fn recorded_id(
        record: &Rc<RefCell<Vec<(usize, crate::EntityId)>>>,
        label: usize,
    ) -> crate::EntityId {
        record
            .borrow()
            .iter()
            .find_map(|(recorded_label, entity_id)| {
                (*recorded_label == label).then_some(*entity_id)
            })
            .expect("state probe label should have been recorded")
    }

    #[crate::test]
    fn keyed_build_element_boundary_preserves_inner_state_across_reorder(cx: &mut TestAppContext) {
        let record = Rc::new(RefCell::new(Vec::new()));
        let record_for_root = record.clone();
        let (root, cx) = cx.add_window_view(move |_window, _cx| BuildStateRoot {
            reversed: false,
            record: record_for_root,
        });

        draw_window(cx);
        let first_1 = recorded_id(&record, 1);
        let first_2 = recorded_id(&record, 2);

        record.borrow_mut().clear();
        root.update(cx, |root, cx| {
            root.reversed = true;
            cx.notify();
        });
        draw_window(cx);

        assert_eq!(first_1, recorded_id(&record, 1));
        assert_eq!(first_2, recorded_id(&record, 2));
    }

    struct CachedBuildReaderView {
        read_record: Rc<RefCell<Option<i32>>>,
        inherit_record: Rc<RefCell<Option<i32>>>,
    }

    impl Render for CachedBuildReaderView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            build_element(ProviderReadBuilder {
                read_record: self.read_record.clone(),
                inherit_record: self.inherit_record.clone(),
            })
        }
    }

    struct CachedBuildProviderRoot {
        provider_value: i32,
        child: Entity<CachedBuildReaderView>,
    }

    impl Render for CachedBuildProviderRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let child = self
                .child
                .clone()
                .into_element()
                .cached(StyleRefinement::default())
                .into_any_element();

            Provider::new_keyed("theme", self.provider_value, child)
        }
    }

    #[crate::test]
    fn cached_view_revalidates_provider_dependencies_through_build_element(
        cx: &mut TestAppContext,
    ) {
        let read_record = Rc::new(RefCell::new(None));
        let inherit_record = Rc::new(RefCell::new(None));
        let read_record_for_child = read_record.clone();
        let inherit_record_for_child = inherit_record.clone();
        let (root, cx) = cx.add_window_view(move |_window, cx| {
            let child = cx.new(|_| CachedBuildReaderView {
                read_record: read_record_for_child,
                inherit_record: inherit_record_for_child,
            });
            CachedBuildProviderRoot {
                provider_value: 11,
                child,
            }
        });

        draw_window(cx);
        assert_eq!(*read_record.borrow(), Some(11));
        assert_eq!(*inherit_record.borrow(), Some(11));

        *read_record.borrow_mut() = None;
        *inherit_record.borrow_mut() = None;
        root.update(cx, |root, cx| {
            root.provider_value = 22;
            cx.notify();
        });
        draw_window(cx);

        assert_eq!(*read_record.borrow(), Some(22));
        assert_eq!(*inherit_record.borrow(), Some(22));
    }

    struct DeferredBuildRoot {
        record: Rc<RefCell<Vec<(usize, GlobalElementId)>>>,
    }

    impl Render for DeferredBuildRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            deferred(build_element(ProbeBuilder {
                label: 1,
                record: self.record.clone(),
            }))
        }
    }

    #[crate::test]
    fn deferred_draw_preserves_build_element_identity(cx: &mut TestAppContext) {
        let record = Rc::new(RefCell::new(Vec::new()));
        let record_for_root = record.clone();
        let (_root, cx) = cx.add_window_view(move |_window, _cx| DeferredBuildRoot {
            record: record_for_root,
        });

        record.borrow_mut().clear();
        draw_window(cx);

        let record = record.borrow();
        assert_eq!(record.len(), 1);
        let global_id = &record[0].1;
        assert!(
            global_id.iter().any(|segment| matches!(
                segment,
                ElementId::Name(name) if name.as_ref() == type_name::<ProbeBuilder>()
            )),
            "deferred build child should keep its build-type namespace"
        );
    }
}
