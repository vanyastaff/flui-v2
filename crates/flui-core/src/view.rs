use crate::provider::registry::{InheritedDependency, ProviderScopeKey};
use crate::{
    AnyElement, AnyEntity, AnyWeakEntity, App, Bounds, ContentMask, Context, Element, ElementId,
    Entity, EntityId, IntoElement, LayoutId, PaintIndex, Pixels, PrepaintStateIndex, Render, Style,
    StyleRefinement, TextStyle, WeakEntity,
};
use crate::{Empty, Window};
use anyhow::Result;
use collections::FxHashSet;
use refineable::Refineable;
use std::rc::Rc;
use std::{any::TypeId, fmt, mem, ops::Range, panic};

struct AnyViewState {
    prepaint_range: Range<PrepaintStateIndex>,
    paint_range: Range<PaintIndex>,
    cache_key: ViewCacheKey,
    accessed_entities: FxHashSet<EntityId>,
    inherited_provider_accesses: Vec<ProviderScopeKey>,
    inherited_dependencies: Vec<InheritedDependency>,
}

#[derive(Default)]
struct ViewCacheKey {
    bounds: Bounds<Pixels>,
    content_mask: ContentMask<Pixels>,
    text_style: TextStyle,
}

/// A dynamically-typed handle to a view, which can be downcast to a [Entity] for a specific type.
#[derive(Clone, Debug)]
pub struct AnyView {
    entity: AnyEntity,
    render: fn(&AnyView, &mut Window, &mut App) -> AnyElement,
    cached_style: Option<Rc<StyleRefinement>>,
}

impl<V: Render> From<Entity<V>> for AnyView {
    fn from(value: Entity<V>) -> Self {
        AnyView {
            entity: value.into_any(),
            render: any_view::render::<V>,
            cached_style: None,
        }
    }
}

impl AnyView {
    /// Indicate that this view should be cached when using it as an element.
    /// When using this method, the view's previous layout and paint will be recycled from the previous frame if [Context::notify] has not been called since it was rendered.
    /// The one exception is when [Window::refresh] is called, in which case caching is ignored.
    pub fn cached(mut self, style: StyleRefinement) -> Self {
        self.cached_style = Some(style.into());
        self
    }

    /// Convert this to a weak handle.
    pub fn downgrade(&self) -> AnyWeakView {
        AnyWeakView {
            entity: self.entity.downgrade(),
            render: self.render,
        }
    }

    /// Convert this to a [Entity] of a specific type.
    /// If this handle does not contain a view of the specified type, returns itself in an `Err` variant.
    pub fn downcast<T: 'static>(self) -> Result<Entity<T>, Self> {
        match self.entity.downcast() {
            Ok(entity) => Ok(entity),
            Err(entity) => Err(Self {
                entity,
                render: self.render,
                cached_style: self.cached_style,
            }),
        }
    }

    /// Gets the [TypeId] of the underlying view.
    pub fn entity_type(&self) -> TypeId {
        self.entity.entity_type
    }

    /// Gets the entity id of this handle.
    pub fn entity_id(&self) -> EntityId {
        self.entity.entity_id()
    }
}

impl PartialEq for AnyView {
    fn eq(&self, other: &Self) -> bool {
        self.entity == other.entity
    }
}

impl Eq for AnyView {}

impl Element for AnyView {
    type RequestLayoutState = Option<AnyElement>;
    type PrepaintState = Option<AnyElement>;

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::View(self.entity_id()))
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        cx: &mut crate::LayoutCx<'_>,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let global_id = cx.global_id().cloned();
        let inspector_id = cx.inspector_id().cloned();
        cx.with_window_app(|window, cx| {
            window.with_rendered_view(self.entity_id(), |window| {
                // Disable caching when inspecting so that mouse_hit_test has all hitboxes.
                let caching_disabled = window.is_inspector_picking(cx);
                match self.cached_style.as_ref() {
                    Some(style) if !caching_disabled => {
                        let mut root_style = Style::default();
                        root_style.refine(style);
                        let layout_id = window.request_layout(root_style, None, cx);
                        (layout_id, None)
                    }
                    _ => {
                        let mut element = (self.render)(self, window, cx);
                        let mut element_cx = crate::LayoutCx::new(
                            window,
                            cx,
                            global_id.as_ref(),
                            inspector_id.as_ref(),
                        );
                        let layout_id = element.request_layout(&mut element_cx);
                        (layout_id, Some(element))
                    }
                }
            })
        })
    }

    fn prepaint(
        &mut self,
        cx: &mut crate::PrepaintCx<'_>,
        element: &mut Self::RequestLayoutState,
    ) -> Option<AnyElement> {
        let global_id = cx.global_id().cloned();
        let inspector_id = cx.inspector_id().cloned();
        let bounds = cx.bounds();
        cx.with_window_app(|window, cx| {
            window.set_view_id(self.entity_id());
            window.with_rendered_view(self.entity_id(), |window| {
                if let Some(mut element) = element.take() {
                    let mut element_cx = crate::PrepaintCx::new(
                        window,
                        cx,
                        global_id.as_ref(),
                        inspector_id.as_ref(),
                        bounds,
                    );
                    element.prepaint(&mut element_cx);
                    return Some(element);
                }

                window.with_element_state::<AnyViewState, _>(
                    global_id.as_ref().unwrap(),
                    |element_state, window| {
                        let content_mask = window.content_mask();
                        let text_style = window.text_style();

                        if let Some(mut element_state) = element_state
                            && element_state.cache_key.bounds == bounds
                            && element_state.cache_key.content_mask == content_mask
                            && element_state.cache_key.text_style == text_style
                            && !window.dirty_views.contains(&self.entity_id())
                            && !window.refreshing
                        {
                            if window.validate_inherited_cache(
                                &element_state.inherited_provider_accesses,
                                &element_state.inherited_dependencies,
                                cx,
                            ) {
                                window.replay_inherited_provider_accesses(
                                    &element_state.inherited_provider_accesses,
                                );
                                let dirty_views = window.replay_inherited_dependencies(
                                    &element_state.inherited_dependencies,
                                    cx,
                                );
                                if dirty_views.is_empty() {
                                    let prepaint_start = window.prepaint_index();
                                    window.reuse_prepaint(element_state.prepaint_range.clone());
                                    cx.entities
                                        .extend_accessed(&element_state.accessed_entities);
                                    let prepaint_end = window.prepaint_index();
                                    element_state.prepaint_range = prepaint_start..prepaint_end;

                                    return (None, element_state);
                                }
                            }
                        }

                        let refreshing = mem::replace(&mut window.refreshing, true);
                        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                            let prepaint_start = window.prepaint_index();
                            let inherited_provider_start = window.inherited_provider_access_index();
                            let inherited_dependency_start = window.inherited_dependency_index();
                            let (mut element, accessed_entities) =
                                cx.detect_accessed_entities(|cx| {
                                    let element_id_stack = window.element_id_stack.clone();
                                    let element =
                                        panic::catch_unwind(panic::AssertUnwindSafe(|| {
                                            let mut element = (self.render)(self, window, cx);
                                            let mut layout_cx = crate::LayoutCx::new(
                                                window,
                                                cx,
                                                global_id.as_ref(),
                                                inspector_id.as_ref(),
                                            );
                                            element
                                                .layout_as_root(bounds.size.into(), &mut layout_cx);
                                            window.element_id_stack.clone_from(&element_id_stack);
                                            let mut prepaint_cx = crate::PrepaintCx::new(
                                                window,
                                                cx,
                                                global_id.as_ref(),
                                                inspector_id.as_ref(),
                                                bounds,
                                            );
                                            element.prepaint_at(bounds.origin, &mut prepaint_cx);
                                            element
                                        }));
                                    window.element_id_stack.clone_from(&element_id_stack);
                                    match element {
                                        Ok(element) => element,
                                        Err(payload) => panic::resume_unwind(payload),
                                    }
                                });

                            let prepaint_end = window.prepaint_index();
                            let inherited_provider_accesses =
                                window.inherited_provider_accesses_since(inherited_provider_start);
                            let inherited_dependencies =
                                window.inherited_dependencies_since(inherited_dependency_start);

                            (
                                Some(element),
                                AnyViewState {
                                    accessed_entities,
                                    prepaint_range: prepaint_start..prepaint_end,
                                    paint_range: PaintIndex::default()..PaintIndex::default(),
                                    cache_key: ViewCacheKey {
                                        bounds,
                                        content_mask,
                                        text_style,
                                    },
                                    inherited_provider_accesses,
                                    inherited_dependencies,
                                },
                            )
                        }));
                        window.refreshing = refreshing;
                        match result {
                            Ok(result) => result,
                            Err(payload) => panic::resume_unwind(payload),
                        }
                    },
                )
            })
        })
    }

    fn paint(
        &mut self,
        cx: &mut crate::PaintCx<'_>,
        _: &mut Self::RequestLayoutState,
        element: &mut Self::PrepaintState,
    ) {
        let global_id = cx.global_id().cloned();
        let inspector_id = cx.inspector_id().cloned();
        let bounds = cx.bounds();
        cx.with_window_app(|window, cx| {
            window.with_rendered_view(self.entity_id(), |window| {
                let caching_disabled = window.is_inspector_picking(cx);
                if self.cached_style.is_some() && !caching_disabled {
                    window.with_element_state::<AnyViewState, _>(
                        global_id.as_ref().unwrap(),
                        |element_state, window| {
                            let mut element_state = element_state.unwrap();

                            let paint_start = window.paint_index();
                            let inherited_provider_start = window.inherited_provider_access_index();
                            let inherited_dependency_start = window.inherited_dependency_index();
                            let painted_element = element.is_some();

                            if let Some(element) = element {
                                let refreshing = mem::replace(&mut window.refreshing, true);
                                let mut element_cx = crate::PaintCx::new(
                                    window,
                                    cx,
                                    global_id.as_ref(),
                                    inspector_id.as_ref(),
                                    bounds,
                                );
                                element.paint(&mut element_cx);
                                window.refreshing = refreshing;
                            } else {
                                window.reuse_paint(element_state.paint_range.clone());
                            }

                            let paint_end = window.paint_index();
                            if painted_element {
                                let paint_provider_accesses = window
                                    .inherited_provider_accesses_since(inherited_provider_start);
                                let paint_dependencies =
                                    window.inherited_dependencies_since(inherited_dependency_start);
                                extend_unique_inherited_provider_accesses(
                                    &mut element_state.inherited_provider_accesses,
                                    paint_provider_accesses,
                                );
                                extend_unique_inherited_dependencies(
                                    &mut element_state.inherited_dependencies,
                                    paint_dependencies,
                                );
                            }
                            element_state.paint_range = paint_start..paint_end;

                            ((), element_state)
                        },
                    )
                } else {
                    let mut element_cx = crate::PaintCx::new(
                        window,
                        cx,
                        global_id.as_ref(),
                        inspector_id.as_ref(),
                        bounds,
                    );
                    element.as_mut().unwrap().paint(&mut element_cx);
                }
            });
        });
    }
}

fn extend_unique_inherited_dependencies(
    target: &mut Vec<InheritedDependency>,
    dependencies: Vec<InheritedDependency>,
) {
    let mut seen = target.iter().cloned().collect::<FxHashSet<_>>();

    for dependency in dependencies {
        if seen.insert(dependency.clone()) {
            target.push(dependency);
        }
    }
}

fn extend_unique_inherited_provider_accesses(
    target: &mut Vec<ProviderScopeKey>,
    provider_accesses: Vec<ProviderScopeKey>,
) {
    let mut seen = target.iter().cloned().collect::<FxHashSet<_>>();

    for provider in provider_accesses {
        if seen.insert(provider.clone()) {
            target.push(provider);
        }
    }
}

impl<V: 'static + Render> IntoElement for Entity<V> {
    type Element = AnyView;

    fn into_element(self) -> Self::Element {
        self.into()
    }
}

impl IntoElement for AnyView {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// A weak, dynamically-typed view handle that does not prevent the view from being released.
pub struct AnyWeakView {
    entity: AnyWeakEntity,
    render: fn(&AnyView, &mut Window, &mut App) -> AnyElement,
}

impl AnyWeakView {
    /// Convert to a strongly-typed handle if the referenced view has not yet been released.
    pub fn upgrade(&self) -> Option<AnyView> {
        let entity = self.entity.upgrade()?;
        Some(AnyView {
            entity,
            render: self.render,
            cached_style: None,
        })
    }
}

impl<V: 'static + Render> From<WeakEntity<V>> for AnyWeakView {
    fn from(view: WeakEntity<V>) -> Self {
        AnyWeakView {
            entity: view.into(),
            render: any_view::render::<V>,
        }
    }
}

impl PartialEq for AnyWeakView {
    fn eq(&self, other: &Self) -> bool {
        self.entity == other.entity
    }
}

impl std::fmt::Debug for AnyWeakView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnyWeakView")
            .field("entity_id", &self.entity.entity_id)
            .finish_non_exhaustive()
    }
}

mod any_view {
    use crate::{AnyElement, AnyView, App, IntoElement, Render, Window};

    pub(crate) fn render<V: 'static + Render>(
        view: &AnyView,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let view = view.clone().downcast::<V>().unwrap();
        view.update(cx, |view, cx| view.render(window, cx).into_any_element())
    }
}

/// A view that renders nothing
pub struct EmptyView;

impl Render for EmptyView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}
