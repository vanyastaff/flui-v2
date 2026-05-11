//! Elements are the workhorses of GPUI. They are responsible for laying out and painting all of
//! the contents of a window. Elements form a tree and are laid out according to the web layout
//! standards as implemented by [taffy](https://github.com/DioxusLabs/taffy). Most of the time,
//! you won't need to interact with this module or these APIs directly. Elements provide their
//! own APIs and GPUI, or other element implementation, uses the APIs in this module to convert
//! that element tree into the pixels you see on the screen.
//!
//! # Element Basics
//!
//! Elements are constructed by calling [`Render::render()`] on the root view of the window,
//! which recursively constructs the element tree from the current state of the application,.
//! These elements are then laid out by Taffy, and painted to the screen according to their own
//! implementation of [`Element::paint()`]. Before the start of the next frame, the entire element
//! tree and any callbacks they have registered with GPUI are dropped and the process repeats.
//!
//! But some state is too simple and voluminous to store in every view that needs it, e.g.
//! whether a hover has been started or not. For this, GPUI provides the [`Element::PrepaintState`], associated type.
//!
//! # Implementing your own elements
//!
//! Elements are intended to be the low level, imperative API to GPUI. They are responsible for upholding,
//! or breaking, GPUI's features as they deem necessary. As an example, most GPUI elements are expected
//! to stay in the bounds that their parent element gives them. But with [`Window::with_content_mask`],
//! you can ignore this restriction and paint anywhere inside of the window's bounds. This is useful for overlays
//! and popups and anything else that shows up 'on top' of other elements.
//! With great power, comes great responsibility.
//!
//! However, most of the time, you won't need to implement your own elements. GPUI provides a number of
//! elements that should cover most common use cases out of the box and it's recommended that you use those
//! to construct `components`, using the [`RenderOnce`] trait and the `#[derive(IntoElement)]` macro. Only implement
//! elements when you need to take manual control of the layout and painting process, such as when using
//! your own custom layout algorithm or rendering a code editor.

use crate::{
    App, ArenaBox, AvailableSpace, Bounds, Context, DispatchNodeId, FocusHandle, InheritedValue,
    InspectorElementId, LayoutId, Pixels, Point, SharedString, Size, Style, Window,
    local_util::FluentBuilder, window::with_element_arena,
};
use derive_more::{Deref, DerefMut};
use std::{
    any::{Any, type_name},
    fmt::{self, Debug, Display},
    mem, panic,
    sync::Arc,
};

mod identity;

pub(crate) use identity::ElementIdStack;
pub use identity::{ElementId, GlobalKey, Key, LocalElementId, ValueKey};

/// Layout-phase access to the current element lifecycle.
///
/// This is the low-level engine context for [`Element::request_layout`]. It is
/// intentionally smaller than a Framework-tier build context: it only carries
/// element identity, inspector identity, and reborrowable access to the current
/// [`Window`] and [`App`].
pub struct LayoutCx<'a> {
    window: &'a mut Window,
    app: &'a mut App,
    global_id: Option<&'a GlobalElementId>,
    inspector_id: Option<&'a InspectorElementId>,
}

impl<'a> LayoutCx<'a> {
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

    /// Returns the globally unique id for this element, if it has one.
    pub fn global_id(&self) -> Option<&GlobalElementId> {
        self.global_id
    }

    /// Returns the inspector id for this element, if inspector metadata exists.
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

    /// Reborrow both window and app for existing APIs that need both handles.
    pub fn with_window_app<R>(&mut self, f: impl FnOnce(&mut Window, &mut App) -> R) -> R {
        f(&mut *self.window, &mut *self.app)
    }

    /// Reads the nearest inherited value of type `T` without subscribing the
    /// current element to future provider changes.
    pub fn read_inherited<T: InheritedValue>(&self) -> Option<T> {
        self.window.read_inherited::<T>()
    }

    /// Reads the nearest inherited value of type `T` and subscribes the
    /// current element's view to future provider changes.
    pub fn inherit<T: InheritedValue>(&mut self) -> Option<T> {
        debug_assert!(
            self.global_id.is_some(),
            "`inherit()` requires a stable element id; use `read_inherited()` for non-subscribing lookups",
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

    /// Run a nested layout operation with a different element id.
    pub fn with_global_id<R>(
        &mut self,
        global_id: Option<&GlobalElementId>,
        f: impl FnOnce(&mut LayoutCx<'_>) -> R,
    ) -> R {
        let mut cx = LayoutCx {
            window: &mut *self.window,
            app: &mut *self.app,
            global_id,
            inspector_id: self.inspector_id,
        };
        f(&mut cx)
    }
}

/// Prepaint-phase access to the current element lifecycle.
///
/// This context carries the element bounds computed from layout in addition to
/// the identity and runtime handles shared with [`LayoutCx`].
pub struct PrepaintCx<'a> {
    window: &'a mut Window,
    app: &'a mut App,
    global_id: Option<&'a GlobalElementId>,
    inspector_id: Option<&'a InspectorElementId>,
    bounds: Bounds<Pixels>,
}

impl<'a> PrepaintCx<'a> {
    pub(crate) fn new(
        window: &'a mut Window,
        app: &'a mut App,
        global_id: Option<&'a GlobalElementId>,
        inspector_id: Option<&'a InspectorElementId>,
        bounds: Bounds<Pixels>,
    ) -> Self {
        Self {
            window,
            app,
            global_id,
            inspector_id,
            bounds,
        }
    }

    /// Returns the globally unique id for this element, if it has one.
    pub fn global_id(&self) -> Option<&GlobalElementId> {
        self.global_id
    }

    /// Returns the inspector id for this element, if inspector metadata exists.
    pub fn inspector_id(&self) -> Option<&InspectorElementId> {
        self.inspector_id
    }

    /// Returns the bounds assigned to this element for the current frame.
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    /// Reborrow the current window.
    pub fn window(&mut self) -> &mut Window {
        &mut *self.window
    }

    /// Reborrow the current application state.
    pub fn app(&mut self) -> &mut App {
        &mut *self.app
    }

    /// Reborrow both window and app for existing APIs that need both handles.
    pub fn with_window_app<R>(&mut self, f: impl FnOnce(&mut Window, &mut App) -> R) -> R {
        f(&mut *self.window, &mut *self.app)
    }

    /// Reads the nearest inherited value of type `T` without subscribing the
    /// current element to future provider changes.
    pub fn read_inherited<T: InheritedValue>(&self) -> Option<T> {
        self.window.read_inherited::<T>()
    }

    /// Reads the nearest inherited value of type `T` and subscribes the
    /// current element's view to future provider changes.
    pub fn inherit<T: InheritedValue>(&mut self) -> Option<T> {
        debug_assert!(
            self.global_id.is_some(),
            "`inherit()` requires a stable element id; use `read_inherited()` for non-subscribing lookups",
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

    /// Run a nested prepaint operation with a different element id.
    pub fn with_global_id<R>(
        &mut self,
        global_id: Option<&GlobalElementId>,
        f: impl FnOnce(&mut PrepaintCx<'_>) -> R,
    ) -> R {
        let mut cx = PrepaintCx {
            window: &mut *self.window,
            app: &mut *self.app,
            global_id,
            inspector_id: self.inspector_id,
            bounds: self.bounds,
        };
        f(&mut cx)
    }

    /// Run a nested prepaint operation with different bounds.
    pub fn with_bounds<R>(
        &mut self,
        bounds: Bounds<Pixels>,
        f: impl FnOnce(&mut PrepaintCx<'_>) -> R,
    ) -> R {
        let mut cx = PrepaintCx {
            window: &mut *self.window,
            app: &mut *self.app,
            global_id: self.global_id,
            inspector_id: self.inspector_id,
            bounds,
        };
        f(&mut cx)
    }
}

/// Paint-phase access to the current element lifecycle.
///
/// This context carries the element bounds and runtime handles needed by
/// [`Element::paint`] without exposing paint plumbing as separate trait
/// parameters.
pub struct PaintCx<'a> {
    window: &'a mut Window,
    app: &'a mut App,
    global_id: Option<&'a GlobalElementId>,
    inspector_id: Option<&'a InspectorElementId>,
    bounds: Bounds<Pixels>,
}

impl<'a> PaintCx<'a> {
    pub(crate) fn new(
        window: &'a mut Window,
        app: &'a mut App,
        global_id: Option<&'a GlobalElementId>,
        inspector_id: Option<&'a InspectorElementId>,
        bounds: Bounds<Pixels>,
    ) -> Self {
        Self {
            window,
            app,
            global_id,
            inspector_id,
            bounds,
        }
    }

    /// Returns the globally unique id for this element, if it has one.
    pub fn global_id(&self) -> Option<&GlobalElementId> {
        self.global_id
    }

    /// Returns the inspector id for this element, if inspector metadata exists.
    pub fn inspector_id(&self) -> Option<&InspectorElementId> {
        self.inspector_id
    }

    /// Returns the bounds assigned to this element for the current frame.
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    /// Reborrow the current window.
    pub fn window(&mut self) -> &mut Window {
        &mut *self.window
    }

    /// Reborrow the current application state.
    pub fn app(&mut self) -> &mut App {
        &mut *self.app
    }

    /// Reborrow both window and app for existing APIs that need both handles.
    pub fn with_window_app<R>(&mut self, f: impl FnOnce(&mut Window, &mut App) -> R) -> R {
        f(&mut *self.window, &mut *self.app)
    }

    /// Reads the nearest inherited value of type `T` without subscribing the
    /// current element to future provider changes.
    pub fn read_inherited<T: InheritedValue>(&self) -> Option<T> {
        self.window.read_inherited::<T>()
    }

    /// Reads the nearest inherited value of type `T` and subscribes the
    /// current element's view to future provider changes.
    pub fn inherit<T: InheritedValue>(&mut self) -> Option<T> {
        debug_assert!(
            self.global_id.is_some(),
            "`inherit()` requires a stable element id; use `read_inherited()` for non-subscribing lookups",
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

    /// Run a nested paint operation with a different element id.
    pub fn with_global_id<R>(
        &mut self,
        global_id: Option<&GlobalElementId>,
        f: impl FnOnce(&mut PaintCx<'_>) -> R,
    ) -> R {
        let mut cx = PaintCx {
            window: &mut *self.window,
            app: &mut *self.app,
            global_id,
            inspector_id: self.inspector_id,
            bounds: self.bounds,
        };
        f(&mut cx)
    }

    /// Run a nested paint operation with different bounds.
    pub fn with_bounds<R>(
        &mut self,
        bounds: Bounds<Pixels>,
        f: impl FnOnce(&mut PaintCx<'_>) -> R,
    ) -> R {
        let mut cx = PaintCx {
            window: &mut *self.window,
            app: &mut *self.app,
            global_id: self.global_id,
            inspector_id: self.inspector_id,
            bounds,
        };
        f(&mut cx)
    }
}

/// Implemented by types that participate in laying out and painting the contents of a window.
/// Elements form a tree and are laid out according to web-based layout rules, as implemented by Taffy.
/// You can create custom elements by implementing this trait, see the module-level documentation
/// for more details.
pub trait Element: 'static + IntoElement {
    /// The type of state returned from [`Element::request_layout`]. A mutable reference to this state is subsequently
    /// provided to [`Element::prepaint`] and [`Element::paint`].
    type RequestLayoutState: 'static;

    /// The type of state returned from [`Element::prepaint`]. A mutable reference to this state is subsequently
    /// provided to [`Element::paint`].
    type PrepaintState: 'static;

    /// If this element has a unique identifier, return it here. This is used to track elements across frames, and
    /// will cause a GlobalElementId to be passed to the request_layout, prepaint, and paint methods.
    ///
    /// The global id can in turn be used to access state that's connected to an element with the same id across
    /// frames. This id must be unique among children of the first containing element with an id.
    ///
    /// `ElementId::CodeLocation` is accepted as a compatibility/local-key input and is normalized by the
    /// window identity stack into a parent-scoped Local segment. Reorder-sensitive children should use
    /// explicit value keys.
    fn id(&self) -> Option<ElementId>;

    /// Source location where this element was constructed, used to disambiguate elements in the
    /// inspector and navigate to their source code.
    fn source_location(&self) -> Option<&'static panic::Location<'static>>;

    /// Before an element can be painted, we need to know where it's going to be and how big it is.
    /// Use this method to request a layout from Taffy and initialize the element's state.
    fn request_layout(&mut self, cx: &mut LayoutCx<'_>) -> (LayoutId, Self::RequestLayoutState);

    /// After laying out an element, we need to commit its bounds to the current frame for hitbox
    /// purposes. The state argument is the same state that was returned from [`Element::request_layout()`].
    fn prepaint(
        &mut self,
        cx: &mut PrepaintCx<'_>,
        request_layout: &mut Self::RequestLayoutState,
    ) -> Self::PrepaintState;

    /// Once layout has been completed, this method will be called to paint the element to the screen.
    /// The state argument is the same state that was returned from [`Element::request_layout()`].
    fn paint(
        &mut self,
        cx: &mut PaintCx<'_>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
    );

    /// Convert this element into a dynamically-typed [`AnyElement`].
    fn into_any(self) -> AnyElement {
        AnyElement::new(self)
    }
}

/// Implemented by any type that can be converted into an element.
pub trait IntoElement: Sized {
    /// The specific type of element into which the implementing type is converted.
    /// Useful for converting other types into elements automatically, like Strings
    type Element: Element;

    /// Convert self into a type that implements [`Element`].
    ///
    /// The caller location is part of Local identity for `RenderOnce` component wrappers, so
    /// forwarding helpers should preserve `#[track_caller]`.
    #[track_caller]
    fn into_element(self) -> Self::Element;

    /// Convert self into a dynamically-typed [`AnyElement`].
    #[track_caller]
    fn into_any_element(self) -> AnyElement {
        self.into_element().into_any()
    }
}

impl<T: IntoElement> FluentBuilder for T {}

/// An object that can be drawn to the screen. This is the trait that distinguishes "views" from
/// other entities. Views are `Entity`'s which `impl Render` and drawn to the screen.
pub trait Render: 'static + Sized {
    /// Render this view into an element tree.
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement;
}

impl Render for Empty {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// You can derive [`IntoElement`] on any type that implements this trait.
/// It is used to construct reusable `components` out of plain data. Think of
/// components as a recipe for a certain pattern of elements. RenderOnce allows
/// you to invoke this pattern, without breaking the fluent builder pattern of
/// the element APIs.
pub trait RenderOnce: 'static {
    /// Render this component into an element tree. Note that this method
    /// takes ownership of self, as compared to [`Render::render()`] method
    /// which takes a mutable reference.
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement;
}

/// This is a helper trait to provide a uniform interface for constructing elements that
/// can accept any number of any kind of child elements
pub trait ParentElement {
    /// Extend this element's children with the given child elements.
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>);

    /// Add a single child element to this element.
    #[track_caller]
    fn child(mut self, child: impl IntoElement) -> Self
    where
        Self: Sized,
    {
        self.extend(std::iter::once(child.into_element().into_any()));
        self
    }

    /// Add multiple child elements to this element.
    #[track_caller]
    fn children(mut self, children: impl IntoIterator<Item = impl IntoElement>) -> Self
    where
        Self: Sized,
    {
        self.extend(children.into_iter().map(|child| child.into_any_element()));
        self
    }
}

/// An element for rendering components. An implementation detail of the [`IntoElement`] derive macro
/// for [`RenderOnce`].
///
/// `Component<C>` is an engine wrapper, not the Framework-tier `Widget` adapter. It participates in
/// engine Local identity via the callsite captured by [`Component::new`], and its children are still
/// namespaced by the component type name. Future Widget reconciliation remains SF01/SF02 work.
#[doc(hidden)]
pub struct Component<C: RenderOnce> {
    component: Option<C>,
    key: Option<ElementId>,
    source: &'static core::panic::Location<'static>,
}

impl<C: RenderOnce> Component<C> {
    /// Create a new component from the given RenderOnce type.
    #[track_caller]
    pub fn new(component: C) -> Self {
        Component {
            component: Some(component),
            key: None,
            source: core::panic::Location::caller(),
        }
    }

    /// Assign an explicit identity key to this component boundary.
    ///
    /// Use this for repeated or reordered `RenderOnce` components whose internal state/provider
    /// identity should follow application data.
    pub fn key(mut self, key: impl Into<ElementId>) -> Self {
        self.key = Some(key.into());
        self
    }
}

fn prepaint_component((element, name): &mut (AnyElement, &'static str), cx: &mut PrepaintCx<'_>) {
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

fn paint_component((element, name): &mut (AnyElement, &'static str), cx: &mut PaintCx<'_>) {
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
impl<C: RenderOnce> Element for Component<C> {
    type RequestLayoutState = (AnyElement, &'static str);
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(
            self.key
                .clone()
                .unwrap_or(ElementId::CodeLocation(*self.source)),
        )
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        Some(self.source)
    }

    fn request_layout(&mut self, cx: &mut LayoutCx<'_>) -> (LayoutId, Self::RequestLayoutState) {
        cx.with_window_app(|window, cx| {
            window.with_id(ElementId::Name(type_name::<C>().into()), |window| {
                let mut element = self
                    .component
                    .take()
                    .unwrap()
                    .render(window, cx)
                    .into_any_element();

                let mut child_cx = LayoutCx::new(window, cx, None, None);
                let layout_id = element.request_layout(&mut child_cx);
                (layout_id, (element, type_name::<C>()))
            })
        })
    }

    fn prepaint(&mut self, cx: &mut PrepaintCx<'_>, state: &mut Self::RequestLayoutState) {
        prepaint_component(state, cx);
    }

    fn paint(
        &mut self,
        cx: &mut PaintCx<'_>,
        state: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
    ) {
        paint_component(state, cx);
    }
}

impl<C: RenderOnce> IntoElement for Component<C> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// A globally unique identifier for an element, used to track state across frames.
#[derive(Deref, DerefMut, Clone, Default, Debug, Eq, PartialEq, Hash)]
pub struct GlobalElementId(pub(crate) Arc<[ElementId]>);

impl Display for GlobalElementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, element_id) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ".")?;
            }
            write!(f, "{}", element_id)?;
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
pub(crate) struct ElementLifecycleMetadata {
    pub(crate) global_id: Option<GlobalElementId>,
    pub(crate) inspector_id: Option<InspectorElementId>,
    pub(crate) bounds: Bounds<Pixels>,
}

trait ElementObject {
    fn inner_element(&mut self) -> &mut dyn Any;

    fn request_layout(&mut self, cx: &mut LayoutCx<'_>) -> LayoutId;

    fn prepaint(&mut self, cx: &mut PrepaintCx<'_>);

    fn paint(&mut self, cx: &mut PaintCx<'_>);

    fn layout_as_root(
        &mut self,
        available_space: Size<AvailableSpace>,
        cx: &mut LayoutCx<'_>,
    ) -> Size<Pixels>;

    fn lifecycle_metadata(&self, window: &mut Window) -> ElementLifecycleMetadata;
}

/// A wrapper around an implementer of [`Element`] that allows it to be drawn in a window.
pub struct Drawable<E: Element> {
    /// The drawn element.
    pub element: E,
    phase: ElementDrawPhase<E::RequestLayoutState, E::PrepaintState>,
}

#[derive(Default)]
enum ElementDrawPhase<RequestLayoutState, PrepaintState> {
    #[default]
    Start,
    RequestLayout {
        layout_id: LayoutId,
        global_id: Option<GlobalElementId>,
        inspector_id: Option<InspectorElementId>,
        request_layout: RequestLayoutState,
    },
    LayoutComputed {
        layout_id: LayoutId,
        global_id: Option<GlobalElementId>,
        inspector_id: Option<InspectorElementId>,
        available_space: Size<AvailableSpace>,
        request_layout: RequestLayoutState,
    },
    Prepaint {
        node_id: DispatchNodeId,
        global_id: Option<GlobalElementId>,
        inspector_id: Option<InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: RequestLayoutState,
        prepaint: PrepaintState,
    },
    Painted,
}

/// A wrapper around an implementer of [`Element`] that allows it to be drawn in a window.
impl<E: Element> Drawable<E> {
    pub(crate) fn new(element: E) -> Self {
        Drawable {
            element,
            phase: ElementDrawPhase::Start,
        }
    }

    fn request_layout(&mut self, cx: &mut LayoutCx<'_>) -> LayoutId {
        match mem::take(&mut self.phase) {
            ElementDrawPhase::Start => cx.with_window_app(|window, app| {
                let element_id_scope = self
                    .element
                    .id()
                    .map(|element_id| window.push_element_id(element_id));
                let global_id = element_id_scope
                    .as_ref()
                    .map(|scope| scope.global_id().clone());

                let inspector_id;
                #[cfg(any(feature = "inspector", debug_assertions))]
                {
                    inspector_id = self.element.source_location().map(|source| {
                        let path = crate::InspectorElementPath {
                            global_id: GlobalElementId(Arc::from(&*window.element_id_stack)),
                            source_location: source,
                        };
                        window.build_inspector_element_id(path)
                    });
                }
                #[cfg(not(any(feature = "inspector", debug_assertions)))]
                {
                    inspector_id = None;
                }

                let mut element_cx =
                    LayoutCx::new(window, app, global_id.as_ref(), inspector_id.as_ref());
                let (layout_id, request_layout) = self.element.request_layout(&mut element_cx);

                self.phase = ElementDrawPhase::RequestLayout {
                    layout_id,
                    global_id,
                    inspector_id,
                    request_layout,
                };
                layout_id
            }),
            _ => panic!("must call request_layout only once"),
        }
    }

    pub(crate) fn prepaint(&mut self, cx: &mut PrepaintCx<'_>) {
        match mem::take(&mut self.phase) {
            ElementDrawPhase::RequestLayout {
                layout_id,
                global_id,
                inspector_id,
                mut request_layout,
            }
            | ElementDrawPhase::LayoutComputed {
                layout_id,
                global_id,
                inspector_id,
                mut request_layout,
                ..
            } => cx.with_window_app(|window, app| {
                let _element_id_scope = if let Some(global_id) = global_id.as_ref() {
                    let scope = window.push_resolved_element_id(global_id);
                    debug_assert_eq!(&*global_id.0, &*window.element_id_stack);
                    Some(scope)
                } else {
                    None
                };

                let bounds = window.layout_bounds(layout_id);
                let node_id = window.next_frame.dispatch_tree.push_node();
                let mut element_cx = PrepaintCx::new(
                    window,
                    app,
                    global_id.as_ref(),
                    inspector_id.as_ref(),
                    bounds,
                );
                let prepaint = self.element.prepaint(&mut element_cx, &mut request_layout);
                window.next_frame.dispatch_tree.pop_node();

                self.phase = ElementDrawPhase::Prepaint {
                    node_id,
                    global_id,
                    inspector_id,
                    bounds,
                    request_layout,
                    prepaint,
                };
            }),
            _ => panic!("must call request_layout before prepaint"),
        }
    }

    pub(crate) fn paint(
        &mut self,
        cx: &mut PaintCx<'_>,
    ) -> (E::RequestLayoutState, E::PrepaintState) {
        match mem::take(&mut self.phase) {
            ElementDrawPhase::Prepaint {
                node_id,
                global_id,
                inspector_id,
                bounds,
                mut request_layout,
                mut prepaint,
                ..
            } => cx.with_window_app(|window, app| {
                let _element_id_scope = if let Some(global_id) = global_id.as_ref() {
                    let scope = window.push_resolved_element_id(global_id);
                    debug_assert_eq!(&*global_id.0, &*window.element_id_stack);
                    Some(scope)
                } else {
                    None
                };

                window.next_frame.dispatch_tree.set_active_node(node_id);
                let mut element_cx = PaintCx::new(
                    window,
                    app,
                    global_id.as_ref(),
                    inspector_id.as_ref(),
                    bounds,
                );
                self.element
                    .paint(&mut element_cx, &mut request_layout, &mut prepaint);

                self.phase = ElementDrawPhase::Painted;
                (request_layout, prepaint)
            }),
            _ => panic!("must call prepaint before paint"),
        }
    }

    pub(crate) fn layout_as_root(
        &mut self,
        available_space: Size<AvailableSpace>,
        cx: &mut LayoutCx<'_>,
    ) -> Size<Pixels> {
        if matches!(&self.phase, ElementDrawPhase::Start) {
            self.request_layout(cx);
        }

        cx.with_window_app(|window, app| {
            let layout_id = match mem::take(&mut self.phase) {
                ElementDrawPhase::RequestLayout {
                    layout_id,
                    global_id,
                    inspector_id,
                    request_layout,
                } => {
                    window.compute_layout(layout_id, available_space, app);
                    self.phase = ElementDrawPhase::LayoutComputed {
                        layout_id,
                        global_id,
                        inspector_id,
                        available_space,
                        request_layout,
                    };
                    layout_id
                }
                ElementDrawPhase::LayoutComputed {
                    layout_id,
                    global_id,
                    inspector_id,
                    available_space: prev_available_space,
                    request_layout,
                } => {
                    if available_space != prev_available_space {
                        window.compute_layout(layout_id, available_space, app);
                    }
                    self.phase = ElementDrawPhase::LayoutComputed {
                        layout_id,
                        global_id,
                        inspector_id,
                        available_space,
                        request_layout,
                    };
                    layout_id
                }
                _ => panic!("cannot measure after painting"),
            };

            window.layout_bounds(layout_id).size
        })
    }
}

impl<E> ElementObject for Drawable<E>
where
    E: Element,
    E::RequestLayoutState: 'static,
{
    fn inner_element(&mut self) -> &mut dyn Any {
        &mut self.element
    }

    #[inline]
    fn request_layout(&mut self, cx: &mut LayoutCx<'_>) -> LayoutId {
        Drawable::request_layout(self, cx)
    }

    #[inline]
    fn prepaint(&mut self, cx: &mut PrepaintCx<'_>) {
        Drawable::prepaint(self, cx);
    }

    #[inline]
    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        Drawable::paint(self, cx);
    }

    #[inline]
    fn layout_as_root(
        &mut self,
        available_space: Size<AvailableSpace>,
        cx: &mut LayoutCx<'_>,
    ) -> Size<Pixels> {
        Drawable::layout_as_root(self, available_space, cx)
    }

    fn lifecycle_metadata(&self, window: &mut Window) -> ElementLifecycleMetadata {
        match &self.phase {
            ElementDrawPhase::RequestLayout {
                layout_id,
                global_id,
                inspector_id,
                ..
            }
            | ElementDrawPhase::LayoutComputed {
                layout_id,
                global_id,
                inspector_id,
                ..
            } => ElementLifecycleMetadata {
                global_id: global_id.clone(),
                inspector_id: inspector_id.clone(),
                bounds: window.layout_bounds(*layout_id),
            },
            ElementDrawPhase::Prepaint {
                global_id,
                inspector_id,
                bounds,
                ..
            } => ElementLifecycleMetadata {
                global_id: global_id.clone(),
                inspector_id: inspector_id.clone(),
                bounds: *bounds,
            },
            ElementDrawPhase::Start | ElementDrawPhase::Painted => {
                ElementLifecycleMetadata::default()
            }
        }
    }
}

/// A dynamically typed element that can be used to store any element type.
pub struct AnyElement(ArenaBox<dyn ElementObject>);

impl AnyElement {
    pub(crate) fn new<E>(element: E) -> Self
    where
        E: 'static + Element,
        E::RequestLayoutState: Any,
    {
        let element = with_element_arena(|arena| arena.alloc(|| Drawable::new(element)))
            .map(|element| element as &mut dyn ElementObject);
        AnyElement(element)
    }

    /// Attempt to downcast a reference to the boxed element to a specific type.
    pub fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.0.inner_element().downcast_mut::<T>()
    }

    /// Request the layout ID of the element stored in this `AnyElement`.
    /// Used for laying out child elements in a parent element.
    pub fn request_layout(&mut self, cx: &mut LayoutCx<'_>) -> LayoutId {
        self.0.request_layout(cx)
    }

    /// Prepares the element to be painted by storing its bounds, giving it a chance to draw hitboxes and
    /// request autoscroll before the final paint pass is confirmed.
    pub fn prepaint(&mut self, cx: &mut PrepaintCx<'_>) -> Option<FocusHandle> {
        let focus_assigned = cx.with_window_app(|window, _cx| window.next_frame.focus.is_some());

        self.0.prepaint(cx);

        if !focus_assigned
            && let Some(focus_handle) = cx.with_window_app(|window, cx| {
                window
                    .next_frame
                    .focus
                    .and_then(|id| FocusHandle::for_id(id, &cx.focus_handles))
            })
        {
            return Some(focus_handle);
        }

        None
    }

    /// Paints the element stored in this `AnyElement`.
    pub fn paint(&mut self, cx: &mut PaintCx<'_>) {
        self.0.paint(cx);
    }

    pub(crate) fn lifecycle_metadata(&self, window: &mut Window) -> ElementLifecycleMetadata {
        self.0.lifecycle_metadata(window)
    }

    pub(crate) fn paint_with_window(&mut self, window: &mut Window, cx: &mut App) {
        window.element_id_stack.begin_pass();
        let mut cx = PaintCx::new(window, cx, None, None, Bounds::default());
        self.paint(&mut cx);
    }

    /// Performs layout for this element within the given available space and returns its size.
    pub fn layout_as_root(
        &mut self,
        available_space: Size<AvailableSpace>,
        cx: &mut LayoutCx<'_>,
    ) -> Size<Pixels> {
        self.0.layout_as_root(available_space, cx)
    }

    pub(crate) fn layout_as_root_with_window(
        &mut self,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
    ) -> Size<Pixels> {
        let mut cx = LayoutCx::new(window, cx, None, None);
        let element_id_stack = cx.window.element_id_stack.clone();
        let size = self.layout_as_root(available_space, &mut cx);
        cx.window.element_id_stack.clone_from(&element_id_stack);
        size
    }

    /// Prepaints this element at the given absolute origin.
    /// If any element in the subtree beneath this element is focused, its FocusHandle is returned.
    pub fn prepaint_at(
        &mut self,
        origin: Point<Pixels>,
        cx: &mut PrepaintCx<'_>,
    ) -> Option<FocusHandle> {
        let global_id = cx.global_id().cloned();
        let inspector_id = cx.inspector_id().cloned();
        let bounds = cx.bounds();
        cx.with_window_app(|window, app| {
            window.with_absolute_element_offset(origin, |window| {
                let mut cx = PrepaintCx::new(
                    window,
                    app,
                    global_id.as_ref(),
                    inspector_id.as_ref(),
                    bounds,
                );
                self.prepaint(&mut cx)
            })
        })
    }

    pub(crate) fn prepaint_at_with_window(
        &mut self,
        origin: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<FocusHandle> {
        let mut cx = PrepaintCx::new(window, cx, None, None, Bounds::default());
        self.prepaint_at(origin, &mut cx)
    }

    /// Performs layout on this element in the available space, then prepaints it at the given absolute origin.
    /// If any element in the subtree beneath this element is focused, its FocusHandle is returned.
    pub fn prepaint_as_root(
        &mut self,
        origin: Point<Pixels>,
        available_space: Size<AvailableSpace>,
        layout_cx: &mut LayoutCx<'_>,
        prepaint_cx: &mut PrepaintCx<'_>,
    ) -> Option<FocusHandle> {
        self.layout_as_root(available_space, layout_cx);
        self.prepaint_at(origin, prepaint_cx)
    }

    pub(crate) fn prepaint_as_root_with_window(
        &mut self,
        origin: Point<Pixels>,
        available_space: Size<AvailableSpace>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<FocusHandle> {
        window.element_id_stack.begin_pass();
        let mut layout_cx = LayoutCx::new(window, cx, None, None);
        self.layout_as_root(available_space, &mut layout_cx);
        window.element_id_stack.begin_pass();
        let mut prepaint_cx = PrepaintCx::new(window, cx, None, None, Bounds::default());
        self.prepaint_at(origin, &mut prepaint_cx)
    }
}

impl Element for AnyElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, cx: &mut LayoutCx<'_>) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = self.request_layout(cx);
        (layout_id, ())
    }

    fn prepaint(&mut self, cx: &mut PrepaintCx<'_>, _: &mut Self::RequestLayoutState) {
        self.prepaint(cx);
    }

    fn paint(
        &mut self,
        cx: &mut PaintCx<'_>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
    ) {
        self.paint(cx);
    }
}

impl IntoElement for AnyElement {
    type Element = Self;

    #[track_caller]
    fn into_element(self) -> Self::Element {
        self
    }

    #[track_caller]
    fn into_any_element(self) -> AnyElement {
        self
    }
}

/// The empty element, which renders nothing.
pub struct Empty;

impl IntoElement for Empty {
    type Element = Self;

    #[track_caller]
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Empty {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, cx: &mut LayoutCx<'_>) -> (LayoutId, Self::RequestLayoutState) {
        (
            cx.with_window_app(|window, cx| {
                window.request_layout(
                    Style {
                        display: crate::Display::None,
                        ..Default::default()
                    },
                    None,
                    cx,
                )
            }),
            (),
        )
    }

    fn prepaint(&mut self, _cx: &mut PrepaintCx<'_>, _state: &mut Self::RequestLayoutState) {}

    fn paint(
        &mut self,
        _cx: &mut PaintCx<'_>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DrawPhase, ElementArenaScope, EntityId, ParentElement, Render, TestAppContext, div, point,
        px, size,
    };
    use smallvec::smallvec;
    use std::{cell::RefCell, rc::Rc};

    #[derive(Default)]
    struct ProbeRecord {
        saw_layout_global_id: bool,
        saw_layout_inspector_id: bool,
        saw_prepaint_global_id: bool,
        saw_prepaint_inspector_id: bool,
        saw_paint_global_id: bool,
        saw_paint_inspector_id: bool,
        prepaint_bounds: Option<Bounds<Pixels>>,
        paint_bounds: Option<Bounds<Pixels>>,
        painted: bool,
    }

    #[derive(Clone)]
    struct ProbeElement {
        record: Rc<RefCell<ProbeRecord>>,
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
            Some("probe".into())
        }

        fn source_location(&self) -> Option<&'static panic::Location<'static>> {
            Some(panic::Location::caller())
        }

        fn request_layout(
            &mut self,
            cx: &mut LayoutCx<'_>,
        ) -> (LayoutId, Self::RequestLayoutState) {
            let mut record = self.record.borrow_mut();
            record.saw_layout_global_id = cx.global_id().is_some();
            record.saw_layout_inspector_id = cx.inspector_id().is_some();
            drop(record);

            cx.with_window_app(|window, cx| {
                let mut style = Style::default();
                style.size = size(px(123.).into(), px(45.).into());
                (window.request_layout(style, [], cx), ())
            })
        }

        fn prepaint(
            &mut self,
            cx: &mut PrepaintCx<'_>,
            _request_layout: &mut Self::RequestLayoutState,
        ) -> Self::PrepaintState {
            let mut record = self.record.borrow_mut();
            record.saw_prepaint_global_id = cx.global_id().is_some();
            record.saw_prepaint_inspector_id = cx.inspector_id().is_some();
            record.prepaint_bounds = Some(cx.bounds());
        }

        fn paint(
            &mut self,
            cx: &mut PaintCx<'_>,
            _request_layout: &mut Self::RequestLayoutState,
            _prepaint: &mut Self::PrepaintState,
        ) {
            let mut record = self.record.borrow_mut();
            record.saw_paint_global_id = cx.global_id().is_some();
            record.saw_paint_inspector_id = cx.inspector_id().is_some();
            record.paint_bounds = Some(cx.bounds());
            record.painted = true;
        }
    }

    #[crate::test]
    fn lifecycle_contexts_expose_identity_bounds_and_runtime(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let record = Rc::new(RefCell::new(ProbeRecord::default()));
        let element_record = record.clone();

        cx.draw(
            point(px(0.), px(0.)),
            size(
                AvailableSpace::Definite(px(123.)),
                AvailableSpace::Definite(px(45.)),
            ),
            move |_, _| ProbeElement {
                record: element_record,
            },
        );

        let record = record.borrow();
        assert!(record.saw_layout_global_id);
        assert!(record.saw_prepaint_global_id);
        assert!(record.saw_paint_global_id);
        #[cfg(any(feature = "inspector", debug_assertions))]
        {
            assert!(record.saw_layout_inspector_id);
            assert!(record.saw_prepaint_inspector_id);
            assert!(record.saw_paint_inspector_id);
        }
        assert_eq!(
            record.prepaint_bounds.unwrap().size,
            size(px(123.), px(45.))
        );
        assert_eq!(record.paint_bounds.unwrap().size, size(px(123.), px(45.)));
        assert!(record.painted);
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum IdentityPanicPhase {
        Layout,
        Prepaint,
        Paint,
    }

    struct IdentityPanicElement {
        phase: IdentityPanicPhase,
    }

    impl IntoElement for IdentityPanicElement {
        type Element = Self;

        fn into_element(self) -> Self::Element {
            self
        }
    }

    impl Element for IdentityPanicElement {
        type RequestLayoutState = ();
        type PrepaintState = ();

        fn id(&self) -> Option<ElementId> {
            Some("identity-panic-probe".into())
        }

        fn source_location(&self) -> Option<&'static panic::Location<'static>> {
            None
        }

        fn request_layout(
            &mut self,
            cx: &mut LayoutCx<'_>,
        ) -> (LayoutId, Self::RequestLayoutState) {
            if self.phase == IdentityPanicPhase::Layout {
                panic!("identity layout panic");
            }

            cx.with_window_app(|window, cx| {
                let mut style = Style::default();
                style.size = size(px(1.).into(), px(1.).into());
                (window.request_layout(style, [], cx), ())
            })
        }

        fn prepaint(
            &mut self,
            _cx: &mut PrepaintCx<'_>,
            _request_layout: &mut Self::RequestLayoutState,
        ) -> Self::PrepaintState {
            if self.phase == IdentityPanicPhase::Prepaint {
                panic!("identity prepaint panic");
            }
        }

        fn paint(
            &mut self,
            _cx: &mut PaintCx<'_>,
            _request_layout: &mut Self::RequestLayoutState,
            _prepaint: &mut Self::PrepaintState,
        ) {
            if self.phase == IdentityPanicPhase::Paint {
                panic!("identity paint panic");
            }
        }
    }

    fn assert_identity_stack_restored_after_drawable_panic(
        cx: &mut TestAppContext,
        phase: IdentityPanicPhase,
    ) {
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let _arena_scope = ElementArenaScope::enter(&cx.element_arena);
            let mut element = Drawable::new(IdentityPanicElement { phase });

            window.invalidator.set_phase(DrawPhase::Prepaint);
            let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                window.element_id_stack.begin_pass();
                {
                    let mut layout_cx = LayoutCx::new(window, cx, None, None);
                    element.layout_as_root(
                        size(
                            AvailableSpace::Definite(px(10.)),
                            AvailableSpace::Definite(px(10.)),
                        ),
                        &mut layout_cx,
                    );
                }

                window.element_id_stack.begin_pass();
                {
                    let mut prepaint_cx =
                        PrepaintCx::new(window, cx, None, None, Bounds::default());
                    element.prepaint(&mut prepaint_cx);
                }

                window.invalidator.set_phase(DrawPhase::Paint);
                window.element_id_stack.begin_pass();
                {
                    let mut paint_cx = PaintCx::new(window, cx, None, None, Bounds::default());
                    let _ = element.paint(&mut paint_cx);
                }
            }));
            window.invalidator.set_phase(DrawPhase::None);

            assert!(result.is_err());
            assert_eq!(window.element_id_stack.len(), 0);

            drop(element);
            cx.element_arena.borrow_mut().clear();
        });
    }

    #[crate::test]
    fn drawable_identity_scope_is_restored_after_layout_panic(cx: &mut TestAppContext) {
        assert_identity_stack_restored_after_drawable_panic(cx, IdentityPanicPhase::Layout);
    }

    #[crate::test]
    fn drawable_identity_scope_is_restored_after_prepaint_panic(cx: &mut TestAppContext) {
        assert_identity_stack_restored_after_drawable_panic(cx, IdentityPanicPhase::Prepaint);
    }

    #[crate::test]
    fn drawable_identity_scope_is_restored_after_paint_panic(cx: &mut TestAppContext) {
        assert_identity_stack_restored_after_drawable_panic(cx, IdentityPanicPhase::Paint);
    }

    #[crate::test]
    fn window_identity_scope_helpers_restore_after_panic(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        cx.update(|window, _| {
            let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                window.with_id("panic-id", |_| panic!("with_id panic"));
            }));
            assert!(result.is_err());
            assert_eq!(window.element_id_stack.len(), 0);

            let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                window.with_element_namespace("panic-namespace", |_| {
                    panic!("with_element_namespace panic")
                });
            }));
            assert!(result.is_err());
            assert_eq!(window.element_id_stack.len(), 0);
        });
    }

    #[crate::test]
    fn any_element_lifecycle_metadata_reflects_layout_phase(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let _arena_scope = ElementArenaScope::enter(&cx.element_arena);
            let mut element = ProbeElement {
                record: Rc::new(RefCell::new(ProbeRecord::default())),
            }
            .into_any_element();

            window.invalidator.set_phase(DrawPhase::Prepaint);
            element.layout_as_root_with_window(
                size(
                    AvailableSpace::Definite(px(123.)),
                    AvailableSpace::Definite(px(45.)),
                ),
                window,
                cx,
            );
            let metadata = element.lifecycle_metadata(window);
            window.invalidator.set_phase(DrawPhase::None);

            assert!(metadata.global_id.is_some());
            #[cfg(any(feature = "inspector", debug_assertions))]
            assert!(metadata.inspector_id.is_some());
            assert_eq!(metadata.bounds.size, size(px(123.), px(45.)));

            drop(element);
            cx.element_arena.borrow_mut().clear();
        });
    }

    struct FocusElement {
        focus_handle: FocusHandle,
    }

    impl IntoElement for FocusElement {
        type Element = Self;

        fn into_element(self) -> Self::Element {
            self
        }
    }

    impl Element for FocusElement {
        type RequestLayoutState = ();
        type PrepaintState = ();

        fn id(&self) -> Option<ElementId> {
            Some("focus-probe".into())
        }

        fn source_location(&self) -> Option<&'static panic::Location<'static>> {
            None
        }

        fn request_layout(
            &mut self,
            cx: &mut LayoutCx<'_>,
        ) -> (LayoutId, Self::RequestLayoutState) {
            cx.with_window_app(|window, cx| {
                let mut style = Style::default();
                style.size = size(px(10.).into(), px(10.).into());
                (window.request_layout(style, [], cx), ())
            })
        }

        fn prepaint(
            &mut self,
            cx: &mut PrepaintCx<'_>,
            _request_layout: &mut Self::RequestLayoutState,
        ) -> Self::PrepaintState {
            cx.with_window_app(|window, cx| {
                window.set_focus_handle(&self.focus_handle, cx);
            });
        }

        fn paint(
            &mut self,
            _cx: &mut PaintCx<'_>,
            _request_layout: &mut Self::RequestLayoutState,
            _prepaint: &mut Self::PrepaintState,
        ) {
        }
    }

    #[crate::test]
    fn any_element_prepaint_returns_newly_assigned_focus_handle(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let _arena_scope = ElementArenaScope::enter(&cx.element_arena);
            let expected_focus = cx.focus_handle();
            let mut element = FocusElement {
                focus_handle: expected_focus.clone(),
            }
            .into_any_element();

            expected_focus.focus(window, cx);
            window.invalidator.set_phase(DrawPhase::Prepaint);
            element.layout_as_root_with_window(
                size(
                    AvailableSpace::Definite(px(10.)),
                    AvailableSpace::Definite(px(10.)),
                ),
                window,
                cx,
            );
            let assigned = element.prepaint_at_with_window(point(px(0.), px(0.)), window, cx);
            window.invalidator.set_phase(DrawPhase::None);

            assert_eq!(assigned.map(|handle| handle.id), Some(expected_focus.id));
            drop(element);
            cx.element_arena.borrow_mut().clear();
        });
    }

    #[derive(Clone, Copy)]
    struct EmptyComponent;

    impl RenderOnce for EmptyComponent {
        fn render(self, _window: &mut Window, _cx: &mut crate::App) -> impl IntoElement {
            Empty
        }
    }

    #[crate::test]
    fn repeated_components_from_same_callsite_get_local_identity(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();

        cx.draw(
            point(px(0.), px(0.)),
            size(
                AvailableSpace::Definite(px(10.)),
                AvailableSpace::Definite(px(10.)),
            ),
            move |_, _| {
                let children = (0..2).map(|_| Component::new(EmptyComponent).into_any_element());
                div().children(children)
            },
        );
    }

    #[crate::test]
    fn dynamic_prepaint_order_reuses_layout_identity(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();

        cx.draw(
            point(px(0.), px(0.)),
            size(
                AvailableSpace::Definite(px(10.)),
                AvailableSpace::Definite(px(10.)),
            ),
            move |_, _| {
                let children = (0..2).map(|_| Component::new(EmptyComponent).into_any_element());
                div()
                    .with_dynamic_prepaint_order(|_, _| smallvec![1, 0])
                    .children(children)
            },
        );
    }

    #[derive(Clone)]
    struct CallsiteComponent {
        label: usize,
        record: Rc<RefCell<Vec<(usize, GlobalElementId)>>>,
    }

    impl IntoElement for CallsiteComponent {
        type Element = Component<Self>;

        #[track_caller]
        fn into_element(self) -> Self::Element {
            Component::new(self)
        }
    }

    impl RenderOnce for CallsiteComponent {
        fn render(self, _window: &mut Window, _cx: &mut crate::App) -> impl IntoElement {
            CallsiteProbe {
                label: self.label,
                record: self.record,
            }
        }
    }

    struct CallsiteProbe {
        label: usize,
        record: Rc<RefCell<Vec<(usize, GlobalElementId)>>>,
    }

    impl IntoElement for CallsiteProbe {
        type Element = Self;

        fn into_element(self) -> Self::Element {
            self
        }
    }

    impl Element for CallsiteProbe {
        type RequestLayoutState = ();
        type PrepaintState = ();

        fn id(&self) -> Option<ElementId> {
            Some(ElementId::Name(SharedString::new_static("callsite-probe")))
        }

        fn source_location(&self) -> Option<&'static panic::Location<'static>> {
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
                .expect("component child should inherit component global id");
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

    fn first_local_source_line(global_id: &GlobalElementId) -> u32 {
        global_id
            .iter()
            .find_map(|segment| match segment {
                ElementId::Local(local) => Some(local.source_location().line()),
                _ => None,
            })
            .expect("component global id should contain a local source segment")
    }

    #[crate::test]
    fn parent_child_preserves_component_callsite_identity(cx: &mut TestAppContext) {
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
                    .child(CallsiteComponent {
                        label: 1,
                        record: record_for_draw.clone(),
                    })
                    .child(CallsiteComponent {
                        label: 2,
                        record: record_for_draw,
                    })
            },
        );

        let record = record.borrow();
        let first = record
            .iter()
            .find_map(|(label, global_id)| (*label == 1).then_some(global_id))
            .expect("first component should record its global id");
        let second = record
            .iter()
            .find_map(|(label, global_id)| (*label == 2).then_some(global_id))
            .expect("second component should record its global id");

        assert_ne!(
            first_local_source_line(first),
            first_local_source_line(second),
            "ParentElement::child must preserve the user's component callsite"
        );
    }

    #[derive(Clone)]
    struct StateProbe {
        label: usize,
        keyed: bool,
        record: Rc<RefCell<Vec<(usize, EntityId)>>>,
    }

    struct StateProbeState;

    impl IntoElement for StateProbe {
        type Element = Self;

        fn into_element(self) -> Self::Element {
            self
        }
    }

    impl Element for StateProbe {
        type RequestLayoutState = ();
        type PrepaintState = ();

        fn id(&self) -> Option<ElementId> {
            None
        }

        fn source_location(&self) -> Option<&'static panic::Location<'static>> {
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
            let label = self.label;
            let state = cx.with_window_app(|window, cx| {
                if self.keyed {
                    window.use_keyed_state(("state-probe", label), cx, |_, _| StateProbeState)
                } else {
                    window.use_state(cx, |_, _| StateProbeState)
                }
            });
            self.record.borrow_mut().push((label, state.entity_id()));
        }

        fn paint(
            &mut self,
            _cx: &mut PaintCx<'_>,
            _request_layout: &mut Self::RequestLayoutState,
            _prepaint: &mut Self::PrepaintState,
        ) {
        }
    }

    struct StateProbeRoot {
        keyed: bool,
        reversed: bool,
        record: Rc<RefCell<Vec<(usize, EntityId)>>>,
    }

    impl Render for StateProbeRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let order = if self.reversed { [2, 1] } else { [1, 2] };
            let children = order.into_iter().map(|label| {
                StateProbe {
                    label,
                    keyed: self.keyed,
                    record: self.record.clone(),
                }
                .into_any_element()
            });

            div().children(children)
        }
    }

    struct StateProbeComponent {
        label: usize,
        record: Rc<RefCell<Vec<(usize, EntityId)>>>,
    }

    impl RenderOnce for StateProbeComponent {
        fn render(self, _window: &mut Window, _cx: &mut crate::App) -> impl IntoElement {
            StateProbe {
                label: self.label,
                keyed: false,
                record: self.record,
            }
        }
    }

    struct ComponentStateProbeRoot {
        reversed: bool,
        record: Rc<RefCell<Vec<(usize, EntityId)>>>,
    }

    impl Render for ComponentStateProbeRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let order = if self.reversed { [2, 1] } else { [1, 2] };
            let children = order.into_iter().map(|label| {
                Component::new(StateProbeComponent {
                    label,
                    record: self.record.clone(),
                })
                .key(("state-probe-component", label))
                .into_any_element()
            });

            div().children(children)
        }
    }

    fn draw_window(cx: &mut crate::VisualTestContext) {
        cx.update(|window, cx| {
            window.draw(cx).clear();
        });
    }

    fn recorded_id(record: &Rc<RefCell<Vec<(usize, EntityId)>>>, label: usize) -> EntityId {
        record
            .borrow()
            .iter()
            .find_map(|(recorded_label, entity_id)| {
                (*recorded_label == label).then_some(*entity_id)
            })
            .expect("state probe label should have been recorded")
    }

    #[crate::test]
    fn local_use_state_occurrences_are_stable_for_same_order(cx: &mut TestAppContext) {
        let record = Rc::new(RefCell::new(Vec::new()));
        let record_for_root = record.clone();
        let (_root, cx) = cx.add_window_view(move |_window, _cx| StateProbeRoot {
            keyed: false,
            reversed: false,
            record: record_for_root,
        });

        draw_window(cx);
        let first_1 = recorded_id(&record, 1);
        let first_2 = recorded_id(&record, 2);

        record.borrow_mut().clear();
        draw_window(cx);
        assert_eq!(first_1, recorded_id(&record, 1));
        assert_eq!(first_2, recorded_id(&record, 2));
    }

    #[crate::test]
    fn keyed_state_follows_value_keys_across_reorder(cx: &mut TestAppContext) {
        let record = Rc::new(RefCell::new(Vec::new()));
        let record_for_root = record.clone();
        let (root, cx) = cx.add_window_view(move |_window, _cx| StateProbeRoot {
            keyed: true,
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

    #[crate::test]
    fn keyed_component_boundary_preserves_inner_state_across_reorder(cx: &mut TestAppContext) {
        let record = Rc::new(RefCell::new(Vec::new()));
        let record_for_root = record.clone();
        let (root, cx) = cx.add_window_view(move |_window, _cx| ComponentStateProbeRoot {
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
}
