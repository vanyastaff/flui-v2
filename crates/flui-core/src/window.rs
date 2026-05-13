#[cfg(any(feature = "inspector", debug_assertions))]
use crate::Inspector;
use crate::provider::{
    InheritedValue,
    registry::{InheritedDependency, InheritedRegistry, ProviderScopeKey},
};
use crate::scheduler::Instant;
use crate::{
    Action, AnyDrag, AnyElement, AnyImageCache, AnyTooltip, AnyView, App, AppContext, Arena, Asset,
    AsyncWindowContext, AvailableSpace, Background, BorderStyle, Bounds, BoxShadow, Capslock,
    Context, Corners, CursorStyle, Decorations, DevicePixels, DispatchActionListener,
    DispatchNodeId, DispatchTree, DisplayId, Edges, Effect, ElementId, ElementIdStack, Entity,
    EntityId, EventEmitter, ExternalDropEvent, ExternalDropPayload, FontId, Global,
    GlobalElementId, GlyphId, GpuSpecs, Hsla, InputHandler, IsZero, KeyBinding, KeyContext,
    KeyDownEvent, KeyEvent, Keystroke, KeystrokeEvent, LayoutId, LineLayoutIndex, MediaQueryData,
    Modifiers, ModifiersChangedEvent, MonochromeSprite, MouseButton, MouseEvent, MouseMoveEvent,
    MouseUpEvent, Path, Pixels, PlatformDisplay, PlatformInput,
    PlatformInputHandler, Point, PolychromeSprite, Priority, PromptButton,
    PromptLevel, Quad, Render, RenderGlyphParams, RenderImage, RenderImageParams, RenderSvgParams,
    Replay, ResizeEdge, SMOOTH_SVG_SCALE_FACTOR, SUBPIXEL_VARIANTS_X, SUBPIXEL_VARIANTS_Y,
    ScaledPixels, Scene, Shadow, SharedString, Size, StrikethroughStyle, Style, SubpixelSprite,
    SubscriberSet, Subscription, SystemWindowTab, SystemWindowTabController, TabStopMap,
    TaffyLayoutEngine, Task, TextRenderingMode, TextStyle, TextStyleRefinement, ThermalState,
    TransformationMatrix, Underline, UnderlineStyle, WindowAppearance, WindowBackgroundAppearance,
    WindowBounds, WindowControls, WindowDecorations, WindowOptions, WindowParams, WindowTextSystem,
    point, prelude::*, px, rems, size, transparent_black,
};
use anyhow::Result;
use collections::{FxHashMap, FxHashSet};
#[cfg(target_os = "macos")]
use core_video::pixel_buffer::CVPixelBuffer;
use derive_more::Deref;
use futures::FutureExt;
use futures::channel::oneshot;
use itertools::FoldWhile::{Continue, Done};
use itertools::Itertools;
use parking_lot::RwLock;
use refineable::Refineable;
use slotmap::SlotMap;
use smallvec::SmallVec;
use std::{
    any::{Any, TypeId},
    borrow::Cow,
    cell::{Cell, RefCell},
    cmp,
    fmt::Debug,
    hash::Hash,
    mem,
    ops::{DerefMut, Range},
    panic::{self, AssertUnwindSafe},
    rc::Rc,
    sync::{
        Arc, Weak,
        atomic::{AtomicUsize, Ordering::SeqCst},
    },
    time::Duration,
};
use util::post_inc;
use util::{ResultExt, measure};

mod prompts;
// A10a PR 1.0: private state container for `Window`. Sibling submodules added in PRs
// 1.3-1.11 will reach `Window`'s ~140 fields through this module's `pub(super)` interface.
// See `docs/superpowers/specs/2026-05-13-A10-xl-file-split-design.md` and ADR-021.
mod core;
// A10a PR 1.1: window handles (WindowId / WindowHandle / AnyWindowHandle) live in
// their own submodule. Re-exported via `pub use handle::*` so existing
// `flui_core::WindowHandle` / `flui_core::WindowId` paths keep working.
mod handle;

use crate::local_util::atomic_incr_if_not_zero;
pub use handle::*;
pub use prompts::*;

/// Default window size used when no explicit size is provided.
pub const DEFAULT_WINDOW_SIZE: Size<Pixels> = size(px(1536.), px(864.));

/// A 6:5 aspect ratio minimum window size to be used for functional,
/// additional-to-main-Zed windows, like the settings and rules library windows.
pub const DEFAULT_ADDITIONAL_WINDOW_SIZE: Size<Pixels> = Size {
    width: Pixels(900.),
    height: Pixels(750.),
};

/// Represents the two different phases when dispatching events.
#[derive(Default, Copy, Clone, Debug, Eq, PartialEq)]
pub enum DispatchPhase {
    /// After the capture phase comes the bubble phase, in which mouse event listeners are
    /// invoked front to back and keyboard event listeners are invoked from the focused element
    /// to the root of the element tree. This is the phase you'll most commonly want to use when
    /// registering event listeners.
    #[default]
    Bubble,
    /// During the initial capture phase, mouse event listeners are invoked back to front, and keyboard
    /// listeners are invoked from the root of the tree downward toward the focused element. This phase
    /// is used for special purposes such as clearing the "pressed" state for click events. If
    /// you stop event propagation during this phase, you need to know what you're doing. Handlers
    /// outside of the immediate region may rely on detecting non-local events during this phase.
    Capture,
}

impl DispatchPhase {
    /// Returns true if this represents the "bubble" phase.
    #[inline]
    pub fn bubble(self) -> bool {
        self == DispatchPhase::Bubble
    }

    /// Returns true if this represents the "capture" phase.
    #[inline]
    pub fn capture(self) -> bool {
        self == DispatchPhase::Capture
    }
}

struct WindowInvalidatorInner {
    pub dirty: bool,
    pub draw_phase: DrawPhase,
    pub dirty_views: FxHashSet<EntityId>,
}

#[derive(Clone)]
pub(crate) struct WindowInvalidator {
    inner: Rc<RefCell<WindowInvalidatorInner>>,
}

impl WindowInvalidator {
    pub fn new() -> Self {
        WindowInvalidator {
            inner: Rc::new(RefCell::new(WindowInvalidatorInner {
                dirty: true,
                draw_phase: DrawPhase::None,
                dirty_views: FxHashSet::default(),
            })),
        }
    }

    pub fn invalidate_view(&self, entity: EntityId, cx: &mut App) -> bool {
        let mut inner = self.inner.borrow_mut();
        inner.dirty_views.insert(entity);
        if inner.draw_phase == DrawPhase::None {
            inner.dirty = true;
            cx.push_effect(Effect::Notify { emitter: entity });
            true
        } else {
            false
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.inner.borrow().dirty
    }

    pub fn set_dirty(&self, dirty: bool) {
        self.inner.borrow_mut().dirty = dirty
    }

    pub fn set_phase(&self, phase: DrawPhase) {
        self.inner.borrow_mut().draw_phase = phase
    }

    pub fn take_views(&self) -> FxHashSet<EntityId> {
        mem::take(&mut self.inner.borrow_mut().dirty_views)
    }

    pub fn replace_views(&self, views: FxHashSet<EntityId>) {
        self.inner.borrow_mut().dirty_views = views;
    }

    pub fn not_drawing(&self) -> bool {
        self.inner.borrow().draw_phase == DrawPhase::None
    }

    #[track_caller]
    pub fn debug_assert_paint(&self) {
        debug_assert!(
            matches!(self.inner.borrow().draw_phase, DrawPhase::Paint),
            "this method can only be called during paint"
        );
    }

    #[track_caller]
    pub fn debug_assert_prepaint(&self) {
        debug_assert!(
            matches!(self.inner.borrow().draw_phase, DrawPhase::Prepaint),
            "this method can only be called during request_layout, or prepaint"
        );
    }

    #[track_caller]
    pub fn debug_assert_paint_or_prepaint(&self) {
        debug_assert!(
            matches!(
                self.inner.borrow().draw_phase,
                DrawPhase::Paint | DrawPhase::Prepaint
            ),
            "this method can only be called during request_layout, prepaint, or paint"
        );
    }

    /// Asserts the window is not currently in `Paint`.
    ///
    /// Used to reject frame-callback registrations from inside `paint`, which is
    /// the pattern behind upstream GPUI #56294 (Wayland 1 px content shift when
    /// an Element registers `on_next_frame` during `paint`). See
    /// `docs/research/adr/ADR-001-invalidation-scope.md` for the contract.
    #[track_caller]
    pub fn debug_assert_not_paint(&self) {
        debug_assert!(
            !matches!(self.inner.borrow().draw_phase, DrawPhase::Paint),
            "this method must not be called during paint; the callback observes the next frame, \
             not the current one. Register it from layout, an event handler, or a deferred effect."
        );
    }
}

type AnyObserver = Box<dyn FnMut(&mut Window, &mut App) -> bool + 'static>;

pub(crate) type AnyWindowFocusListener =
    Box<dyn FnMut(&WindowFocusEvent, &mut Window, &mut App) -> bool + 'static>;

pub(crate) struct WindowFocusEvent {
    pub(crate) previous_focus_path: SmallVec<[FocusId; 8]>,
    pub(crate) current_focus_path: SmallVec<[FocusId; 8]>,
}

impl WindowFocusEvent {
    pub fn is_focus_in(&self, focus_id: FocusId) -> bool {
        !self.previous_focus_path.contains(&focus_id) && self.current_focus_path.contains(&focus_id)
    }

    pub fn is_focus_out(&self, focus_id: FocusId) -> bool {
        self.previous_focus_path.contains(&focus_id) && !self.current_focus_path.contains(&focus_id)
    }
}

/// This is provided when subscribing for `Context::on_focus_out` events.
pub struct FocusOutEvent {
    /// A weak focus handle representing what was blurred.
    pub blurred: WeakFocusHandle,
}

slotmap::new_key_type! {
    /// A globally unique identifier for a focusable element.
    pub struct FocusId;
}

thread_local! {
    /// Fallback arena used when no app-specific arena is active.
    /// In production, each window draw sets CURRENT_ELEMENT_ARENA to the app's arena.
    pub(crate) static ELEMENT_ARENA: RefCell<Arena> = RefCell::new(Arena::new(1024 * 1024));

    /// Points to the current App's element arena during draw operations.
    /// This allows multiple test Apps to have isolated arenas, preventing
    /// cross-session corruption when the scheduler interleaves their tasks.
    static CURRENT_ELEMENT_ARENA: Cell<Option<*const RefCell<Arena>>> = const { Cell::new(None) };
}

/// Allocates an element in the current arena. Uses the app-specific arena if one
/// is active (during draw), otherwise falls back to the thread-local ELEMENT_ARENA.
pub(crate) fn with_element_arena<R>(f: impl FnOnce(&mut Arena) -> R) -> R {
    CURRENT_ELEMENT_ARENA.with(|current| {
        if let Some(arena_ptr) = current.get() {
            // SAFETY: The pointer is valid for the duration of the draw operation
            // that set it, and we're being called during that same draw.
            let arena_cell = unsafe { &*arena_ptr };
            f(&mut arena_cell.borrow_mut())
        } else {
            ELEMENT_ARENA.with_borrow_mut(f)
        }
    })
}

/// RAII guard that sets CURRENT_ELEMENT_ARENA for the duration of a draw operation.
/// When dropped, restores the previous arena (supporting nested draws).
pub(crate) struct ElementArenaScope {
    previous: Option<*const RefCell<Arena>>,
}

impl ElementArenaScope {
    /// Enter a scope where element allocations use the given arena.
    pub(crate) fn enter(arena: &RefCell<Arena>) -> Self {
        let previous = CURRENT_ELEMENT_ARENA.with(|current| {
            let prev = current.get();
            current.set(Some(arena as *const RefCell<Arena>));
            prev
        });
        Self { previous }
    }
}

impl Drop for ElementArenaScope {
    fn drop(&mut self) {
        CURRENT_ELEMENT_ARENA.with(|current| {
            current.set(self.previous);
        });
    }
}

/// Returned when the element arena has been used and so must be cleared before the next draw.
#[must_use]
pub struct ArenaClearNeeded {
    arena: *const RefCell<Arena>,
}

impl ArenaClearNeeded {
    /// Create a new ArenaClearNeeded that will clear the given arena.
    pub(crate) fn new(arena: &RefCell<Arena>) -> Self {
        Self {
            arena: arena as *const RefCell<Arena>,
        }
    }

    /// Clear the element arena.
    pub fn clear(self) {
        // SAFETY: The arena pointer is valid because ArenaClearNeeded is created
        // at the end of draw() and must be cleared before the next draw.
        let arena_cell = unsafe { &*self.arena };
        arena_cell.borrow_mut().clear();
    }
}

pub(crate) type FocusMap = RwLock<SlotMap<FocusId, FocusRef>>;
pub(crate) struct FocusRef {
    pub(crate) ref_count: AtomicUsize,
    pub(crate) tab_index: isize,
    pub(crate) tab_stop: bool,
}

impl FocusId {
    /// Obtains whether the element associated with this handle is currently focused.
    pub fn is_focused(&self, window: &Window) -> bool {
        window.core.focus == Some(*self)
    }

    /// Obtains whether the element associated with this handle contains the focused
    /// element or is itself focused.
    pub fn contains_focused(&self, window: &Window, cx: &App) -> bool {
        window
            .focused(cx)
            .is_some_and(|focused| self.contains(focused.id, window))
    }

    /// Obtains whether the element associated with this handle is contained within the
    /// focused element or is itself focused.
    pub fn within_focused(&self, window: &Window, cx: &App) -> bool {
        let focused = window.focused(cx);
        focused.is_some_and(|focused| focused.id.contains(*self, window))
    }

    /// Obtains whether this handle contains the given handle in the most recently rendered frame.
    pub(crate) fn contains(&self, other: Self, window: &Window) -> bool {
        window
            .core.rendered_frame
            .dispatch_tree
            .focus_contains(*self, other)
    }
}

/// A handle which can be used to track and manipulate the focused element in a window.
pub struct FocusHandle {
    pub(crate) id: FocusId,
    handles: Arc<FocusMap>,
    /// The index of this element in the tab order.
    pub tab_index: isize,
    /// Whether this element can be focused by tab navigation.
    pub tab_stop: bool,
}

impl std::fmt::Debug for FocusHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("FocusHandle({:?})", self.id))
    }
}

impl FocusHandle {
    pub(crate) fn new(handles: &Arc<FocusMap>) -> Self {
        let id = handles.write().insert(FocusRef {
            ref_count: AtomicUsize::new(1),
            tab_index: 0,
            tab_stop: false,
        });

        Self {
            id,
            tab_index: 0,
            tab_stop: false,
            handles: handles.clone(),
        }
    }

    pub(crate) fn for_id(id: FocusId, handles: &Arc<FocusMap>) -> Option<Self> {
        let lock = handles.read();
        let focus = lock.get(id)?;
        if atomic_incr_if_not_zero(&focus.ref_count) == 0 {
            return None;
        }
        Some(Self {
            id,
            tab_index: focus.tab_index,
            tab_stop: focus.tab_stop,
            handles: handles.clone(),
        })
    }

    /// Sets the tab index of the element associated with this handle.
    pub fn tab_index(mut self, index: isize) -> Self {
        self.tab_index = index;
        if let Some(focus) = self.handles.write().get_mut(self.id) {
            focus.tab_index = index;
        }
        self
    }

    /// Sets whether the element associated with this handle is a tab stop.
    ///
    /// When `false`, the element will not be included in the tab order.
    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        if let Some(focus) = self.handles.write().get_mut(self.id) {
            focus.tab_stop = tab_stop;
        }
        self
    }

    /// Converts this focus handle into a weak variant, which does not prevent it from being released.
    pub fn downgrade(&self) -> WeakFocusHandle {
        WeakFocusHandle {
            id: self.id,
            handles: Arc::downgrade(&self.handles),
        }
    }

    /// Moves the focus to the element associated with this handle.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(self, cx)
    }

    /// Obtains whether the element associated with this handle is currently focused.
    pub fn is_focused(&self, window: &Window) -> bool {
        self.id.is_focused(window)
    }

    /// Obtains whether the element associated with this handle contains the focused
    /// element or is itself focused.
    pub fn contains_focused(&self, window: &Window, cx: &App) -> bool {
        self.id.contains_focused(window, cx)
    }

    /// Obtains whether the element associated with this handle is contained within the
    /// focused element or is itself focused.
    pub fn within_focused(&self, window: &Window, cx: &mut App) -> bool {
        self.id.within_focused(window, cx)
    }

    /// Obtains whether this handle contains the given handle in the most recently rendered frame.
    pub fn contains(&self, other: &Self, window: &Window) -> bool {
        self.id.contains(other.id, window)
    }

    /// Dispatch an action on the element that rendered this focus handle
    pub fn dispatch_action(&self, action: &dyn Action, window: &mut Window, cx: &mut App) {
        if let Some(node_id) = window
            .core.rendered_frame
            .dispatch_tree
            .focusable_node_id(self.id)
        {
            window.dispatch_action_on_node(node_id, action, cx)
        }
    }
}

impl Clone for FocusHandle {
    fn clone(&self) -> Self {
        Self::for_id(self.id, &self.handles).unwrap()
    }
}

impl PartialEq for FocusHandle {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for FocusHandle {}

impl Drop for FocusHandle {
    fn drop(&mut self) {
        self.handles
            .read()
            .get(self.id)
            .unwrap()
            .ref_count
            .fetch_sub(1, SeqCst);
    }
}

/// A weak reference to a focus handle.
#[derive(Clone, Debug)]
pub struct WeakFocusHandle {
    pub(crate) id: FocusId,
    pub(crate) handles: Weak<FocusMap>,
}

impl WeakFocusHandle {
    /// Attempts to upgrade the [WeakFocusHandle] to a [FocusHandle].
    pub fn upgrade(&self) -> Option<FocusHandle> {
        let handles = self.handles.upgrade()?;
        FocusHandle::for_id(self.id, &handles)
    }
}

impl PartialEq for WeakFocusHandle {
    fn eq(&self, other: &WeakFocusHandle) -> bool {
        self.id == other.id
    }
}

impl Eq for WeakFocusHandle {}

impl PartialEq<FocusHandle> for WeakFocusHandle {
    fn eq(&self, other: &FocusHandle) -> bool {
        self.id == other.id
    }
}

impl PartialEq<WeakFocusHandle> for FocusHandle {
    fn eq(&self, other: &WeakFocusHandle) -> bool {
        self.id == other.id
    }
}

/// Focusable allows users of your view to easily
/// focus it (using window.focus_view(cx, view))
pub trait Focusable: 'static {
    /// Returns the focus handle associated with this view.
    fn focus_handle(&self, cx: &App) -> FocusHandle;
}

impl<V: Focusable> Focusable for Entity<V> {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.read(cx).focus_handle(cx)
    }
}

/// ManagedView is a view (like a Modal, Popover, Menu, etc.)
/// where the lifecycle of the view is handled by another view.
pub trait ManagedView: Focusable + EventEmitter<DismissEvent> + Render {}

impl<M: Focusable + EventEmitter<DismissEvent> + Render> ManagedView for M {}

/// Emitted by implementers of [`ManagedView`] to indicate the view should be dismissed, such as when a view is presented as a modal.
pub struct DismissEvent;

type FrameCallback = Box<dyn FnOnce(&mut Window, &mut App)>;

pub(crate) type AnyMouseListener =
    Box<dyn FnMut(&dyn Any, DispatchPhase, &mut Window, &mut App) + 'static>;

#[derive(Clone)]
pub(crate) struct CursorStyleRequest {
    pub(crate) hitbox_id: Option<HitboxId>,
    pub(crate) style: CursorStyle,
}

#[derive(Default, Eq, PartialEq)]
pub(crate) struct HitTest {
    pub(crate) ids: SmallVec<[HitboxId; 8]>,
    pub(crate) hover_hitbox_count: usize,
}

/// A type of window control area that corresponds to the platform window.
///
/// # ADR-008 contract for callers (custom title-bar implementations)
///
/// Implementations of [`Platform::on_hit_test_window_control`] consult
/// the flui-side hit-test BEFORE returning [`WindowControlArea::Drag`].
/// Concretely: when a pointer-down lands within the title-bar bounds,
/// the callback must first walk the gesture hit-tree at that point; if
/// any child element with a `mouse_down` listener (close button, tab
/// strip tab, dropdown trigger) claims the point, the callback returns
/// the matching control area (`Close` / `Max` / `Min`) or `None` so the
/// click reaches the child. Only the *bare* title-bar surface — with no
/// child claiming the point — yields [`WindowControlArea::Drag`].
///
/// This is decision 4 of `docs/research/adr/ADR-008-window-chrome-contract.md`:
/// "Drag-region is computed *after* the per-child hit-test, not instead
/// of it." Programmatic opt-in (a child element explicitly declaring
/// `.window_drag()`) re-enables the move gesture on that child by
/// returning `Drag` from inside the child's bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowControlArea {
    /// An area that allows dragging of the platform window.
    Drag,
    /// An area that allows closing of the platform window.
    Close,
    /// An area that allows maximizing of the platform window.
    Max,
    /// An area that allows minimizing of the platform window.
    Min,
}

/// An identifier for a [Hitbox] which also includes [HitboxBehavior].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct HitboxId(u64);

#[cfg(test)]
impl HitboxId {
    /// Construct a synthetic [`HitboxId`] for tests in sibling
    /// modules. Production code receives ids only from
    /// [`Window::next_hitbox_id`].
    pub(crate) fn for_test(raw: u64) -> Self {
        Self(raw)
    }
}

impl HitboxId {
    /// Checks if the hitbox with this ID is currently hovered. Returns `false` during keyboard
    /// input modality so that keyboard navigation suppresses hover highlights. Except when handling
    /// `ScrollWheelEvent`, this is typically what you want when determining whether to handle mouse
    /// events or paint hover styles.
    ///
    /// See [`Hitbox::is_hovered`] for details.
    pub fn is_hovered(self, window: &Window) -> bool {
        // If this hitbox has captured the pointer, it's always considered hovered
        if window.core.captured_hitbox == Some(self) {
            return true;
        }
        if window.last_input_was_keyboard() {
            return false;
        }
        let hit_test = &window.core.mouse_hit_test;
        for id in hit_test.ids.iter().take(hit_test.hover_hitbox_count) {
            if self == *id {
                return true;
            }
        }
        false
    }

    /// Checks if the hitbox with this ID contains the mouse and should handle scroll events.
    /// Typically this should only be used when handling `ScrollWheelEvent`, and otherwise
    /// `is_hovered` should be used. See the documentation of `Hitbox::is_hovered` for details about
    /// this distinction.
    pub fn should_handle_scroll(self, window: &Window) -> bool {
        window.core.mouse_hit_test.ids.contains(&self)
    }

    fn next(mut self) -> HitboxId {
        HitboxId(self.0.wrapping_add(1))
    }
}

/// A rectangular region that potentially blocks hitboxes inserted prior.
/// See [Window::insert_hitbox] for more details.
#[derive(Clone, Debug, Deref)]
pub struct Hitbox {
    /// A unique identifier for the hitbox.
    pub id: HitboxId,
    /// The bounds of the hitbox.
    #[deref]
    pub bounds: Bounds<Pixels>,
    /// The content mask when the hitbox was inserted.
    pub content_mask: ContentMask<Pixels>,
    /// Flags that specify hitbox behavior.
    pub behavior: HitboxBehavior,
}

impl Hitbox {
    /// Checks if the hitbox is currently hovered. Returns `false` during keyboard input modality
    /// so that keyboard navigation suppresses hover highlights. Except when handling
    /// `ScrollWheelEvent`, this is typically what you want when determining whether to handle mouse
    /// events or paint hover styles.
    ///
    /// This can return `false` even when the hitbox contains the mouse, if a hitbox in front of
    /// this sets `HitboxBehavior::BlockMouse` (`InteractiveElement::occlude`) or
    /// `HitboxBehavior::BlockMouseExceptScroll` (`InteractiveElement::block_mouse_except_scroll`),
    /// or if the current input modality is keyboard (see [`Window::last_input_was_keyboard`]).
    ///
    /// Handling of `ScrollWheelEvent` should typically use `should_handle_scroll` instead.
    /// Concretely, this is due to use-cases like overlays that cause the elements under to be
    /// non-interactive while still allowing scrolling. More abstractly, this is because
    /// `is_hovered` is about element interactions directly under the mouse - mouse moves, clicks,
    /// hover styling, etc. In contrast, scrolling is about finding the current outer scrollable
    /// container.
    pub fn is_hovered(&self, window: &Window) -> bool {
        self.id.is_hovered(window)
    }

    /// Checks if the hitbox contains the mouse and should handle scroll events. Typically this
    /// should only be used when handling `ScrollWheelEvent`, and otherwise `is_hovered` should be
    /// used. See the documentation of `Hitbox::is_hovered` for details about this distinction.
    ///
    /// This can return `false` even when the hitbox contains the mouse, if a hitbox in front of
    /// this sets `HitboxBehavior::BlockMouse` (`InteractiveElement::occlude`).
    pub fn should_handle_scroll(&self, window: &Window) -> bool {
        self.id.should_handle_scroll(window)
    }
}

/// How the hitbox affects mouse behavior.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum HitboxBehavior {
    /// Normal hitbox mouse behavior, doesn't affect mouse handling for other hitboxes.
    #[default]
    Normal,

    /// All hitboxes behind this hitbox will be ignored and so will have `hitbox.is_hovered() ==
    /// false` and `hitbox.should_handle_scroll() == false`. Typically for elements this causes
    /// skipping of all mouse events, hover styles, and tooltips. This flag is set by
    /// [`InteractiveElement::occlude`].
    ///
    /// For mouse handlers that check those hitboxes, this behaves the same as registering a
    /// bubble-phase handler for every mouse event type:
    ///
    /// ```ignore
    /// window.on_mouse_event(move |_: &EveryMouseEventTypeHere, phase, window, cx| {
    ///     if phase == DispatchPhase::Capture && hitbox.is_hovered(window) {
    ///         cx.stop_propagation();
    ///     }
    /// })
    /// ```
    ///
    /// This has effects beyond event handling - any use of hitbox checking, such as hover
    /// styles and tooltips. These other behaviors are the main point of this mechanism. An
    /// alternative might be to not affect mouse event handling - but this would allow
    /// inconsistent UI where clicks and moves interact with elements that are not considered to
    /// be hovered.
    BlockMouse,

    /// All hitboxes behind this hitbox will have `hitbox.is_hovered() == false`, even when
    /// `hitbox.should_handle_scroll() == true`. Typically for elements this causes all mouse
    /// interaction except scroll events to be ignored - see the documentation of
    /// [`Hitbox::is_hovered`] for details. This flag is set by
    /// [`InteractiveElement::block_mouse_except_scroll`].
    ///
    /// For mouse handlers that check those hitboxes, this behaves the same as registering a
    /// bubble-phase handler for every mouse event type **except** `ScrollWheelEvent`:
    ///
    /// ```ignore
    /// window.on_mouse_event(move |_: &EveryMouseEventTypeExceptScroll, phase, window, cx| {
    ///     if phase == DispatchPhase::Bubble && hitbox.should_handle_scroll(window) {
    ///         cx.stop_propagation();
    ///     }
    /// })
    /// ```
    ///
    /// See the documentation of [`Hitbox::is_hovered`] for details of why `ScrollWheelEvent` is
    /// handled differently than other mouse events. If also blocking these scroll events is
    /// desired, then a `cx.stop_propagation()` handler like the one above can be used.
    ///
    /// This has effects beyond event handling - this affects any use of `is_hovered`, such as
    /// hover styles and tooltips. These other behaviors are the main point of this mechanism.
    /// An alternative might be to not affect mouse event handling - but this would allow
    /// inconsistent UI where clicks and moves interact with elements that are not considered to
    /// be hovered.
    BlockMouseExceptScroll,
}

/// An identifier for a tooltip.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct TooltipId(usize);

impl TooltipId {
    /// Checks if the tooltip is currently hovered.
    pub fn is_hovered(&self, window: &Window) -> bool {
        window
            .core.tooltip_bounds
            .as_ref()
            .is_some_and(|tooltip_bounds| {
                tooltip_bounds.id == *self
                    && tooltip_bounds.bounds.contains(&window.mouse_position())
            })
    }
}

pub(crate) struct TooltipBounds {
    id: TooltipId,
    bounds: Bounds<Pixels>,
}

#[derive(Clone)]
pub(crate) struct TooltipRequest {
    id: TooltipId,
    tooltip: AnyTooltip,
}

pub(crate) struct DeferredDraw {
    current_view: EntityId,
    priority: usize,
    parent_node: DispatchNodeId,
    global_id: Option<GlobalElementId>,
    inspector_id: Option<crate::InspectorElementId>,
    bounds: Bounds<Pixels>,
    element_id_stack: ElementIdStack,
    text_style_stack: Vec<TextStyleRefinement>,
    content_mask: Option<ContentMask<Pixels>>,
    rem_size: Pixels,
    element: Option<AnyElement>,
    absolute_offset: Point<Pixels>,
    prepaint_range: Range<PrepaintStateIndex>,
    paint_range: Range<PaintIndex>,
}

pub(crate) struct Frame {
    pub(crate) focus: Option<FocusId>,
    pub(crate) window_active: bool,
    pub(crate) element_states: FxHashMap<(GlobalElementId, TypeId), ElementStateBox>,
    accessed_element_states: Vec<(GlobalElementId, TypeId)>,
    pub(crate) mouse_listeners: Vec<Option<AnyMouseListener>>,
    pub(crate) dispatch_tree: DispatchTree,
    pub(crate) scene: Scene,
    pub(crate) hitboxes: Vec<Hitbox>,
    pub(crate) window_control_hitboxes: Vec<(WindowControlArea, Hitbox)>,
    pub(crate) deferred_draws: Vec<DeferredDraw>,
    pub(crate) input_handlers: Vec<Option<PlatformInputHandler>>,
    pub(crate) tooltip_requests: Vec<Option<TooltipRequest>>,
    pub(crate) cursor_styles: Vec<CursorStyleRequest>,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) debug_bounds: FxHashMap<String, Bounds<Pixels>>,
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(crate) next_inspector_instance_ids: FxHashMap<Rc<crate::InspectorElementPath>, usize>,
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(crate) inspector_hitboxes: FxHashMap<HitboxId, crate::InspectorElementId>,
    pub(crate) tab_stops: TabStopMap,
}

#[derive(Clone, Default)]
pub(crate) struct PrepaintStateIndex {
    hitboxes_index: usize,
    tooltips_index: usize,
    deferred_draws_index: usize,
    dispatch_tree_index: usize,
    accessed_element_states_index: usize,
    line_layout_index: LineLayoutIndex,
}

#[derive(Clone, Default)]
pub(crate) struct PaintIndex {
    scene_index: usize,
    mouse_listeners_index: usize,
    input_handlers_index: usize,
    cursor_styles_index: usize,
    accessed_element_states_index: usize,
    tab_handle_index: usize,
    line_layout_index: LineLayoutIndex,
}

impl Frame {
    pub(crate) fn new(dispatch_tree: DispatchTree) -> Self {
        Frame {
            focus: None,
            window_active: false,
            element_states: FxHashMap::default(),
            accessed_element_states: Vec::new(),
            mouse_listeners: Vec::new(),
            dispatch_tree,
            scene: Scene::default(),
            hitboxes: Vec::new(),
            window_control_hitboxes: Vec::new(),
            deferred_draws: Vec::new(),
            input_handlers: Vec::new(),
            tooltip_requests: Vec::new(),
            cursor_styles: Vec::new(),

            #[cfg(any(test, feature = "test-support"))]
            debug_bounds: FxHashMap::default(),

            #[cfg(any(feature = "inspector", debug_assertions))]
            next_inspector_instance_ids: FxHashMap::default(),

            #[cfg(any(feature = "inspector", debug_assertions))]
            inspector_hitboxes: FxHashMap::default(),
            tab_stops: TabStopMap::default(),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.element_states.clear();
        self.accessed_element_states.clear();
        self.mouse_listeners.clear();
        self.dispatch_tree.clear();
        self.scene.clear();
        self.input_handlers.clear();
        self.tooltip_requests.clear();
        self.cursor_styles.clear();
        self.hitboxes.clear();
        self.window_control_hitboxes.clear();
        self.deferred_draws.clear();
        self.tab_stops.clear();
        self.focus = None;

        #[cfg(any(test, feature = "test-support"))]
        {
            self.debug_bounds.clear();
        }

        #[cfg(any(feature = "inspector", debug_assertions))]
        {
            self.next_inspector_instance_ids.clear();
            self.inspector_hitboxes.clear();
        }
    }

    pub(crate) fn cursor_style(&self, window: &Window) -> Option<CursorStyle> {
        self.cursor_styles
            .iter()
            .rev()
            .fold_while(None, |style, request| match request.hitbox_id {
                None => Done(Some(request.style)),
                Some(hitbox_id) => Continue(
                    style.or_else(|| hitbox_id.is_hovered(window).then_some(request.style)),
                ),
            })
            .into_inner()
    }

    pub(crate) fn hit_test(&self, position: Point<Pixels>) -> HitTest {
        let mut set_hover_hitbox_count = false;
        let mut hit_test = HitTest::default();
        for hitbox in self.hitboxes.iter().rev() {
            let bounds = hitbox.bounds.intersect(&hitbox.content_mask.bounds);
            if bounds.contains(&position) {
                hit_test.ids.push(hitbox.id);
                if !set_hover_hitbox_count
                    && hitbox.behavior == HitboxBehavior::BlockMouseExceptScroll
                {
                    hit_test.hover_hitbox_count = hit_test.ids.len();
                    set_hover_hitbox_count = true;
                }
                if hitbox.behavior == HitboxBehavior::BlockMouse {
                    break;
                }
            }
        }
        if !set_hover_hitbox_count {
            hit_test.hover_hitbox_count = hit_test.ids.len();
        }
        hit_test
    }

    pub(crate) fn focus_path(&self) -> SmallVec<[FocusId; 8]> {
        self.focus
            .map(|focus_id| self.dispatch_tree.focus_path(focus_id))
            .unwrap_or_default()
    }

    pub(crate) fn finish(&mut self, prev_frame: &mut Self) {
        for element_state_key in &self.accessed_element_states {
            if let Some((element_state_key, element_state)) =
                prev_frame.element_states.remove_entry(element_state_key)
            {
                self.element_states.insert(element_state_key, element_state);
            }
        }

        self.scene.finish();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
enum InputModality {
    Mouse,
    Keyboard,
}

/// Holds the state for a specific window.
///
/// A10a PR 1.0: fields live on [`WindowCore`](core::WindowCore) in `window/core.rs`.
/// Plain field embedding (no `Deref`, no `Box`/`Arc`/`Rc` wrapper) preserves
/// `Rc::ptr_eq` semantics for the `active` / `needs_present` / `input_rate_tracker`
/// fields shared with platform callbacks.
///
/// **PR 1.0 amendment to ADR-021 Practice 1**: `WindowCore` and the `core` field are
/// `pub(crate)` (not `pub(super)`) because crate-internal callers in `crate::app`,
/// `crate::view`, `crate::element`, etc. previously accessed `Window`'s `pub(crate)`
/// fields directly. Tightening to `pub(super)` requires introducing ~30 accessor
/// methods on `Window`; that work is deferred and tracked alongside K06's
/// `BuildOwner`/`PipelineOwner`/`SemanticsOwner` redesign (see ROADMAP K06 entry).
/// The Tier-A → Tier-B+ boundary is still enforced: downstream crates
/// (`flui-framework`, `flui-widgets`, examples) cannot name `WindowCore` because the
/// type is `pub(crate)`.
///
/// See `docs/superpowers/specs/2026-05-13-A10-xl-file-split-design.md` Decisions
/// D1/D11/D12 and ADR-021 Practice 1.
pub struct Window {
    pub(crate) core: core::WindowCore,
}

#[derive(Clone, Debug, Default)]
struct ModifierState {
    modifiers: Modifiers,
    saw_keystroke: bool,
}

/// Tracks input event timestamps to determine if input is arriving at a high rate.
/// Used for selective VRR (Variable Refresh Rate) optimization.
#[derive(Clone, Debug)]
pub(crate) struct InputRateTracker {
    timestamps: Vec<Instant>,
    window: Duration,
    inputs_per_second: u32,
    sustain_until: Instant,
    sustain_duration: Duration,
}

impl Default for InputRateTracker {
    fn default() -> Self {
        Self {
            timestamps: Vec::new(),
            window: Duration::from_millis(100),
            inputs_per_second: 60,
            sustain_until: Instant::now(),
            sustain_duration: Duration::from_secs(1),
        }
    }
}

impl InputRateTracker {
    pub(crate) fn record_input(&mut self) {
        let now = Instant::now();
        self.timestamps.push(now);
        self.prune_old_timestamps(now);

        let min_events = self.inputs_per_second as u128 * self.window.as_millis() / 1000;
        if self.timestamps.len() as u128 >= min_events {
            self.sustain_until = now + self.sustain_duration;
        }
    }

    pub(crate) fn is_high_rate(&self) -> bool {
        Instant::now() < self.sustain_until
    }

    fn prune_old_timestamps(&mut self, now: Instant) {
        self.timestamps
            .retain(|&t| now.duration_since(t) <= self.window);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DrawPhase {
    None,
    Prepaint,
    Paint,
    Focus,
}

#[derive(Default, Debug)]
struct PendingInput {
    keystrokes: SmallVec<[Keystroke; 1]>,
    focus: Option<FocusId>,
    timer: Option<Task<()>>,
    needs_timeout: bool,
}

pub(crate) struct ElementStateBox {
    pub(crate) inner: Box<dyn Any>,
    #[cfg(debug_assertions)]
    pub(crate) type_name: &'static str,
}

fn default_bounds(display_id: Option<DisplayId>, cx: &mut App) -> WindowBounds {
    // TODO, BUG: if you open a window with the currently active window
    // on the stack, this will erroneously fallback to `None`
    //
    // TODO these should be the initial window bounds not considering maximized/fullscreen
    let active_window_bounds = cx
        .active_window()
        .and_then(|w| w.update(cx, |_, window, _| window.window_bounds()).ok());

    const CASCADE_OFFSET: f32 = 25.0;

    let display = display_id
        .map(|id| cx.find_display(id))
        .unwrap_or_else(|| cx.primary_display());

    let default_placement = || Bounds::new(point(px(0.), px(0.)), DEFAULT_WINDOW_SIZE);

    // Use visible_bounds to exclude taskbar/dock areas
    let display_bounds = display
        .as_ref()
        .map(|d| d.visible_bounds())
        .unwrap_or_else(default_placement);

    let (
        Bounds {
            origin: base_origin,
            size: base_size,
        },
        window_bounds_ctor,
    ): (_, fn(Bounds<Pixels>) -> WindowBounds) = match active_window_bounds {
        Some(bounds) => match bounds {
            WindowBounds::Windowed(bounds) => (bounds, WindowBounds::Windowed),
            WindowBounds::Maximized(bounds) => (bounds, WindowBounds::Maximized),
            WindowBounds::Fullscreen(bounds) => (bounds, WindowBounds::Fullscreen),
        },
        None => (
            display
                .as_ref()
                .map(|d| d.default_bounds())
                .unwrap_or_else(default_placement),
            WindowBounds::Windowed,
        ),
    };

    let cascade_offset = point(px(CASCADE_OFFSET), px(CASCADE_OFFSET));
    let proposed_origin = base_origin + cascade_offset;
    let proposed_bounds = Bounds::new(proposed_origin, base_size);

    let display_right = display_bounds.origin.x + display_bounds.size.width;
    let display_bottom = display_bounds.origin.y + display_bounds.size.height;
    let window_right = proposed_bounds.origin.x + proposed_bounds.size.width;
    let window_bottom = proposed_bounds.origin.y + proposed_bounds.size.height;

    let fits_horizontally = window_right <= display_right;
    let fits_vertically = window_bottom <= display_bottom;

    let final_origin = match (fits_horizontally, fits_vertically) {
        (true, true) => proposed_origin,
        (false, true) => point(display_bounds.origin.x, base_origin.y),
        (true, false) => point(base_origin.x, display_bounds.origin.y),
        (false, false) => display_bounds.origin,
    };
    window_bounds_ctor(Bounds::new(final_origin, base_size))
}

impl Window {
    pub(crate) fn new(
        handle: AnyWindowHandle,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<Self> {
        let WindowOptions {
            window_bounds,
            titlebar,
            focus,
            show,
            kind,
            is_movable,
            is_resizable,
            is_minimizable,
            display_id,
            window_background,
            app_id,
            window_min_size,
            window_decorations,
            #[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
            tabbing_identifier,
        } = options;

        let window_bounds = window_bounds.unwrap_or_else(|| default_bounds(display_id, cx));
        let mut platform_window = cx.platform.open_window(
            handle,
            WindowParams {
                bounds: window_bounds.get_bounds(),
                titlebar,
                kind,
                is_movable,
                is_resizable,
                is_minimizable,
                focus,
                show,
                display_id,
                window_min_size,
                #[cfg(target_os = "macos")]
                tabbing_identifier,
            },
        )?;

        let tab_bar_visible = platform_window.tab_bar_visible();
        SystemWindowTabController::init_visible(cx, tab_bar_visible);
        if let Some(tabs) = platform_window.tabbed_windows() {
            SystemWindowTabController::add_tab(cx, handle.window_id(), tabs);
        }

        let display_id = platform_window.display().map(|display| display.id());
        let sprite_atlas = platform_window.sprite_atlas();
        let mouse_position = platform_window.mouse_position();
        let modifiers = platform_window.modifiers();
        let capslock = platform_window.capslock();
        let content_size = platform_window.content_size();
        let scale_factor = platform_window.scale_factor();
        let appearance = platform_window.appearance();
        let text_system = Arc::new(WindowTextSystem::new(cx.text_system().clone()));
        let invalidator = WindowInvalidator::new();
        let active = Rc::new(Cell::new(platform_window.is_active()));
        let hovered = Rc::new(Cell::new(platform_window.is_hovered()));
        let needs_present = Rc::new(Cell::new(false));
        let input_rate_tracker = Rc::new(RefCell::new(InputRateTracker::default()));
        let last_frame_time = Rc::new(Cell::new(None));

        platform_window
            .request_decorations(window_decorations.unwrap_or(WindowDecorations::Server));
        platform_window.set_background_appearance(window_background);

        match window_bounds {
            WindowBounds::Fullscreen(_) => platform_window.toggle_fullscreen(),
            WindowBounds::Maximized(_) => platform_window.zoom(),
            WindowBounds::Windowed(_) => {}
        }

        platform_window.on_close(Box::new({
            let window_id = handle.window_id();
            let mut cx = cx.to_async();
            move || {
                let _ = handle.update(&mut cx, |_, window, _| window.remove_window());
                let _ = cx.update(|cx| {
                    SystemWindowTabController::remove_tab(cx, window_id);
                });
            }
        }));
        platform_window.on_request_frame(Box::new({
            let mut cx = cx.to_async();
            let invalidator = invalidator.clone();
            let active = active.clone();
            let needs_present = needs_present.clone();
            // K04 Task 36: `next_frame_callbacks` now lives on `Window`
            // directly as `RefCell<SmallVec<...>>`. The platform callback
            // reaches it via `handle.update(...)` rather than holding a
            // separate `Rc` clone.
            let input_rate_tracker = input_rate_tracker.clone();
            // A10a PR 1.0 review pass (flui-arch-reviewer IMP): clone
            // `last_frame_time` for the closure so the canonical `Rc`
            // also lives on `WindowCore`. Both clones point at the same
            // heap allocation — `Rc::ptr_eq` invariant preserved.
            let last_frame_time = last_frame_time.clone();
            move |request_frame_options| {
                let thermal_state = handle
                    .update(&mut cx, |_, _, cx| cx.thermal_state())
                    .log_err();

                if thermal_state == Some(ThermalState::Serious)
                    || thermal_state == Some(ThermalState::Critical)
                {
                    let now = Instant::now();
                    let last_frame_time = last_frame_time.replace(Some(now));

                    if let Some(last_frame) = last_frame_time
                        && now.duration_since(last_frame) < Duration::from_micros(16667)
                    {
                        return;
                    }
                }

                // K04 Tasks 33/36 + review fix #5: drain pre-frame callbacks
                // (the renamed `on_next_frame` queue) on the production
                // platform path. `take` the storage so callbacks queueing
                // more pre-frame work fire next frame, not this one.
                //
                // Dual-drain guard: in debug builds we assert that no frame
                // is currently in flight from a parallel `App::run_frame`
                // path (which today only runs through `TestApp::advance_frame`
                // and therefore should never race with this platform
                // callback). If a future spec makes `run_frame` the
                // production entrypoint, this guard becomes the migration
                // checkpoint — the assert will trip if the new code path
                // re-enters here before tearing the platform callback down.
                handle
                    .update(&mut cx, |_, window, cx| {
                        debug_assert_eq!(
                            cx.current_phase(),
                            crate::frame::FramePhase::Idle,
                            "platform on_request_frame fired while App::run_frame is mid-frame; dual-drain hazard"
                        );
                        let drained: SmallVec<[FrameCallback; 4]> =
                            RefCell::borrow_mut(&window.core.next_frame_callbacks)
                                .drain(..)
                                .collect();
                        for callback in drained {
                            callback(window, cx);
                        }
                    })
                    .log_err();

                // K04 Task 32: drain the `request_next_frame` flag. Any
                // caller that hit `Window::request_animation_frame` since
                // the last frame set the flag; the act of clearing it now
                // marks the invalidator dirty so the `if is_dirty()
                // || force_render` predicate below redraws. Multiple
                // pending calls collapse to a single redraw.
                handle
                    .update(&mut cx, |_, window, _| {
                        if window.core.request_next_frame.replace(false) {
                            window.core.invalidator.set_dirty(true);
                        }
                    })
                    .log_err();

                // Keep presenting if input was recently arriving at a high rate (>= 60fps).
                // Once high-rate input is detected, we sustain presentation for 1 second
                // to prevent display underclocking during active input.
                let needs_present = request_frame_options.require_presentation
                    || needs_present.get()
                    || (active.get() && input_rate_tracker.borrow_mut().is_high_rate());

                if invalidator.is_dirty() || request_frame_options.force_render {
                    measure("frame duration", || {
                        handle
                            .update(&mut cx, |_, window, cx| {
                                let arena_clear_needed = window.draw(cx);
                                window.present();
                                arena_clear_needed.clear();
                            })
                            .log_err();
                    })
                } else if needs_present {
                    handle
                        .update(&mut cx, |_, window, _| window.present())
                        .log_err();
                }

                handle
                    .update(&mut cx, |_, window, _| {
                        window.complete_frame();
                    })
                    .log_err();

                // K04 review fix #10: drain `post_frame_callbacks` after
                // `complete_frame`. Without this, the production platform
                // path (`on_request_frame`) silently ignores every
                // `Window::on_post_frame` / `Context::on_post_frame` /
                // `AsyncWindowContext::on_post_frame` callback — those
                // would only fire from the test path via `App::run_frame`.
                handle
                    .update(&mut cx, |_, window, cx| {
                        let drained: SmallVec<[FrameCallback; 4]> =
                            RefCell::borrow_mut(&window.core.post_frame_callbacks)
                                .drain(..)
                                .collect();
                        for callback in drained {
                            callback(window, cx);
                        }
                    })
                    .log_err();
            }
        }));
        platform_window.on_resize(Box::new({
            let mut cx = cx.to_async();
            move |_, _| {
                handle
                    .update(&mut cx, |_, window, cx| window.bounds_changed(cx))
                    .log_err();
            }
        }));
        platform_window.on_moved(Box::new({
            let mut cx = cx.to_async();
            move || {
                handle
                    .update(&mut cx, |_, window, cx| window.bounds_changed(cx))
                    .log_err();
            }
        }));
        platform_window.on_appearance_changed(Box::new({
            let mut cx = cx.to_async();
            move || {
                handle
                    .update(&mut cx, |_, window, cx| window.appearance_changed(cx))
                    .log_err();
            }
        }));
        platform_window.on_active_status_change(Box::new({
            let mut cx = cx.to_async();
            move |active| {
                handle
                    .update(&mut cx, |_, window, cx| {
                        window.core.active.set(active);
                        window.core.modifiers = window.core.platform_window.modifiers();
                        window.core.capslock = window.core.platform_window.capslock();
                        window
                            .core.activation_observers
                            .clone()
                            .retain(&(), |callback| callback(window, cx));

                        window.bounds_changed(cx);
                        window.refresh();

                        SystemWindowTabController::update_last_active(cx, window.core.handle.id);
                    })
                    .log_err();
            }
        }));
        platform_window.on_hover_status_change(Box::new({
            let mut cx = cx.to_async();
            move |active| {
                handle
                    .update(&mut cx, |_, window, _| {
                        window.core.hovered.set(active);
                        window.refresh();
                    })
                    .log_err();
            }
        }));
        platform_window.on_input({
            let mut cx = cx.to_async();
            Box::new(move |event| {
                handle
                    .update(&mut cx, |_, window, cx| window.dispatch_event(event, cx))
                    .log_err()
                    .unwrap_or(DispatchEventResult::default())
            })
        });
        platform_window.on_hit_test_window_control({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, window, _cx| {
                        for (area, hitbox) in &window.core.rendered_frame.window_control_hitboxes {
                            if window.core.mouse_hit_test.ids.contains(&hitbox.id) {
                                return Some(*area);
                            }
                        }
                        None
                    })
                    .log_err()
                    .unwrap_or(None)
            })
        });
        platform_window.on_move_tab_to_new_window({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, _window, cx| {
                        SystemWindowTabController::move_tab_to_new_window(cx, handle.window_id());
                    })
                    .log_err();
            })
        });
        platform_window.on_merge_all_windows({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, _window, cx| {
                        SystemWindowTabController::merge_all_windows(cx, handle.window_id());
                    })
                    .log_err();
            })
        });
        platform_window.on_select_next_tab({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, _window, cx| {
                        SystemWindowTabController::select_next_tab(cx, handle.window_id());
                    })
                    .log_err();
            })
        });
        platform_window.on_select_previous_tab({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, _window, cx| {
                        SystemWindowTabController::select_previous_tab(cx, handle.window_id())
                    })
                    .log_err();
            })
        });
        platform_window.on_toggle_tab_bar({
            let mut cx = cx.to_async();
            Box::new(move || {
                handle
                    .update(&mut cx, |_, window, cx| {
                        let tab_bar_visible = window.core.platform_window.tab_bar_visible();
                        SystemWindowTabController::set_visible(cx, tab_bar_visible);
                    })
                    .log_err();
            })
        });

        if let Some(app_id) = app_id {
            platform_window.set_app_id(&app_id);
        }

        platform_window.map_window().unwrap();

        // A10a PR 1.0: build WindowCore first, then wrap. Plain field embedding
        // (no Box/Arc) keeps Rc<Cell<...>> heap allocations of `active`,
        // `needs_present`, and `input_rate_tracker` intact — platform-callback
        // clones still satisfy `Rc::ptr_eq` against the canonical fields.
        Ok(Window {
            core: core::WindowCore {
                handle,
                invalidator,
                removed: false,
                platform_window,
                display_id,
                sprite_atlas,
                text_system,
                text_rendering_mode: cx.text_rendering_mode.clone(),
                rem_size: px(16.),
                rem_size_override_stack: SmallVec::new(),
                viewport_size: content_size,
                layout_engine: Some(TaffyLayoutEngine::new()),
                root: None,
                element_id_stack: ElementIdStack::default(),
                text_style_stack: Vec::new(),
                rendered_entity_stack: Vec::new(),
                element_offset_stack: Vec::new(),
                content_mask_stack: Vec::new(),
                element_opacity: 1.0,
                requested_autoscroll: None,
                inherited_registry: InheritedRegistry::default(),
                rendered_frame: Frame::new(DispatchTree::new(
                    cx.keymap.clone(),
                    cx.actions.clone(),
                )),
                next_frame: Frame::new(DispatchTree::new(cx.keymap.clone(), cx.actions.clone())),
                next_frame_callbacks: RefCell::new(SmallVec::new()),
                post_frame_callbacks: RefCell::new(SmallVec::new()),
                request_next_frame: Cell::new(false),
                next_hitbox_id: HitboxId(0),
                next_tooltip_id: TooltipId::default(),
                tooltip_bounds: None,
                dirty_views: FxHashSet::default(),
                focus_listeners: SubscriberSet::new(),
                focus_lost_listeners: SubscriberSet::new(),
                default_prevented: true,
                mouse_position,
                mouse_hit_test: HitTest::default(),
                hit_test_behaviors: collections::FxHashMap::default(),
                pending_recognizers: collections::FxHashMap::default(),
                gesture_binding: crate::gesture::GestureBinding::default(),
                modifiers,
                capslock,
                scale_factor,
                bounds_observers: SubscriberSet::new(),
                display_change_observers: SubscriberSet::new(),
                is_movable,
                is_resizable,
                is_minimizable,
                appearance,
                appearance_observers: SubscriberSet::new(),
                active,
                hovered,
                needs_present,
                input_rate_tracker,
                last_frame_time,
                last_input_modality: InputModality::Mouse,
                refreshing: false,
                activation_observers: SubscriberSet::new(),
                focus: None,
                focus_enabled: true,
                pending_input: None,
                pending_modifier: ModifierState::default(),
                pending_input_observers: SubscriberSet::new(),
                prompt: None,
                client_inset: None,
                image_cache_stack: Vec::new(),
                captured_hitbox: None,
                #[cfg(any(feature = "inspector", debug_assertions))]
                inspector: None,
            },
        })
    }

    pub(crate) fn new_focus_listener(
        &self,
        value: AnyWindowFocusListener,
    ) -> (Subscription, impl FnOnce() + use<>) {
        self.core.focus_listeners.insert((), value)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[expect(missing_docs)]
pub struct DispatchEventResult {
    pub propagate: bool,
    pub default_prevented: bool,
}

/// Indicates which region of the window is visible. Content falling outside of this mask will not be
/// rendered. Currently, only rectangular content masks are supported, but we give the mask its own type
/// to leave room to support more complex shapes in the future.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct ContentMask<P: Clone + Debug + Default + PartialEq> {
    /// The bounds
    pub bounds: Bounds<P>,
}

impl ContentMask<Pixels> {
    /// Scale the content mask's pixel units by the given scaling factor.
    pub fn scale(&self, factor: f32) -> ContentMask<ScaledPixels> {
        ContentMask {
            bounds: self.bounds.scale(factor),
        }
    }

    /// Intersect the content mask with the given content mask.
    pub fn intersect(&self, other: &Self) -> Self {
        let bounds = self.bounds.intersect(&other.bounds);
        ContentMask { bounds }
    }
}

impl Window {
    fn mark_view_dirty(&mut self, view_id: EntityId) {
        // Mark ancestor views as dirty. If already in the `dirty_views` set, then all its ancestors
        // should already be dirty.
        for view_id in self
            .core.rendered_frame
            .dispatch_tree
            .view_path_reversed(view_id)
        {
            if !self.core.dirty_views.insert(view_id) {
                break;
            }
        }
    }

    /// Registers a callback to be invoked when the window appearance changes.
    pub fn observe_window_appearance(
        &self,
        mut callback: impl FnMut(&mut Window, &mut App) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.core.appearance_observers.insert(
            (),
            Box::new(move |window, cx| {
                callback(window, cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// ADR-007: observe display changes for *this* window.
    ///
    /// The callback fires when:
    /// - this window's bound display id changes (the window moved between
    ///   outputs, or its output was disconnected and the window was
    ///   reattached to the primary display — see ADR-007 decision 5), OR
    /// - that display's scale factor changes (DPI shift on the current
    ///   output, e.g. user switched scaling in System Settings).
    ///
    /// This is *distinct* from `bounds_observers` (window-size-only) and
    /// from `Platform::on_displays_changed` (app-wide: a display was
    /// added or removed somewhere, not necessarily affecting this window).
    ///
    /// The eventually-consistent guarantee from ADR-007 decision 4: between
    /// any two consecutive frames, `Window::scale_factor()` reflects what
    /// the platform currently reports for the window's display. Observers
    /// run synchronously inside `bounds_changed`; ordering relative to
    /// `bounds_observers` is `bounds_observers` first, then
    /// `display_change_observers` — callers chaining the two should not
    /// assume the reverse.
    ///
    /// See: `docs/research/adr/ADR-007-display-lifecycle.md` — decision 3.
    pub fn observe_display_change(
        &self,
        mut callback: impl FnMut(&mut Window, &mut App) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.core.display_change_observers.insert(
            (),
            Box::new(move |window, cx| {
                callback(window, cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// Replaces the root entity of the window with a new one.
    pub fn replace_root<E>(
        &mut self,
        cx: &mut App,
        build_view: impl FnOnce(&mut Window, &mut Context<E>) -> E,
    ) -> Entity<E>
    where
        E: 'static + Render,
    {
        let view = cx.new(|cx| build_view(self, cx));
        self.core.root = Some(view.clone().into());
        self.refresh();
        view
    }

    /// Returns the root entity of the window, if it has one.
    pub fn root<E>(&self) -> Option<Option<Entity<E>>>
    where
        E: 'static + Render,
    {
        self.core.root
            .as_ref()
            .map(|view| view.clone().downcast::<E>().ok())
    }

    /// Obtain a handle to the window that belongs to this context.
    pub fn window_handle(&self) -> AnyWindowHandle {
        self.core.handle
    }

    /// Mark the window as dirty, scheduling it to be redrawn on the next frame.
    pub fn refresh(&mut self) {
        if self.core.invalidator.not_drawing() {
            self.core.refreshing = true;
            self.core.invalidator.set_dirty(true);
        }
    }

    /// Close this window.
    pub fn remove_window(&mut self) {
        self.core.removed = true;
    }

    /// Obtain the currently focused [`FocusHandle`]. If no elements are focused, returns `None`.
    pub fn focused(&self, cx: &App) -> Option<FocusHandle> {
        self.core.focus
            .and_then(|id| FocusHandle::for_id(id, &cx.focus_handles))
    }

    /// Move focus to the element associated with the given [`FocusHandle`].
    pub fn focus(&mut self, handle: &FocusHandle, cx: &mut App) {
        if !self.core.focus_enabled || self.core.focus == Some(handle.id) {
            return;
        }

        self.core.focus = Some(handle.id);
        self.clear_pending_keystrokes();

        // Avoid re-entrant entity updates by deferring observer notifications to the end of the
        // current effect cycle, and only for this window.
        let window_handle = self.core.handle;
        cx.defer(move |cx| {
            window_handle
                .update(cx, |_, window, cx| {
                    window.pending_input_changed(cx);
                })
                .ok();
        });

        self.refresh();
    }

    /// Remove focus from all elements within this context's window.
    pub fn blur(&mut self) {
        if !self.core.focus_enabled {
            return;
        }

        self.core.focus = None;
        self.refresh();
    }

    /// Blur the window and don't allow anything in it to be focused again.
    pub fn disable_focus(&mut self) {
        self.blur();
        self.core.focus_enabled = false;
    }

    /// Move focus to next tab stop.
    pub fn focus_next(&mut self, cx: &mut App) {
        if !self.core.focus_enabled {
            return;
        }

        if let Some(handle) = self.core.rendered_frame.tab_stops.next(self.core.focus.as_ref()) {
            self.focus(&handle, cx)
        }
    }

    /// Move focus to previous tab stop.
    pub fn focus_prev(&mut self, cx: &mut App) {
        if !self.core.focus_enabled {
            return;
        }

        if let Some(handle) = self.core.rendered_frame.tab_stops.prev(self.core.focus.as_ref()) {
            self.focus(&handle, cx)
        }
    }

    /// Accessor for the text system.
    pub fn text_system(&self) -> &Arc<WindowTextSystem> {
        &self.core.text_system
    }

    /// The current text style. Which is composed of all the style refinements provided to `with_text_style`.
    pub fn text_style(&self) -> TextStyle {
        let mut style = TextStyle::default();
        for refinement in &self.core.text_style_stack {
            style.refine(refinement);
        }
        style
    }

    /// Check if the platform window is maximized.
    ///
    /// On some platforms (namely Windows) this is different than the bounds being the size of the display
    pub fn is_maximized(&self) -> bool {
        self.core.platform_window.is_maximized()
    }

    /// request a certain window decoration (Wayland)
    pub fn request_decorations(&self, decorations: WindowDecorations) {
        self.core.platform_window.request_decorations(decorations);
    }

    /// Start a window resize operation (Wayland)
    ///
    /// ADR-008 decision 6: gated on the `WindowOptions::is_resizable`
    /// invariant. A non-resizable window must reject resize gestures
    /// from any source — same contract as `minimize_window` below.
    pub fn start_window_resize(&self, edge: ResizeEdge) {
        if !self.core.is_resizable {
            log::warn!(
                "ADR-008: ignored programmatic start_window_resize() on a \
                 window created with `is_resizable = false`."
            );
            return;
        }
        self.core.platform_window.start_window_resize(edge);
    }

    /// Return the `WindowBounds` to indicate that how a window should be opened
    /// after it has been closed
    pub fn window_bounds(&self) -> WindowBounds {
        self.core.platform_window.window_bounds()
    }

    /// Return the `WindowBounds` excluding insets (Wayland and X11)
    pub fn inner_window_bounds(&self) -> WindowBounds {
        self.core.platform_window.inner_window_bounds()
    }

    /// Dispatch the given action on the currently focused element.
    pub fn dispatch_action(&mut self, action: Box<dyn Action>, cx: &mut App) {
        let focus_id = self.focused(cx).map(|handle| handle.id);

        let window = self.core.handle;
        cx.defer(move |cx| {
            window
                .update(cx, |_, window, cx| {
                    let node_id = window.focus_node_id_in_rendered_frame(focus_id);
                    window.dispatch_action_on_node(node_id, action.as_ref(), cx);
                })
                .log_err();
        })
    }

    pub(crate) fn dispatch_keystroke_observers(
        &mut self,
        event: &dyn Any,
        action: Option<Box<dyn Action>>,
        context_stack: Vec<KeyContext>,
        cx: &mut App,
    ) {
        let Some(key_down_event) = event.downcast_ref::<KeyDownEvent>() else {
            return;
        };

        cx.keystroke_observers.clone().retain(&(), move |callback| {
            (callback)(
                &KeystrokeEvent {
                    keystroke: key_down_event.keystroke.clone(),
                    action: action.as_ref().map(|action| action.boxed_clone()),
                    context_stack: context_stack.clone(),
                },
                self,
                cx,
            )
        });
    }

    pub(crate) fn dispatch_keystroke_interceptors(
        &mut self,
        event: &dyn Any,
        context_stack: Vec<KeyContext>,
        cx: &mut App,
    ) {
        let Some(key_down_event) = event.downcast_ref::<KeyDownEvent>() else {
            return;
        };

        cx.keystroke_interceptors
            .clone()
            .retain(&(), move |callback| {
                (callback)(
                    &KeystrokeEvent {
                        keystroke: key_down_event.keystroke.clone(),
                        action: None,
                        context_stack: context_stack.clone(),
                    },
                    self,
                    cx,
                )
            });
    }

    /// Schedules the given function to be run at the end of the current effect cycle, allowing entities
    /// that are currently on the stack to be returned to the app.
    ///
    /// Routes through K04 [`DeferPlacement::EndOfUpdate`] — the placement that
    /// drains at every phase boundary. Callers that want a specific later phase
    /// should use [`Window::defer_to`] instead.
    ///
    /// [`DeferPlacement::EndOfUpdate`]: crate::frame::DeferPlacement::EndOfUpdate
    pub fn defer(&self, cx: &mut App, f: impl FnOnce(&mut Window, &mut App) + 'static) {
        let handle = self.core.handle;
        cx.defer(move |cx| {
            handle.update(cx, |_, window, cx| f(window, cx)).ok();
        });
    }

    /// K04 placement-aware deferred callback (window-scoped).
    ///
    /// Schedules `f` to run at the matching phase boundary against this
    /// window via [`App::defer_to`] under the hood; the window handle is
    /// captured so the callback runs only if the window is still alive at
    /// drain time. See [`App::defer_to`] for the per-placement drain
    /// semantics — `flush_effects_at` filters by `placement` via
    /// [`FlushScope::admits`].
    pub fn defer_to(
        &self,
        cx: &mut App,
        placement: crate::frame::DeferPlacement,
        f: impl FnOnce(&mut Window, &mut App) + 'static,
    ) {
        let handle = self.core.handle;
        cx.defer_to(placement, move |cx| {
            handle.update(cx, |_, window, cx| f(window, cx)).ok();
        });
    }

    /// Subscribe to events emitted by a entity.
    /// The entity to which you're subscribing must implement the [`EventEmitter`] trait.
    /// The callback will be invoked a handle to the emitting entity, the event, and a window context for the current window.
    pub fn observe<T: 'static>(
        &mut self,
        observed: &Entity<T>,
        cx: &mut App,
        mut on_notify: impl FnMut(Entity<T>, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        let entity_id = observed.entity_id();
        let observed = observed.downgrade();
        let window_handle = self.core.handle;
        cx.new_observer(
            entity_id,
            Box::new(move |cx| {
                window_handle
                    .update(cx, |_, window, cx| {
                        if let Some(handle) = observed.upgrade() {
                            on_notify(handle, window, cx);
                            true
                        } else {
                            false
                        }
                    })
                    .unwrap_or(false)
            }),
        )
    }

    /// Subscribe to events emitted by a entity.
    /// The entity to which you're subscribing must implement the [`EventEmitter`] trait.
    /// The callback will be invoked a handle to the emitting entity, the event, and a window context for the current window.
    pub fn subscribe<Emitter, Evt>(
        &mut self,
        entity: &Entity<Emitter>,
        cx: &mut App,
        mut on_event: impl FnMut(Entity<Emitter>, &Evt, &mut Window, &mut App) + 'static,
    ) -> Subscription
    where
        Emitter: EventEmitter<Evt>,
        Evt: 'static,
    {
        let entity_id = entity.entity_id();
        let handle = entity.downgrade();
        let window_handle = self.core.handle;
        cx.new_subscription(
            entity_id,
            (
                TypeId::of::<Evt>(),
                Box::new(move |event, cx| {
                    window_handle
                        .update(cx, |_, window, cx| {
                            if let Some(entity) = handle.upgrade() {
                                let event = event.downcast_ref().expect("invalid event type");
                                on_event(entity, event, window, cx);
                                true
                            } else {
                                false
                            }
                        })
                        .unwrap_or(false)
                }),
            ),
        )
    }

    /// Register a callback to be invoked when the given `Entity` is released.
    pub fn observe_release<T>(
        &self,
        entity: &Entity<T>,
        cx: &mut App,
        mut on_release: impl FnOnce(&mut T, &mut Window, &mut App) + 'static,
    ) -> Subscription
    where
        T: 'static,
    {
        let entity_id = entity.entity_id();
        let window_handle = self.core.handle;
        let (subscription, activate) = cx.release_listeners.insert(
            entity_id,
            Box::new(move |entity, cx| {
                let entity = entity.downcast_mut().expect("invalid entity type");
                let _ = window_handle.update(cx, |_, window, cx| on_release(entity, window, cx));
            }),
        );
        activate();
        subscription
    }

    /// Creates an [`AsyncWindowContext`], which has a static lifetime and can be held across
    /// await points in async code.
    pub fn to_async(&self, cx: &App) -> AsyncWindowContext {
        AsyncWindowContext::new_context(cx.to_async(), self.core.handle)
    }

    /// K04 review fix #4: returns an opaque
    /// [`FrameClockView`](crate::frame::FrameClockView) snapshot of the
    /// App-wide [`FrameClock`](crate::frame::FrameClock).
    ///
    /// Today the snapshot always reflects the App-level clock — the
    /// indirection exists so a future R-track / Wasm spec can introduce
    /// per-window epoch divergence (tab visibility, iOS background scene)
    /// without a SemVer break.
    ///
    /// Call from inside a frame (e.g. layout, prepaint, paint, or a
    /// `Window::on_pre_frame` / `on_post_frame` callback) to read the
    /// per-frame `Instant`, `frame_index`, and `delta`.
    pub fn frame_clock_view(&self, cx: &App) -> crate::frame::FrameClockView {
        cx.frame_clock().view()
    }

    /// K04 Task 33: schedule the given closure to be run at the start of the
    /// next frame's [`PreFrame`](crate::frame::FramePhase::PreFrame) phase —
    /// BEFORE `window.draw()` paints the next frame.
    ///
    /// Use this for layout-affecting work that wants the upcoming frame's
    /// `FrameClock::now()` (axiom P3) but must run before paint (e.g. seeding
    /// a scroll position, finalizing a deferred focus change).
    ///
    /// For work that wants to observe the painted scene before running
    /// (telemetry export, inspector readout, post-frame settle), use
    /// [`Self::on_post_frame`] (K04 Task 34) instead.
    ///
    /// # Deprecated alias
    ///
    /// [`Self::on_next_frame`] continues to forward to this method with a
    /// `#[deprecated]` warning until the K04+1 release cycle removes it.
    ///
    /// # ADR-001 contract
    ///
    /// Must not be called during `Paint`. The callback observes the next
    /// frame; registering it from `paint` produces the same Wayland resize
    /// artefact as upstream GPUI #56294. Call from layout, an event handler,
    /// or a deferred effect instead. The deprecated `on_next_frame` alias
    /// inherits the same guard through this forward. See
    /// `docs/research/adr/ADR-001-invalidation-scope.md`.
    #[track_caller]
    pub fn on_pre_frame(&self, callback: impl FnOnce(&mut Window, &mut App) + 'static) {
        self.core.invalidator.debug_assert_not_paint();
        RefCell::borrow_mut(&self.core.next_frame_callbacks).push(Box::new(callback));
    }

    /// K04 Task 33: deprecated alias for [`Self::on_pre_frame`]. The name
    /// `on_next_frame` was misleading — callbacks fire BEFORE the next
    /// frame's draw, not after. Use `on_pre_frame` going forward.
    #[deprecated(
        since = "0.1.0",
        note = "renamed to `on_pre_frame` (K04) — the callback fires before the next frame's draw; this alias is scheduled for removal in 0.2.0"
    )]
    pub fn on_next_frame(&self, callback: impl FnOnce(&mut Window, &mut App) + 'static) {
        self.on_pre_frame(callback);
    }

    /// K04 Task 34: schedule the given closure to run in the current frame's
    /// [`PostFrame`](crate::frame::FramePhase::PostFrame) phase — AFTER
    /// `window.draw()` has produced the scene and `window.complete_frame()`
    /// has fired.
    ///
    /// Use this for work that needs to observe the resolved layout / painted
    /// scene before running: telemetry export, inspector readout, future
    /// post-frame settle (Flutter's `addPostFrameCallback` analogue).
    ///
    /// # K04 contract
    ///
    /// Per axiom P5, callbacks scheduled via this API MUST NOT mutate
    /// elements directly. To mutate, queue via
    /// `cx.defer_to(DeferPlacement::NextFrameStart, ...)` instead — the
    /// PostFrame phase is read-only for the in-flight frame's scene state.
    pub fn on_post_frame(&self, callback: impl FnOnce(&mut Window, &mut App) + 'static) {
        RefCell::borrow_mut(&self.core.post_frame_callbacks).push(Box::new(callback));
    }

    /// Schedule a frame to be drawn on the next animation frame.
    ///
    /// If called from within a view, notifies that view on the next frame.
    /// Otherwise, refreshes the entire window.
    ///
    /// # K04 review fix #6: per-view notify restored
    ///
    /// An earlier K04 iteration replaced the per-view notify closure with a
    /// `Cell<bool>` flag that dirtied the entire window invalidator. That
    /// regressed multi-view windows: every sibling view re-rendered on
    /// every animation frame, not just the animating one. This method now
    /// pushes a per-view notify closure (pre-K04 behavior) AND sets the
    /// idempotence flag, so:
    ///
    /// - per-view granularity is preserved (the closure's `cx.notify(entity)`
    ///   dedups via `App::pending_notifications`),
    /// - multiple calls in the same frame coalesce at the App-level
    ///   dedup, not at this method's storage,
    /// - the flag still drains in `App::run_frame`'s PreFrame phase as a
    ///   defense-in-depth invalidator mark for callers that hit this method
    ///   outside a view context.
    pub fn request_animation_frame(&self) {
        // Inside a view-rendering context (paint / prepaint / render),
        // `try_current_view` returns the active view's `EntityId` and we
        // schedule a per-view notify for next frame. Outside that context
        // (e.g. called from a `cx.spawn` task, an `on_post_frame` callback,
        // or a non-view async path), `try_current_view` is `None` and we
        // fall back to dirtying the entire window invalidator. Without the
        // fallback, `current_view()` would assert and panic.
        if let Some(entity) = self.try_current_view() {
            self.on_pre_frame(move |_, cx| cx.notify(entity));
        } else {
            self.core.invalidator.set_dirty(true);
        }
        // Idempotence flag — defense-in-depth for callers outside a view.
        self.core.request_next_frame.set(true);
    }

    /// Spawn the future returned by the given closure on the application thread pool.
    /// The closure is provided a handle to the current window and an `AsyncWindowContext` for
    /// use within your future.
    #[track_caller]
    pub fn spawn<AsyncFn, R>(&self, cx: &App, f: AsyncFn) -> Task<R>
    where
        R: 'static,
        AsyncFn: AsyncFnOnce(&mut AsyncWindowContext) -> R + 'static,
    {
        let handle = self.core.handle;
        cx.spawn(async move |app| {
            let mut async_window_cx = AsyncWindowContext::new_context(app.clone(), handle);
            f(&mut async_window_cx).await
        })
    }

    /// Spawn the future returned by the given closure on the application thread
    /// pool, with the given priority. The closure is provided a handle to the
    /// current window and an `AsyncWindowContext` for use within your future.
    #[track_caller]
    pub fn spawn_with_priority<AsyncFn, R>(
        &self,
        priority: Priority,
        cx: &App,
        f: AsyncFn,
    ) -> Task<R>
    where
        R: 'static,
        AsyncFn: AsyncFnOnce(&mut AsyncWindowContext) -> R + 'static,
    {
        let handle = self.core.handle;
        cx.spawn_with_priority(priority, async move |app| {
            let mut async_window_cx = AsyncWindowContext::new_context(app.clone(), handle);
            f(&mut async_window_cx).await
        })
    }

    /// Notify the window that its bounds have changed.
    ///
    /// This updates internal state like `viewport_size` and `scale_factor` from
    /// the platform window, then notifies observers. Normally called automatically
    /// by the platform's resize callback, but exposed publicly for test infrastructure.
    ///
    /// ADR-007: when the window's `display_id` or `scale_factor` changes,
    /// `display_change_observers` fires *in addition to* `bounds_observers`.
    /// The latter is kept window-size-focused; the former is the dedicated
    /// "external display state changed for this window" hook.
    pub fn bounds_changed(&mut self, cx: &mut App) {
        let prev_scale_factor = self.core.scale_factor;
        let prev_display_id = self.core.display_id;

        self.core.scale_factor = self.core.platform_window.scale_factor();
        self.core.viewport_size = self.core.platform_window.content_size();
        self.core.display_id = self.core.platform_window.display().map(|display| display.id());

        self.refresh();

        let display_changed = self.core.display_id != prev_display_id
            || (self.core.scale_factor - prev_scale_factor).abs() > f32::EPSILON;

        self.core.bounds_observers
            .clone()
            .retain(&(), |callback| callback(self, cx));

        if display_changed {
            self.core.display_change_observers
                .clone()
                .retain(&(), |callback| callback(self, cx));
        }
    }

    /// Returns the bounds of the current window in the global coordinate space, which could span across multiple displays.
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.core.platform_window.bounds()
    }

    /// Renders the current frame's scene to a texture and returns the pixel data as an RGBA image.
    /// This does not present the frame to screen - useful for visual testing where we want
    /// to capture what would be rendered without displaying it or requiring the window to be visible.
    #[cfg(any(test, feature = "test-support"))]
    pub fn render_to_image(&self) -> anyhow::Result<image::RgbaImage> {
        self.core.platform_window
            .render_to_image(&self.core.rendered_frame.scene)
    }

    /// Set the content size of the window.
    pub fn resize(&mut self, size: Size<Pixels>) {
        self.core.platform_window.resize(size);
    }

    /// Returns whether or not the window is currently fullscreen
    pub fn is_fullscreen(&self) -> bool {
        self.core.platform_window.is_fullscreen()
    }

    pub(crate) fn appearance_changed(&mut self, cx: &mut App) {
        self.core.appearance = self.core.platform_window.appearance();

        self.core.appearance_observers
            .clone()
            .retain(&(), |callback| callback(self, cx));
    }

    /// Returns the appearance of the current window.
    pub fn appearance(&self) -> WindowAppearance {
        self.core.appearance
    }

    /// Returns the size of the drawable area within the window.
    pub fn viewport_size(&self) -> Size<Pixels> {
        self.core.viewport_size
    }

    /// Returns whether this window is focused by the operating system (receiving key events).
    pub fn is_window_active(&self) -> bool {
        self.core.active.get()
    }

    /// Returns whether this window is considered to be the window
    /// that currently owns the mouse cursor.
    /// On mac, this is equivalent to `is_window_active`.
    pub fn is_window_hovered(&self) -> bool {
        if cfg!(any(
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd"
        )) {
            self.core.hovered.get()
        } else {
            self.is_window_active()
        }
    }

    /// Toggle zoom on the window.
    ///
    /// ADR-008 decision 3: a non-resizable window is also non-
    /// maximizable (Cocoa conflates resize + maximize via
    /// `NSResizableWindowMask`; Win32 via `WS_THICKFRAME`). This gate
    /// matches `start_window_resize` so callers cannot bypass the
    /// invariant through the zoom path.
    pub fn zoom_window(&self) {
        if !self.core.is_resizable {
            log::warn!(
                "ADR-008: ignored programmatic zoom_window() on a window \
                 created with `is_resizable = false`."
            );
            return;
        }
        self.core.platform_window.zoom();
    }

    /// Opens the native title bar context menu, useful when implementing client side decorations (Wayland and X11)
    pub fn show_window_menu(&self, position: Point<Pixels>) {
        self.core.platform_window.show_window_menu(position)
    }

    /// Handle window movement for Linux and macOS.
    /// Tells the compositor to take control of window movement (Wayland and X11)
    ///
    /// Events may not be received during a move operation.
    ///
    /// ADR-008 decision 6: gated on the `WindowOptions::is_movable`
    /// invariant. A non-movable window must reject drag-to-move
    /// gestures from any source — including this programmatic
    /// compositor handoff.
    pub fn start_window_move(&self) {
        if !self.core.is_movable {
            log::warn!(
                "ADR-008: ignored programmatic start_window_move() on a \
                 window created with `is_movable = false`."
            );
            return;
        }
        self.core.platform_window.start_window_move()
    }

    /// When using client side decorations, set this to the width of the invisible decorations (Wayland and X11)
    pub fn set_client_inset(&mut self, inset: Pixels) {
        self.core.client_inset = Some(inset);
        self.core.platform_window.set_client_inset(inset);
    }

    /// Returns the client_inset value by [`Self::set_client_inset`].
    pub fn client_inset(&self) -> Option<Pixels> {
        self.core.client_inset
    }

    /// Returns whether the title bar window controls need to be rendered by the application (Wayland and X11)
    pub fn window_decorations(&self) -> Decorations {
        self.core.platform_window.window_decorations()
    }

    /// Returns which window controls are currently visible (Wayland)
    pub fn window_controls(&self) -> WindowControls {
        self.core.platform_window.window_controls()
    }

    /// Updates the window's title at the platform level.
    pub fn set_window_title(&mut self, title: &str) {
        self.core.platform_window.set_title(title);
    }

    /// Sets the application identifier.
    pub fn set_app_id(&mut self, app_id: &str) {
        self.core.platform_window.set_app_id(app_id);
    }

    /// Sets the window background appearance.
    pub fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance) {
        self.core.platform_window
            .set_background_appearance(background_appearance);
    }

    /// Mark the window as dirty at the platform level.
    pub fn set_window_edited(&mut self, edited: bool) {
        self.core.platform_window.set_edited(edited);
    }

    /// Determine the display on which the window is visible.
    pub fn display(&self, cx: &App) -> Option<Rc<dyn PlatformDisplay>> {
        cx.platform
            .displays()
            .into_iter()
            .find(|display| Some(display.id()) == self.core.display_id)
    }

    /// Show the platform character palette.
    pub fn show_character_palette(&self) {
        self.core.platform_window.show_character_palette();
    }

    /// The scale factor of the display associated with the window. For example, it could
    /// return 2.0 for a "retina" display, indicating that each logical pixel should actually
    /// be rendered as two pixels on screen.
    pub fn scale_factor(&self) -> f32 {
        self.core.scale_factor
    }

    /// Get aggregated media query data for this window.
    pub fn media_query(&self, cx: &App) -> MediaQueryData {
        MediaQueryData {
            size: self.bounds().size,
            scale_factor: self.scale_factor(),
            brightness: cx.platform_brightness(),
            text_scale_factor: 1.0, // TODO: detect from OS
        }
    }

    /// The size of an em for the base font of the application. Adjusting this value allows the
    /// UI to scale, just like zooming a web page.
    pub fn rem_size(&self) -> Pixels {
        self.core.rem_size_override_stack
            .last()
            .copied()
            .unwrap_or(self.core.rem_size)
    }

    /// Sets the size of an em for the base font of the application. Adjusting this value allows the
    /// UI to scale, just like zooming a web page.
    pub fn set_rem_size(&mut self, rem_size: impl Into<Pixels>) {
        self.core.rem_size = rem_size.into();
    }

    /// Acquire a globally unique identifier for the given ElementId.
    /// Only valid for the duration of the provided closure.
    pub fn with_global_id<R>(
        &mut self,
        element_id: impl Into<ElementId>,
        f: impl FnOnce(&GlobalElementId, &mut Self) -> R,
    ) -> R {
        self.with_pushed_element_id(element_id, f)
    }

    pub(crate) fn with_pushed_element_id<R>(
        &mut self,
        element_id: impl Into<ElementId>,
        f: impl FnOnce(&GlobalElementId, &mut Self) -> R,
    ) -> R {
        self.core.element_id_stack.push(element_id.into());
        let global_id = GlobalElementId(Arc::from(&*self.core.element_id_stack));
        self.with_current_element_id_scope(|this| f(&global_id, this))
    }

    pub(crate) fn with_resolved_element_id<R>(
        &mut self,
        global_id: &GlobalElementId,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let parent_len = self.core.element_id_stack.len();
        assert!(
            global_id.len() > parent_len && &global_id[..parent_len] == &*self.core.element_id_stack,
            "stored global id must extend the current element path"
        );
        let element_id = global_id
            .get(parent_len)
            .expect("stored global id must contain the current element segment")
            .clone();
        self.core.element_id_stack.push_resolved(element_id);
        self.with_current_element_id_scope(f)
    }

    fn with_current_element_id_scope<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let result = panic::catch_unwind(AssertUnwindSafe(|| f(self)));
        let popped = self.core.element_id_stack.pop();
        assert!(
            popped.is_some(),
            "element identity scope ended with an empty stack"
        );
        match result {
            Ok(result) => result,
            Err(payload) => panic::resume_unwind(payload),
        }
    }

    /// Calls the provided closure with the element ID pushed on the stack.
    #[inline]
    pub fn with_id<R>(
        &mut self,
        element_id: impl Into<ElementId>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.with_pushed_element_id(element_id, |_, this| f(this))
    }

    /// Executes the provided function with the specified rem size.
    ///
    /// This method must only be called as part of element drawing.
    // This function is called in a highly recursive manner in editor
    // prepainting, make sure its inlined to reduce the stack burden
    #[inline]
    pub fn with_rem_size<F, R>(&mut self, rem_size: Option<impl Into<Pixels>>, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.core.invalidator.debug_assert_paint_or_prepaint();

        if let Some(rem_size) = rem_size {
            self.core.rem_size_override_stack.push(rem_size.into());
            let result = f(self);
            self.core.rem_size_override_stack.pop();
            result
        } else {
            f(self)
        }
    }

    /// The line height associated with the current text style.
    pub fn line_height(&self) -> Pixels {
        self.text_style().line_height_in_pixels(self.rem_size())
    }

    /// Call to prevent the default action of an event. Currently only used to prevent
    /// parent elements from becoming focused on mouse down.
    pub fn prevent_default(&mut self) {
        self.core.default_prevented = true;
    }

    /// Obtain whether default has been prevented for the event currently being dispatched.
    pub fn default_prevented(&self) -> bool {
        self.core.default_prevented
    }

    /// Determine whether the given action is available along the dispatch path to the currently focused element.
    pub fn is_action_available(&self, action: &dyn Action, cx: &App) -> bool {
        let node_id =
            self.focus_node_id_in_rendered_frame(self.focused(cx).map(|handle| handle.id));
        self.core.rendered_frame
            .dispatch_tree
            .is_action_available(action, node_id)
    }

    /// Determine whether the given action is available along the dispatch path to the given focus_handle.
    pub fn is_action_available_in(&self, action: &dyn Action, focus_handle: &FocusHandle) -> bool {
        let node_id = self.focus_node_id_in_rendered_frame(Some(focus_handle.id));
        self.core.rendered_frame
            .dispatch_tree
            .is_action_available(action, node_id)
    }

    /// The position of the mouse relative to the window.
    pub fn mouse_position(&self) -> Point<Pixels> {
        self.core.mouse_position
    }

    /// Borrow the per-`Window` gesture binding (arena + settings +
    /// sanitizer). See [`crate::gesture::GestureBinding`].
    pub fn gesture_binding(&self) -> &crate::gesture::GestureBinding {
        &self.core.gesture_binding
    }

    /// Mutably borrow the per-`Window` gesture binding.
    pub fn gesture_binding_mut(&mut self) -> &mut crate::gesture::GestureBinding {
        &mut self.core.gesture_binding
    }

    /// Mutate gesture thresholds. The S14 MediaQuery seam — when
    /// `MediaQueryData::gesture_settings` lands, it routes through
    /// this accessor.
    pub fn gesture_settings_mut(&mut self) -> &mut crate::gesture::GestureSettings {
        self.core.gesture_binding.settings_mut()
    }

    /// Compute a typed `HitTestResult` for `position` against the
    /// rendered frame, paired with each entry's `HitTestBehavior`
    /// from the per-frame `hit_test_behaviors` map.
    ///
    /// Always honours `position`: walks `rendered_frame.hit_test(position)`
    /// rather than relying on the cached `mouse_hit_test` (which is
    /// only fresh for the current `mouse_position` and is updated
    /// downstream inside `dispatch_mouse_event`). This matters in the
    /// gesture pass that runs **before** `dispatch_mouse_event`:
    /// recognizers receive a `HitTestResult` for the actual event
    /// position, not for the previous frame's mouse position
    /// (Copilot review A).
    ///
    /// Cost: O(n) over hitboxes in the rendered frame. Spatial
    /// indexing (BVH/quadtree) is deferred to a P-track perf
    /// milestone — current linear scan over `SmallVec<[HitboxId; 8]>`
    /// is sufficient for trees of up to ~16 hitboxes typical of
    /// `flui-core` consumers.
    pub fn hit_test(&self, position: Point<Pixels>) -> crate::gesture::HitTestResult {
        let mut result = crate::gesture::HitTestResult::default();
        // S07.5b: open a single identity scope so the transform-stack
        // path is exercised on every hit-test pass. S09 will replace
        // this with a per-paint-layer push driven by the rendered
        // frame's transform tree.
        let mut scope = result.push_transform(crate::Affine2::IDENTITY);
        let frame_hit = self.core.rendered_frame.hit_test(position);
        for &hitbox_id in frame_hit.ids.iter() {
            let behavior = self
                .core.hit_test_behaviors
                .get(&hitbox_id)
                .copied()
                .unwrap_or(crate::gesture::HitTestBehavior::Opaque);
            // SmallVec inline storage is 8; pushing more than 8 hits
            // a one-time heap allocation that we accept for the rare
            // very-deep-nesting case. T22 bench enforces the
            // `<2µs/query` budget in the realistic 8-deep case.
            //
            // `HitTestEntry` is `#[non_exhaustive]` for downstream
            // consumers; same-crate code can use the struct literal.
            scope.add(crate::gesture::HitTestEntry {
                hitbox_id,
                position,
                behavior,
                transform: None,
            });
        }
        drop(scope);
        result
    }

    /// Captures the pointer for the given hitbox. While captured, all mouse move and mouse up
    /// events will be routed to listeners that check this hitbox's `is_hovered` status,
    /// regardless of actual hit testing. This enables drag operations that continue
    /// even when the pointer moves outside the element's bounds.
    ///
    /// The capture is automatically released on mouse up.
    pub fn capture_pointer(&mut self, hitbox_id: HitboxId) {
        self.core.captured_hitbox = Some(hitbox_id);
    }

    /// Releases any active pointer capture.
    pub fn release_pointer(&mut self) {
        self.core.captured_hitbox = None;
    }

    /// Returns the hitbox that has captured the pointer, if any.
    pub fn captured_hitbox(&self) -> Option<HitboxId> {
        self.core.captured_hitbox
    }

    /// The current state of the keyboard's modifiers
    pub fn modifiers(&self) -> Modifiers {
        self.core.modifiers
    }

    /// Returns true if the last input event was keyboard-based (key press, tab navigation, etc.)
    /// This is used for focus-visible styling to show focus indicators only for keyboard navigation.
    pub fn last_input_was_keyboard(&self) -> bool {
        self.core.last_input_modality == InputModality::Keyboard
    }

    /// The current state of the keyboard's capslock
    pub fn capslock(&self) -> Capslock {
        self.core.capslock
    }

    fn complete_frame(&self) {
        self.core.platform_window.completed_frame();
    }

    /// Produces a new frame and assigns it to `rendered_frame`. To actually show
    /// the contents of the new [`Scene`], use `Self::present`.
    #[profiling::function]
    pub fn draw(&mut self, cx: &mut App) -> ArenaClearNeeded {
        // Set up the per-App arena for element allocation during this draw.
        // This ensures that multiple test Apps have isolated arenas.
        let _arena_scope = ElementArenaScope::enter(&cx.element_arena);

        self.invalidate_entities();
        cx.entities.clear_accessed();
        debug_assert!(self.core.rendered_entity_stack.is_empty());
        self.core.invalidator.set_dirty(false);
        self.core.requested_autoscroll = None;

        // Clear the per-frame `HitTestBehavior` map before painting
        // refills it from `Interactivity::paint` (T14). Without this,
        // entries from previous frames accumulate forever and stale
        // behaviors leak across hitbox reuse — Copilot review B.
        self.core.hit_test_behaviors.clear();
        // Same lifecycle for `pending_recognizers` (T15) — paint
        // refills the map with that frame's gesture recognizers; the
        // dispatcher drains them on `PointerPhase::Down`.
        self.core.pending_recognizers.clear();

        // Restore the previously-used input handler.
        if let Some(input_handler) = self.core.platform_window.take_input_handler() {
            self.core.rendered_frame.input_handlers.push(Some(input_handler));
        }
        if !cx.mode.skip_drawing() {
            self.core.inherited_registry.begin_frame();
            self.draw_roots(cx);
            let dirty_views = self.core.inherited_registry.remove_unaccessed_providers();
            self.invalidate_inherited_dependents(dirty_views, cx);
        }
        self.core.dirty_views.clear();
        self.core.next_frame.window_active = self.core.active.get();

        // Register requested input handler with the platform window.
        if let Some(input_handler) = self.core.next_frame.input_handlers.pop() {
            self.core.platform_window
                .set_input_handler(input_handler.unwrap());
        }

        self.core.layout_engine.as_mut().unwrap().clear();
        self.text_system().finish_frame();
        self.core.next_frame.finish(&mut self.core.rendered_frame);

        self.core.invalidator.set_phase(DrawPhase::Focus);
        let previous_focus_path = self.core.rendered_frame.focus_path();
        let previous_window_active = self.core.rendered_frame.window_active;
        mem::swap(&mut self.core.rendered_frame, &mut self.core.next_frame);
        self.core.next_frame.clear();
        let current_focus_path = self.core.rendered_frame.focus_path();
        let current_window_active = self.core.rendered_frame.window_active;

        if previous_focus_path != current_focus_path
            || previous_window_active != current_window_active
        {
            if !previous_focus_path.is_empty() && current_focus_path.is_empty() {
                self.core.focus_lost_listeners
                    .clone()
                    .retain(&(), |listener| listener(self, cx));
            }

            let event = WindowFocusEvent {
                previous_focus_path: if previous_window_active {
                    previous_focus_path
                } else {
                    Default::default()
                },
                current_focus_path: if current_window_active {
                    current_focus_path
                } else {
                    Default::default()
                },
            };
            self.core.focus_listeners
                .clone()
                .retain(&(), |listener| listener(&event, self, cx));
        }

        debug_assert!(self.core.rendered_entity_stack.is_empty());
        self.record_entities_accessed(cx);
        self.reset_cursor_style(cx);
        self.core.refreshing = false;
        self.core.invalidator.set_phase(DrawPhase::None);
        self.core.needs_present.set(true);

        ArenaClearNeeded::new(&cx.element_arena)
    }

    fn record_entities_accessed(&mut self, cx: &mut App) {
        let mut entities_ref = cx.entities.accessed_entities.get_mut();
        let mut entities = mem::take(entities_ref.deref_mut());
        let handle = self.core.handle;
        cx.record_entities_accessed(
            handle,
            // Try moving window invalidator into the Window
            self.core.invalidator.clone(),
            &entities,
        );
        let mut entities_ref = cx.entities.accessed_entities.get_mut();
        mem::swap(&mut entities, entities_ref.deref_mut());
    }

    fn invalidate_entities(&mut self) {
        let mut views = self.core.invalidator.take_views();
        for entity in views.drain() {
            self.mark_view_dirty(entity);
        }
        self.core.invalidator.replace_views(views);
    }

    #[profiling::function]
    fn present(&self) {
        self.core.platform_window.draw(&self.core.rendered_frame.scene);
        self.core.needs_present.set(false);
        profiling::finish_frame!();
    }

    fn draw_roots(&mut self, cx: &mut App) {
        self.core.invalidator.set_phase(DrawPhase::Prepaint);
        self.core.tooltip_bounds.take();

        let _inspector_width: Pixels = rems(30.0).to_pixels(self.rem_size());
        let root_size = {
            #[cfg(any(feature = "inspector", debug_assertions))]
            {
                if self.core.inspector.is_some() {
                    let mut size = self.core.viewport_size;
                    size.width = (size.width - _inspector_width).max(px(0.0));
                    size
                } else {
                    self.core.viewport_size
                }
            }
            #[cfg(not(any(feature = "inspector", debug_assertions)))]
            {
                self.core.viewport_size
            }
        };

        // Layout all root elements.
        let mut root_element = self.core.root.as_ref().unwrap().clone().into_any();
        root_element.prepaint_as_root_with_window(Point::default(), root_size.into(), self, cx);

        #[cfg(any(feature = "inspector", debug_assertions))]
        let inspector_element = self.prepaint_inspector(_inspector_width, cx);

        self.prepaint_deferred_draws(cx);

        let mut prompt_element = None;
        let mut active_drag_element = None;
        let mut tooltip_element = None;
        if let Some(prompt) = self.core.prompt.take() {
            let mut element = prompt.view.any_view().into_any();
            element.prepaint_as_root_with_window(Point::default(), root_size.into(), self, cx);
            prompt_element = Some(element);
            self.core.prompt = Some(prompt);
        } else if let Some(active_drag) = cx.active_drag.take() {
            let mut element = active_drag.view.clone().into_any();
            let offset = self.mouse_position() - active_drag.cursor_offset;
            element.prepaint_as_root_with_window(offset, AvailableSpace::min_size(), self, cx);
            active_drag_element = Some(element);
            cx.active_drag = Some(active_drag);
        } else {
            tooltip_element = self.prepaint_tooltip(cx);
        }

        self.core.mouse_hit_test = self.core.next_frame.hit_test(self.core.mouse_position);

        // Now actually paint the elements.
        self.core.invalidator.set_phase(DrawPhase::Paint);
        root_element.paint_with_window(self, cx);

        #[cfg(any(feature = "inspector", debug_assertions))]
        self.paint_inspector(inspector_element, cx);

        self.paint_deferred_draws(cx);

        if let Some(mut prompt_element) = prompt_element {
            prompt_element.paint_with_window(self, cx);
        } else if let Some(mut drag_element) = active_drag_element {
            drag_element.paint_with_window(self, cx);
        } else if let Some(mut tooltip_element) = tooltip_element {
            tooltip_element.paint_with_window(self, cx);
        }

        #[cfg(any(feature = "inspector", debug_assertions))]
        self.paint_inspector_hitbox(cx);
    }

    fn prepaint_tooltip(&mut self, cx: &mut App) -> Option<AnyElement> {
        // Use indexing instead of iteration to avoid borrowing self for the duration of the loop.
        for tooltip_request_index in (0..self.core.next_frame.tooltip_requests.len()).rev() {
            let Some(Some(tooltip_request)) = self
                .core.next_frame
                .tooltip_requests
                .get(tooltip_request_index)
                .cloned()
            else {
                log::error!("Unexpectedly absent TooltipRequest");
                continue;
            };
            let mut element = tooltip_request.tooltip.view.clone().into_any();
            let mouse_position = tooltip_request.tooltip.mouse_position;
            let tooltip_size =
                element.layout_as_root_with_window(AvailableSpace::min_size(), self, cx);

            let mut tooltip_bounds =
                Bounds::new(mouse_position + point(px(1.), px(1.)), tooltip_size);
            let window_bounds = Bounds {
                origin: Point::default(),
                size: self.viewport_size(),
            };

            if tooltip_bounds.right() > window_bounds.right() {
                let new_x = mouse_position.x - tooltip_bounds.size.width - px(1.);
                if new_x >= Pixels::ZERO {
                    tooltip_bounds.origin.x = new_x;
                } else {
                    tooltip_bounds.origin.x = cmp::max(
                        Pixels::ZERO,
                        tooltip_bounds.origin.x - tooltip_bounds.right() - window_bounds.right(),
                    );
                }
            }

            if tooltip_bounds.bottom() > window_bounds.bottom() {
                let new_y = mouse_position.y - tooltip_bounds.size.height - px(1.);
                if new_y >= Pixels::ZERO {
                    tooltip_bounds.origin.y = new_y;
                } else {
                    tooltip_bounds.origin.y = cmp::max(
                        Pixels::ZERO,
                        tooltip_bounds.origin.y - tooltip_bounds.bottom() - window_bounds.bottom(),
                    );
                }
            }

            // It's possible for an element to have an active tooltip while not being painted (e.g.
            // via the `visible_on_hover` method). Since mouse listeners are not active in this
            // case, instead update the tooltip's visibility here.
            let is_visible =
                (tooltip_request.tooltip.check_visible_and_update)(tooltip_bounds, self, cx);
            if !is_visible {
                continue;
            }

            element.prepaint_at_with_window(tooltip_bounds.origin, self, cx);

            self.core.tooltip_bounds = Some(TooltipBounds {
                id: tooltip_request.id,
                bounds: tooltip_bounds,
            });
            return Some(element);
        }
        None
    }

    fn prepaint_deferred_draws(&mut self, cx: &mut App) {
        assert_eq!(self.core.element_id_stack.len(), 0);

        let mut completed_draws = Vec::new();

        // Process deferred draws in multiple rounds to support nesting.
        // Each round processes all current deferred draws, which may produce new ones.
        let mut depth = 0;
        loop {
            // Limit maximum nesting depth to prevent infinite loops.
            assert!(depth < 10, "Exceeded maximum (10) deferred depth");
            depth += 1;
            let deferred_count = self.core.next_frame.deferred_draws.len();
            if deferred_count == 0 {
                break;
            }

            // Sort by priority for this round
            let traversal_order = self.deferred_draw_traversal_order();
            let mut deferred_draws = mem::take(&mut self.core.next_frame.deferred_draws);

            for deferred_draw_ix in traversal_order {
                let deferred_draw = &mut deferred_draws[deferred_draw_ix];
                self.core.element_id_stack
                    .clone_from(&deferred_draw.element_id_stack);
                self.core.text_style_stack
                    .clone_from(&deferred_draw.text_style_stack);
                self.core.next_frame
                    .dispatch_tree
                    .set_active_node(deferred_draw.parent_node);

                let prepaint_start = self.prepaint_index();
                if let Some(element) = deferred_draw.element.as_mut() {
                    self.with_rendered_view(deferred_draw.current_view, |window| {
                        window.with_rem_size(Some(deferred_draw.rem_size), |window| {
                            window.with_absolute_element_offset(
                                deferred_draw.absolute_offset,
                                |window| {
                                    let mut element_cx = crate::PrepaintCx::new(
                                        window,
                                        cx,
                                        deferred_draw.global_id.as_ref(),
                                        deferred_draw.inspector_id.as_ref(),
                                        deferred_draw.bounds,
                                    );
                                    element.prepaint(&mut element_cx);
                                },
                            );
                        });
                    })
                } else {
                    self.reuse_prepaint(deferred_draw.prepaint_range.clone());
                }
                let prepaint_end = self.prepaint_index();
                deferred_draw.prepaint_range = prepaint_start..prepaint_end;
            }

            // Save completed draws and continue with newly added ones
            completed_draws.append(&mut deferred_draws);

            self.core.element_id_stack.clear();
            self.core.text_style_stack.clear();
        }

        // Restore all completed draws
        self.core.next_frame.deferred_draws = completed_draws;
    }

    fn paint_deferred_draws(&mut self, cx: &mut App) {
        assert_eq!(self.core.element_id_stack.len(), 0);

        // Paint all deferred draws in priority order.
        // Since prepaint has already processed nested deferreds, we just paint them all.
        if self.core.next_frame.deferred_draws.len() == 0 {
            return;
        }

        let traversal_order = self.deferred_draw_traversal_order();
        let mut deferred_draws = mem::take(&mut self.core.next_frame.deferred_draws);
        for deferred_draw_ix in traversal_order {
            let mut deferred_draw = &mut deferred_draws[deferred_draw_ix];
            self.core.element_id_stack
                .clone_from(&deferred_draw.element_id_stack);
            self.core.next_frame
                .dispatch_tree
                .set_active_node(deferred_draw.parent_node);

            let paint_start = self.paint_index();
            let content_mask = deferred_draw.content_mask.clone();
            if let Some(element) = deferred_draw.element.as_mut() {
                self.with_rendered_view(deferred_draw.current_view, |window| {
                    window.with_content_mask(content_mask, |window| {
                        window.with_rem_size(Some(deferred_draw.rem_size), |window| {
                            let mut element_cx = crate::PaintCx::new(
                                window,
                                cx,
                                deferred_draw.global_id.as_ref(),
                                deferred_draw.inspector_id.as_ref(),
                                deferred_draw.bounds,
                            );
                            element.paint(&mut element_cx);
                        });
                    })
                })
            } else {
                self.reuse_paint(deferred_draw.paint_range.clone());
            }
            let paint_end = self.paint_index();
            deferred_draw.paint_range = paint_start..paint_end;
        }
        self.core.next_frame.deferred_draws = deferred_draws;
        self.core.element_id_stack.clear();
    }

    fn deferred_draw_traversal_order(&mut self) -> SmallVec<[usize; 8]> {
        let deferred_count = self.core.next_frame.deferred_draws.len();
        let mut sorted_indices = (0..deferred_count).collect::<SmallVec<[_; 8]>>();
        sorted_indices.sort_by_key(|ix| self.core.next_frame.deferred_draws[*ix].priority);
        sorted_indices
    }

    pub(crate) fn prepaint_index(&self) -> PrepaintStateIndex {
        PrepaintStateIndex {
            hitboxes_index: self.core.next_frame.hitboxes.len(),
            tooltips_index: self.core.next_frame.tooltip_requests.len(),
            deferred_draws_index: self.core.next_frame.deferred_draws.len(),
            dispatch_tree_index: self.core.next_frame.dispatch_tree.len(),
            accessed_element_states_index: self.core.next_frame.accessed_element_states.len(),
            line_layout_index: self.core.text_system.layout_index(),
        }
    }

    pub(crate) fn reuse_prepaint(&mut self, range: Range<PrepaintStateIndex>) {
        self.core.next_frame.hitboxes.extend(
            self.core.rendered_frame.hitboxes[range.start.hitboxes_index..range.end.hitboxes_index]
                .iter()
                .cloned(),
        );
        self.core.next_frame.tooltip_requests.extend(
            self.core.rendered_frame.tooltip_requests
                [range.start.tooltips_index..range.end.tooltips_index]
                .iter_mut()
                .map(|request| request.take()),
        );
        self.core.next_frame.accessed_element_states.extend(
            self.core.rendered_frame.accessed_element_states[range.start.accessed_element_states_index
                ..range.end.accessed_element_states_index]
                .iter()
                .map(|(id, type_id)| (id.clone(), *type_id)),
        );
        self.core.text_system
            .reuse_layouts(range.start.line_layout_index..range.end.line_layout_index);

        let reused_subtree = self.core.next_frame.dispatch_tree.reuse_subtree(
            range.start.dispatch_tree_index..range.end.dispatch_tree_index,
            &mut self.core.rendered_frame.dispatch_tree,
            self.core.focus,
        );

        if reused_subtree.contains_focus() {
            self.core.next_frame.focus = self.core.focus;
        }

        self.core.next_frame.deferred_draws.extend(
            self.core.rendered_frame.deferred_draws
                [range.start.deferred_draws_index..range.end.deferred_draws_index]
                .iter()
                .map(|deferred_draw| DeferredDraw {
                    current_view: deferred_draw.current_view,
                    parent_node: reused_subtree.refresh_node_id(deferred_draw.parent_node),
                    global_id: deferred_draw.global_id.clone(),
                    inspector_id: deferred_draw.inspector_id.clone(),
                    bounds: deferred_draw.bounds,
                    element_id_stack: deferred_draw.element_id_stack.clone(),
                    text_style_stack: deferred_draw.text_style_stack.clone(),
                    content_mask: deferred_draw.content_mask.clone(),
                    rem_size: deferred_draw.rem_size,
                    priority: deferred_draw.priority,
                    element: None,
                    absolute_offset: deferred_draw.absolute_offset,
                    prepaint_range: deferred_draw.prepaint_range.clone(),
                    paint_range: deferred_draw.paint_range.clone(),
                }),
        );
    }

    pub(crate) fn paint_index(&self) -> PaintIndex {
        PaintIndex {
            scene_index: self.core.next_frame.scene.len(),
            mouse_listeners_index: self.core.next_frame.mouse_listeners.len(),
            input_handlers_index: self.core.next_frame.input_handlers.len(),
            cursor_styles_index: self.core.next_frame.cursor_styles.len(),
            accessed_element_states_index: self.core.next_frame.accessed_element_states.len(),
            tab_handle_index: self.core.next_frame.tab_stops.paint_index(),
            line_layout_index: self.core.text_system.layout_index(),
        }
    }

    pub(crate) fn reuse_paint(&mut self, range: Range<PaintIndex>) {
        self.core.next_frame.cursor_styles.extend(
            self.core.rendered_frame.cursor_styles
                [range.start.cursor_styles_index..range.end.cursor_styles_index]
                .iter()
                .cloned(),
        );
        self.core.next_frame.input_handlers.extend(
            self.core.rendered_frame.input_handlers
                [range.start.input_handlers_index..range.end.input_handlers_index]
                .iter_mut()
                .map(|handler| handler.take()),
        );
        self.core.next_frame.mouse_listeners.extend(
            self.core.rendered_frame.mouse_listeners
                [range.start.mouse_listeners_index..range.end.mouse_listeners_index]
                .iter_mut()
                .map(|listener| listener.take()),
        );
        self.core.next_frame.accessed_element_states.extend(
            self.core.rendered_frame.accessed_element_states[range.start.accessed_element_states_index
                ..range.end.accessed_element_states_index]
                .iter()
                .map(|(id, type_id)| (id.clone(), *type_id)),
        );
        self.core.next_frame.tab_stops.replay(
            &self.core.rendered_frame.tab_stops.insertion_history
                [range.start.tab_handle_index..range.end.tab_handle_index],
        );

        self.core.text_system
            .reuse_layouts(range.start.line_layout_index..range.end.line_layout_index);
        self.core.next_frame.scene.replay(
            range.start.scene_index..range.end.scene_index,
            &self.core.rendered_frame.scene,
        );
    }

    /// Push a text style onto the stack, and call a function with that style active.
    /// Use [`Window::text_style`] to get the current, combined text style. This method
    /// should only be called as part of element drawing.
    // This function is called in a highly recursive manner in editor
    // prepainting, make sure its inlined to reduce the stack burden
    #[inline]
    pub fn with_text_style<F, R>(&mut self, style: Option<TextStyleRefinement>, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.core.invalidator.debug_assert_paint_or_prepaint();
        if let Some(style) = style {
            self.core.text_style_stack.push(style);
            let result = f(self);
            self.core.text_style_stack.pop();
            result
        } else {
            f(self)
        }
    }

    /// Updates the cursor style at the platform level. This method should only be called
    /// during the paint phase of element drawing.
    pub fn set_cursor_style(&mut self, style: CursorStyle, hitbox: &Hitbox) {
        self.core.invalidator.debug_assert_paint();
        self.core.next_frame.cursor_styles.push(CursorStyleRequest {
            hitbox_id: Some(hitbox.id),
            style,
        });
    }

    /// Updates the cursor style for the entire window at the platform level. A cursor
    /// style using this method will have precedence over any cursor style set using
    /// `set_cursor_style`. This method should only be called during the paint
    /// phase of element drawing.
    pub fn set_window_cursor_style(&mut self, style: CursorStyle) {
        self.core.invalidator.debug_assert_paint();
        self.core.next_frame.cursor_styles.push(CursorStyleRequest {
            hitbox_id: None,
            style,
        })
    }

    /// Sets a tooltip to be rendered for the upcoming frame. This method should only be called
    /// during the paint phase of element drawing.
    pub fn set_tooltip(&mut self, tooltip: AnyTooltip) -> TooltipId {
        self.core.invalidator.debug_assert_prepaint();
        let id = TooltipId(post_inc(&mut self.core.next_tooltip_id.0));
        self.core.next_frame
            .tooltip_requests
            .push(Some(TooltipRequest { id, tooltip }));
        id
    }

    /// Invoke the given function with the given content mask after intersecting it
    /// with the current mask. This method should only be called during element drawing.
    // This function is called in a highly recursive manner in editor
    // prepainting, make sure its inlined to reduce the stack burden
    #[inline]
    pub fn with_content_mask<R>(
        &mut self,
        mask: Option<ContentMask<Pixels>>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.core.invalidator.debug_assert_paint_or_prepaint();
        if let Some(mask) = mask {
            let mask = mask.intersect(&self.content_mask());
            self.core.content_mask_stack.push(mask);
            let result = f(self);
            self.core.content_mask_stack.pop();
            result
        } else {
            f(self)
        }
    }

    /// Updates the global element offset relative to the current offset. This is used to implement
    /// scrolling. This method should only be called during the prepaint phase of element drawing.
    pub fn with_element_offset<R>(
        &mut self,
        offset: Point<Pixels>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.core.invalidator.debug_assert_prepaint();

        if offset.is_zero() {
            return f(self);
        };

        let abs_offset = self.element_offset() + offset;
        self.with_absolute_element_offset(abs_offset, f)
    }

    /// Updates the global element offset based on the given offset. This is used to implement
    /// drag handles and other manual painting of elements. This method should only be called during
    /// the prepaint phase of element drawing.
    pub fn with_absolute_element_offset<R>(
        &mut self,
        offset: Point<Pixels>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.core.invalidator.debug_assert_prepaint();
        self.core.element_offset_stack.push(offset);
        let result = f(self);
        self.core.element_offset_stack.pop();
        result
    }

    pub(crate) fn with_element_opacity<R>(
        &mut self,
        opacity: Option<f32>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.core.invalidator.debug_assert_paint_or_prepaint();

        let Some(opacity) = opacity else {
            return f(self);
        };

        let previous_opacity = self.core.element_opacity;
        self.core.element_opacity = previous_opacity * opacity;
        let result = f(self);
        self.core.element_opacity = previous_opacity;
        result
    }

    /// Perform prepaint on child elements in a "retryable" manner, so that any side effects
    /// of prepaints can be discarded before prepainting again. This is used to support autoscroll
    /// where we need to prepaint children to detect the autoscroll bounds, then adjust the
    /// element offset and prepaint again. See [`crate::List`] for an example. This method should only be
    /// called during the prepaint phase of element drawing.
    pub fn transact<T, U>(&mut self, f: impl FnOnce(&mut Self) -> Result<T, U>) -> Result<T, U> {
        self.core.invalidator.debug_assert_prepaint();
        let index = self.prepaint_index();
        let result = f(self);
        if result.is_err() {
            self.core.next_frame.hitboxes.truncate(index.hitboxes_index);
            self.core.next_frame
                .tooltip_requests
                .truncate(index.tooltips_index);
            self.core.next_frame
                .deferred_draws
                .truncate(index.deferred_draws_index);
            self.core.next_frame
                .dispatch_tree
                .truncate(index.dispatch_tree_index);
            self.core.next_frame
                .accessed_element_states
                .truncate(index.accessed_element_states_index);
            self.core.text_system.truncate_layouts(index.line_layout_index);
        }
        result
    }

    /// When you call this method during [`Element::prepaint`], containing elements will attempt to
    /// scroll to cause the specified bounds to become visible. When they decide to autoscroll, they will call
    /// [`Element::prepaint`] again with a new set of bounds. See [`crate::List`] for an example of an element
    /// that supports this method being called on the elements it contains. This method should only be
    /// called during the prepaint phase of element drawing.
    pub fn request_autoscroll(&mut self, bounds: Bounds<Pixels>) {
        self.core.invalidator.debug_assert_prepaint();
        self.core.requested_autoscroll = Some(bounds);
    }

    /// This method can be called from a containing element such as [`crate::List`] to support the autoscroll behavior
    /// described in [`Self::request_autoscroll`].
    pub fn take_autoscroll(&mut self) -> Option<Bounds<Pixels>> {
        self.core.invalidator.debug_assert_prepaint();
        self.core.requested_autoscroll.take()
    }

    /// Asynchronously load an asset, if the asset hasn't finished loading this will return None.
    /// Your view will be re-drawn once the asset has finished loading.
    ///
    /// Note that the multiple calls to this method will only result in one `Asset::load` call at a
    /// time.
    pub fn use_asset<A: Asset>(&mut self, source: &A::Source, cx: &mut App) -> Option<A::Output> {
        let (task, is_first) = cx.fetch_asset::<A>(source);
        task.clone().now_or_never().or_else(|| {
            if is_first {
                let entity_id = self.current_view();
                self.spawn(cx, {
                    let task = task.clone();
                    async move |cx| {
                        task.await;

                        cx.on_pre_frame(move |_, cx| {
                            cx.notify(entity_id);
                        });
                    }
                })
                .detach();
            }

            None
        })
    }

    /// Asynchronously load an asset, if the asset hasn't finished loading or doesn't exist this will return None.
    /// Your view will not be re-drawn once the asset has finished loading.
    ///
    /// Note that the multiple calls to this method will only result in one `Asset::load` call at a
    /// time.
    pub fn get_asset<A: Asset>(&mut self, source: &A::Source, cx: &mut App) -> Option<A::Output> {
        let (task, _) = cx.fetch_asset::<A>(source);
        task.now_or_never()
    }
    /// Obtain the current element offset. This method should only be called during the
    /// prepaint phase of element drawing.
    pub fn element_offset(&self) -> Point<Pixels> {
        self.core.invalidator.debug_assert_prepaint();
        self.core.element_offset_stack
            .last()
            .copied()
            .unwrap_or_default()
    }

    /// Obtain the current element opacity. This method should only be called during the
    /// prepaint phase of element drawing.
    #[inline]
    pub(crate) fn element_opacity(&self) -> f32 {
        self.core.invalidator.debug_assert_paint_or_prepaint();
        self.core.element_opacity
    }

    /// Obtain the current content mask. This method should only be called during element drawing.
    pub fn content_mask(&self) -> ContentMask<Pixels> {
        self.core.invalidator.debug_assert_paint_or_prepaint();
        self.core.content_mask_stack
            .last()
            .cloned()
            .unwrap_or_else(|| ContentMask {
                bounds: Bounds {
                    origin: Point::default(),
                    size: self.core.viewport_size,
                },
            })
    }

    /// Provide elements in the called function with a new namespace in which their identifiers must be unique.
    /// This can be used within a custom element to distinguish multiple sets of child elements.
    pub fn with_element_namespace<R>(
        &mut self,
        element_id: impl Into<ElementId>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.with_pushed_element_id(element_id, |_, this| f(this))
    }

    /// Use a piece of state that exists as long this element is being rendered in consecutive frames.
    ///
    /// The key may be any existing [`ElementId`] conversion or a [`crate::Key`]. Use explicit value
    /// keys for reordered lists and repeated children whose state must follow data rather than
    /// sibling position.
    pub fn use_keyed_state<S: 'static>(
        &mut self,
        key: impl Into<ElementId>,
        cx: &mut App,
        init: impl FnOnce(&mut Self, &mut Context<S>) -> S,
    ) -> Entity<S> {
        let current_view = self.current_view();
        self.with_global_id(key.into(), |global_id, window| {
            window.with_element_state(global_id, |state: Option<Entity<S>>, window| {
                if let Some(state) = state {
                    (state.clone(), state)
                } else {
                    let new_state = cx.new(|cx| init(window, cx));
                    cx.observe(&new_state, move |_, cx| {
                        cx.notify(current_view);
                    })
                    .detach();
                    (new_state.clone(), new_state)
                }
            })
        })
    }

    /// Use a piece of state that exists as long this element is being rendered in consecutive frames, without needing to specify a key.
    ///
    /// This method uses the caller location plus the sibling occurrence in the current parent
    /// namespace. That is deterministic for a fixed tree shape, but it is not reorder-stable. If
    /// the state belongs to a list item or movable child, use [`Window::use_keyed_state`].
    #[track_caller]
    pub fn use_state<S: 'static>(
        &mut self,
        cx: &mut App,
        init: impl FnOnce(&mut Self, &mut Context<S>) -> S,
    ) -> Entity<S> {
        self.use_keyed_state(
            // Absolute path: `mod core;` (A10a PR 1.0) shadows the stdlib `core` crate
            // within this file's scope. Use `::core` to reach the standard library.
            ElementId::CodeLocation(*::core::panic::Location::caller()),
            cx,
            init,
        )
    }

    /// Updates or initializes state for an element with the given id that lives across multiple
    /// frames. If an element with this ID existed in the rendered frame, its state will be passed
    /// to the given closure. The state returned by the closure will be stored so it can be referenced
    /// when drawing the next frame. This method should only be called as part of element drawing.
    pub fn with_element_state<S, R>(
        &mut self,
        global_id: &GlobalElementId,
        f: impl FnOnce(Option<S>, &mut Self) -> (R, S),
    ) -> R
    where
        S: 'static,
    {
        self.core.invalidator.debug_assert_paint_or_prepaint();

        let key = (global_id.clone(), TypeId::of::<S>());
        self.core.next_frame.accessed_element_states.push(key.clone());

        if let Some(any) = self
            .core.next_frame
            .element_states
            .remove(&key)
            .or_else(|| self.core.rendered_frame.element_states.remove(&key))
        {
            let ElementStateBox {
                inner,
                #[cfg(debug_assertions)]
                type_name,
            } = any;
            // Using the extra inner option to avoid needing to reallocate a new box.
            let mut state_box = inner
                .downcast::<Option<S>>()
                .map_err(|_| {
                    #[cfg(debug_assertions)]
                    {
                        anyhow::anyhow!(
                            "invalid element state type for id, requested {:?}, actual: {:?}",
                            std::any::type_name::<S>(),
                            type_name
                        )
                    }

                    #[cfg(not(debug_assertions))]
                    {
                        anyhow::anyhow!(
                            "invalid element state type for id, requested {:?}",
                            std::any::type_name::<S>(),
                        )
                    }
                })
                .unwrap();

            // K15 (Phase 0-K): structured re-entry panic. The bare `expect`
            // is replaced with `unwrap_or_else(|| panic!(...))` carrying a
            // `ReentryError::ElementStateInUse` Display so log scrapers and
            // diagnostics see a stable, structured message. The function
            // signature is unchanged (still returns `R`, not
            // `Result<R, ReentryError>`) — preserves source compat at all 7
            // callsites; the panic shape satisfies the ROADMAP K15 intent of
            // "no undefined `RefCell::borrow_mut` panics" because the panic
            // is now defined and named.
            let state = state_box.take().unwrap_or_else(|| {
                panic!(
                    "{}",
                    crate::reentrancy::ReentryError::ElementStateInUse {
                        global_element_id: global_id.clone(),
                        type_id: TypeId::of::<S>(),
                    }
                )
            });
            let (result, state) = f(Some(state), self);
            state_box.replace(state);
            self.core.next_frame.element_states.insert(
                key,
                ElementStateBox {
                    inner: state_box,
                    #[cfg(debug_assertions)]
                    type_name,
                },
            );
            result
        } else {
            let (result, state) = f(None, self);
            self.core.next_frame.element_states.insert(
                key,
                ElementStateBox {
                    inner: Box::new(Some(state)),
                    #[cfg(debug_assertions)]
                    type_name: std::any::type_name::<S>(),
                },
            );
            result
        }
    }

    /// A variant of `with_element_state` that allows the element's id to be optional. This is a convenience
    /// method for elements where the element id may or may not be assigned. Prefer using `with_element_state`
    /// when the element is guaranteed to have an id.
    ///
    /// The first option means 'no ID provided'
    /// The second option means 'not yet initialized'
    pub fn with_optional_element_state<S, R>(
        &mut self,
        global_id: Option<&GlobalElementId>,
        f: impl FnOnce(Option<Option<S>>, &mut Self) -> (R, Option<S>),
    ) -> R
    where
        S: 'static,
    {
        self.core.invalidator.debug_assert_paint_or_prepaint();

        if let Some(global_id) = global_id {
            self.with_element_state(global_id, |state, cx| {
                let (result, state) = f(Some(state), cx);
                let state =
                    state.expect("you must return some state when you pass some element id");
                (result, state)
            })
        } else {
            let (result, state) = f(None, self);
            debug_assert!(
                state.is_none(),
                "you must not return an element state when passing None for the global id"
            );
            result
        }
    }

    /// Executes the given closure within the context of a tab group.
    ///
    /// ADR-010: this is the stable public sugar for the engine's
    /// `TabStopMap::{begin_group, end_group}` primitives. Widget authors
    /// **must** use this helper (not the underlying `TabStopMap`, which
    /// is `pub(crate)`) so a future change to the path representation
    /// stays an internal refactor.
    ///
    /// When `index` is `None` the closure runs without entering a group
    /// — convenient sugar for conditional grouping.
    ///
    /// Group boundaries are **not** absorbing (decision 4): tabbing out of
    /// the last element of the group lands on the next sibling in the
    /// parent's order, not on a group sentinel.
    ///
    /// See `docs/research/adr/ADR-010-local-tab-index.md`.
    #[inline]
    pub fn with_tab_group<R>(&mut self, index: Option<isize>, f: impl FnOnce(&mut Self) -> R) -> R {
        if let Some(index) = index {
            self.core.next_frame.tab_stops.begin_group(index);
            let result = f(self);
            self.core.next_frame.tab_stops.end_group();
            result
        } else {
            f(self)
        }
    }

    /// Defers the drawing of the given element, scheduling it to be painted on top of the currently-drawn tree
    /// at a later time. The `priority` parameter determines the drawing order relative to other deferred elements,
    /// with higher values being drawn on top.
    ///
    /// When `content_mask` is provided, the deferred element will be clipped to that region during
    /// both prepaint and paint. When `None`, no additional clipping is applied.
    ///
    /// This method should only be called as part of the prepaint phase of element drawing.
    pub fn defer_draw(
        &mut self,
        element: AnyElement,
        absolute_offset: Point<Pixels>,
        priority: usize,
        content_mask: Option<ContentMask<Pixels>>,
    ) {
        self.core.invalidator.debug_assert_prepaint();
        let parent_node = self.core.next_frame.dispatch_tree.active_node_id().unwrap();
        let lifecycle_metadata = element.lifecycle_metadata(self);
        self.core.next_frame.deferred_draws.push(DeferredDraw {
            current_view: self.current_view(),
            parent_node,
            global_id: lifecycle_metadata.global_id,
            inspector_id: lifecycle_metadata.inspector_id,
            bounds: lifecycle_metadata.bounds,
            element_id_stack: self.core.element_id_stack.clone(),
            text_style_stack: self.core.text_style_stack.clone(),
            content_mask,
            rem_size: self.rem_size(),
            priority,
            element: Some(element),
            absolute_offset,
            prepaint_range: PrepaintStateIndex::default()..PrepaintStateIndex::default(),
            paint_range: PaintIndex::default()..PaintIndex::default(),
        });
    }

    /// Creates a new painting layer for the specified bounds. A "layer" is a batch
    /// of geometry that are non-overlapping and have the same draw order. This is typically used
    /// for performance reasons.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_layer<R>(&mut self, bounds: Bounds<Pixels>, f: impl FnOnce(&mut Self) -> R) -> R {
        self.core.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let content_mask = self.content_mask();
        let clipped_bounds = bounds.intersect(&content_mask.bounds);
        if !clipped_bounds.is_empty() {
            self.core.next_frame
                .scene
                .push_layer(clipped_bounds.scale(scale_factor));
        }

        let result = f(self);

        if !clipped_bounds.is_empty() {
            self.core.next_frame.scene.pop_layer();
        }

        result
    }

    /// Paint one or more drop shadows into the scene for the next frame at the current z-index.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_shadows(
        &mut self,
        bounds: Bounds<Pixels>,
        corner_radii: Corners<Pixels>,
        shadows: &[BoxShadow],
    ) {
        self.core.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let content_mask = self.content_mask();
        let opacity = self.element_opacity();
        for shadow in shadows {
            let shadow_bounds = (bounds + shadow.offset).dilate(shadow.spread_radius);
            self.core.next_frame.scene.insert_primitive(Shadow {
                order: 0,
                blur_radius: shadow.blur_radius.scale(scale_factor),
                bounds: shadow_bounds.scale(scale_factor),
                content_mask: content_mask.scale(scale_factor),
                corner_radii: corner_radii.scale(scale_factor),
                color: shadow.color.opacity(opacity),
            });
        }
    }

    /// Paint one or more quads into the scene for the next frame at the current stacking context.
    /// Quads are colored rectangular regions with an optional background, border, and corner radius.
    /// see [`fill`], [`outline`], and [`quad`] to construct this type.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    ///
    /// Note that the `quad.corner_radii` are allowed to exceed the bounds, creating sharp corners
    /// where the circular arcs meet. This will not display well when combined with dashed borders.
    /// Use `Corners::clamp_radii_for_quad_size` if the radii should fit within the bounds.
    pub fn paint_quad(&mut self, quad: PaintQuad) {
        self.core.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let content_mask = self.content_mask();
        let opacity = self.element_opacity();
        self.core.next_frame.scene.insert_primitive(Quad {
            order: 0,
            bounds: quad.bounds.scale(scale_factor),
            content_mask: content_mask.scale(scale_factor),
            background: quad.background.opacity(opacity),
            border_color: quad.border_color.opacity(opacity),
            corner_radii: quad.corner_radii.scale(scale_factor),
            border_widths: quad.border_widths.scale(scale_factor),
            border_style: quad.border_style,
        });
    }

    /// Paint the given `Path` into the scene for the next frame at the current z-index.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_path(&mut self, mut path: Path<Pixels>, color: impl Into<Background>) {
        self.core.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let content_mask = self.content_mask();
        let opacity = self.element_opacity();
        path.content_mask = content_mask;
        let color: Background = color.into();
        path.color = color.opacity(opacity);
        self.core.next_frame
            .scene
            .insert_primitive(path.scale(scale_factor));
    }

    /// Paint an underline into the scene for the next frame at the current z-index.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_underline(
        &mut self,
        origin: Point<Pixels>,
        width: Pixels,
        style: &UnderlineStyle,
    ) {
        self.core.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let height = if style.wavy {
            style.thickness * 3.
        } else {
            style.thickness
        };
        let bounds = Bounds {
            origin,
            size: size(width, height),
        };
        let content_mask = self.content_mask();
        let element_opacity = self.element_opacity();

        self.core.next_frame.scene.insert_primitive(Underline {
            order: 0,
            pad: 0,
            bounds: bounds.scale(scale_factor),
            content_mask: content_mask.scale(scale_factor),
            color: style.color.unwrap_or_default().opacity(element_opacity),
            thickness: style.thickness.scale(scale_factor),
            wavy: if style.wavy { 1 } else { 0 },
        });
    }

    /// Paint a strikethrough into the scene for the next frame at the current z-index.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_strikethrough(
        &mut self,
        origin: Point<Pixels>,
        width: Pixels,
        style: &StrikethroughStyle,
    ) {
        self.core.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let height = style.thickness;
        let bounds = Bounds {
            origin,
            size: size(width, height),
        };
        let content_mask = self.content_mask();
        let opacity = self.element_opacity();

        self.core.next_frame.scene.insert_primitive(Underline {
            order: 0,
            pad: 0,
            bounds: bounds.scale(scale_factor),
            content_mask: content_mask.scale(scale_factor),
            thickness: style.thickness.scale(scale_factor),
            color: style.color.unwrap_or_default().opacity(opacity),
            wavy: 0,
        });
    }

    /// Paints a monochrome (non-emoji) glyph into the scene for the next frame at the current z-index.
    ///
    /// The y component of the origin is the baseline of the glyph.
    /// You should generally prefer to use the [`ShapedLine::paint`](crate::ShapedLine::paint) or
    /// [`WrappedLine::paint`](crate::WrappedLine::paint) methods in the [`TextSystem`](crate::TextSystem).
    /// This method is only useful if you need to paint a single glyph that has already been shaped.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_glyph(
        &mut self,
        origin: Point<Pixels>,
        font_id: FontId,
        glyph_id: GlyphId,
        font_size: Pixels,
        color: Hsla,
    ) -> Result<()> {
        self.core.invalidator.debug_assert_paint();

        let element_opacity = self.element_opacity();
        let scale_factor = self.scale_factor();
        let glyph_origin = origin.scale(scale_factor);

        let subpixel_variant = Point {
            x: (glyph_origin.x.0.fract() * SUBPIXEL_VARIANTS_X as f32).floor() as u8,
            y: (glyph_origin.y.0.fract() * SUBPIXEL_VARIANTS_Y as f32).floor() as u8,
        };
        let subpixel_rendering = self.should_use_subpixel_rendering(font_id, font_size);
        // ADR-013: cache key includes the raster mode. The text style
        // cascade plumbs `raster_mode` through; the resolved mode here
        // is the one actually drawn after the per-platform fallback
        // chain (see `TextRasterMode::resolve_with_fallback`). Wiring
        // the live style → mode resolution is a per-paint-call
        // follow-up; for now the engine defaults to `Subpixel` which
        // matches pre-ADR cache identity (no atlas churn from this
        // commit).
        let raster_mode = crate::TextRasterMode::Subpixel;
        let params = RenderGlyphParams {
            font_id,
            glyph_id,
            font_size,
            subpixel_variant,
            scale_factor,
            is_emoji: false,
            subpixel_rendering,
            raster_mode,
        };

        let raster_bounds = self.text_system().raster_bounds(&params)?;
        if !raster_bounds.is_zero() {
            let tile = self
                .core.sprite_atlas
                .get_or_insert_with(&params.clone().into(), &mut || {
                    let (size, bytes) = self.text_system().rasterize_glyph(&params)?;
                    Ok(Some((size, Cow::Owned(bytes))))
                })?
                .expect("Callback above only errors or returns Some");
            let bounds = Bounds {
                origin: glyph_origin.map(|px| px.floor()) + raster_bounds.origin.map(Into::into),
                size: tile.bounds.size.map(Into::into),
            };
            let content_mask = self.content_mask().scale(scale_factor);

            if subpixel_rendering {
                self.core.next_frame.scene.insert_primitive(SubpixelSprite {
                    order: 0,
                    pad: 0,
                    bounds,
                    content_mask,
                    color: color.opacity(element_opacity),
                    tile,
                    transformation: TransformationMatrix::unit(),
                });
            } else {
                self.core.next_frame.scene.insert_primitive(MonochromeSprite {
                    order: 0,
                    pad: 0,
                    bounds,
                    content_mask,
                    color: color.opacity(element_opacity),
                    tile,
                    transformation: TransformationMatrix::unit(),
                });
            }
        }
        Ok(())
    }

    /// Paints a monochrome glyph with pre-computed raster bounds.
    ///
    /// This is faster than `paint_glyph` because it skips the per-glyph cache lookup.
    /// Use `ShapedLine::compute_glyph_raster_data` to batch-compute raster bounds during prepaint.
    pub fn paint_glyph_with_raster_bounds(
        &mut self,
        origin: Point<Pixels>,
        _font_id: FontId,
        _glyph_id: GlyphId,
        _font_size: Pixels,
        color: Hsla,
        raster_bounds: Bounds<DevicePixels>,
        params: &RenderGlyphParams,
    ) -> Result<()> {
        self.core.invalidator.debug_assert_paint();

        let element_opacity = self.element_opacity();
        let scale_factor = self.scale_factor();
        let glyph_origin = origin.scale(scale_factor);

        if !raster_bounds.is_zero() {
            let tile = self
                .core.sprite_atlas
                .get_or_insert_with(&params.clone().into(), &mut || {
                    let (size, bytes) = self.text_system().rasterize_glyph(params)?;
                    Ok(Some((size, Cow::Owned(bytes))))
                })?
                .expect("Callback above only errors or returns Some");
            let bounds = Bounds {
                origin: glyph_origin.map(|px| px.floor()) + raster_bounds.origin.map(Into::into),
                size: tile.bounds.size.map(Into::into),
            };
            let content_mask = self.content_mask().scale(scale_factor);
            self.core.next_frame.scene.insert_primitive(MonochromeSprite {
                order: 0,
                pad: 0,
                bounds,
                content_mask,
                color: color.opacity(element_opacity),
                tile,
                transformation: TransformationMatrix::unit(),
            });
        }
        Ok(())
    }

    /// Paints an emoji glyph with pre-computed raster bounds.
    ///
    /// This is faster than `paint_emoji` because it skips the per-glyph cache lookup.
    /// Use `ShapedLine::compute_glyph_raster_data` to batch-compute raster bounds during prepaint.
    pub fn paint_emoji_with_raster_bounds(
        &mut self,
        origin: Point<Pixels>,
        _font_id: FontId,
        _glyph_id: GlyphId,
        _font_size: Pixels,
        raster_bounds: Bounds<DevicePixels>,
        params: &RenderGlyphParams,
    ) -> Result<()> {
        self.core.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let glyph_origin = origin.scale(scale_factor);

        if !raster_bounds.is_zero() {
            let tile = self
                .core.sprite_atlas
                .get_or_insert_with(&params.clone().into(), &mut || {
                    let (size, bytes) = self.text_system().rasterize_glyph(params)?;
                    Ok(Some((size, Cow::Owned(bytes))))
                })?
                .expect("Callback above only errors or returns Some");

            let bounds = Bounds {
                origin: glyph_origin.map(|px| px.floor()) + raster_bounds.origin.map(Into::into),
                size: tile.bounds.size.map(Into::into),
            };
            let content_mask = self.content_mask().scale(scale_factor);
            let opacity = self.element_opacity();

            self.core.next_frame.scene.insert_primitive(PolychromeSprite {
                order: 0,
                pad: 0,
                grayscale: false,
                bounds,
                corner_radii: Default::default(),
                content_mask,
                tile,
                opacity,
            });
        }
        Ok(())
    }

    fn should_use_subpixel_rendering(&self, font_id: FontId, font_size: Pixels) -> bool {
        if self.core.platform_window.background_appearance() != WindowBackgroundAppearance::Opaque {
            return false;
        }

        if !self.core.platform_window.is_subpixel_rendering_supported() {
            return false;
        }

        let mode = match self.core.text_rendering_mode.get() {
            TextRenderingMode::PlatformDefault => self
                .text_system()
                .recommended_rendering_mode(font_id, font_size),
            mode => mode,
        };

        mode == TextRenderingMode::Subpixel
    }

    /// Paints an emoji glyph into the scene for the next frame at the current z-index.
    ///
    /// The y component of the origin is the baseline of the glyph.
    /// You should generally prefer to use the [`ShapedLine::paint`](crate::ShapedLine::paint) or
    /// [`WrappedLine::paint`](crate::WrappedLine::paint) methods in the [`TextSystem`](crate::TextSystem).
    /// This method is only useful if you need to paint a single emoji that has already been shaped.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_emoji(
        &mut self,
        origin: Point<Pixels>,
        font_id: FontId,
        glyph_id: GlyphId,
        font_size: Pixels,
    ) -> Result<()> {
        self.core.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let glyph_origin = origin.scale(scale_factor);
        let params = RenderGlyphParams {
            font_id,
            glyph_id,
            font_size,
            // We don't render emojis with subpixel variants.
            subpixel_variant: Default::default(),
            scale_factor,
            is_emoji: true,
            // ADR-013: emoji raster mode is fixed to `Subpixel` (the
            // default) — bi-level / hinted modes only apply to outline
            // glyphs, not colour-bitmap emoji.
            raster_mode: crate::TextRasterMode::Subpixel,
            subpixel_rendering: false,
        };

        let raster_bounds = self.text_system().raster_bounds(&params)?;
        if !raster_bounds.is_zero() {
            let tile = self
                .core.sprite_atlas
                .get_or_insert_with(&params.clone().into(), &mut || {
                    let (size, bytes) = self.text_system().rasterize_glyph(&params)?;
                    Ok(Some((size, Cow::Owned(bytes))))
                })?
                .expect("Callback above only errors or returns Some");

            let bounds = Bounds {
                origin: glyph_origin.map(|px| px.floor()) + raster_bounds.origin.map(Into::into),
                size: tile.bounds.size.map(Into::into),
            };
            let content_mask = self.content_mask().scale(scale_factor);
            let opacity = self.element_opacity();

            self.core.next_frame.scene.insert_primitive(PolychromeSprite {
                order: 0,
                pad: 0,
                grayscale: false,
                bounds,
                corner_radii: Default::default(),
                content_mask,
                tile,
                opacity,
            });
        }
        Ok(())
    }

    /// Paint a monochrome SVG into the scene for the next frame at the current stacking context.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_svg(
        &mut self,
        bounds: Bounds<Pixels>,
        path: SharedString,
        mut data: Option<&[u8]>,
        transformation: TransformationMatrix,
        color: Hsla,
        cx: &App,
    ) -> Result<()> {
        self.core.invalidator.debug_assert_paint();

        let element_opacity = self.element_opacity();
        let scale_factor = self.scale_factor();

        let bounds = bounds.scale(scale_factor);
        let params = RenderSvgParams {
            path,
            size: bounds.size.map(|pixels| {
                DevicePixels::from((pixels.0 * SMOOTH_SVG_SCALE_FACTOR).ceil() as i32)
            }),
        };

        let Some(tile) =
            self.core.sprite_atlas
                .get_or_insert_with(&params.clone().into(), &mut || {
                    let Some((size, bytes)) = cx.svg_renderer.render_alpha_mask(&params, data)?
                    else {
                        return Ok(None);
                    };
                    Ok(Some((size, Cow::Owned(bytes))))
                })?
        else {
            return Ok(());
        };
        let content_mask = self.content_mask().scale(scale_factor);
        let svg_bounds = Bounds {
            origin: bounds.center()
                - Point::new(
                    ScaledPixels(tile.bounds.size.width.0 as f32 / SMOOTH_SVG_SCALE_FACTOR / 2.),
                    ScaledPixels(tile.bounds.size.height.0 as f32 / SMOOTH_SVG_SCALE_FACTOR / 2.),
                ),
            size: tile
                .bounds
                .size
                .map(|value| ScaledPixels(value.0 as f32 / SMOOTH_SVG_SCALE_FACTOR)),
        };

        self.core.next_frame.scene.insert_primitive(MonochromeSprite {
            order: 0,
            pad: 0,
            bounds: svg_bounds
                .map_origin(|origin| origin.round())
                .map_size(|size| size.ceil()),
            content_mask,
            color: color.opacity(element_opacity),
            tile,
            transformation,
        });

        Ok(())
    }

    /// Paint an image into the scene for the next frame at the current z-index.
    /// This method will panic if the frame_index is not valid
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn paint_image(
        &mut self,
        bounds: Bounds<Pixels>,
        corner_radii: Corners<Pixels>,
        data: Arc<RenderImage>,
        frame_index: usize,
        grayscale: bool,
    ) -> Result<()> {
        self.core.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let bounds = bounds.scale(scale_factor);
        let params = RenderImageParams {
            image_id: data.id,
            frame_index,
        };

        let tile = self
            .core.sprite_atlas
            .get_or_insert_with(&params.into(), &mut || {
                Ok(Some((
                    data.size(frame_index),
                    Cow::Borrowed(
                        data.as_bytes(frame_index)
                            .expect("It's the caller's job to pass a valid frame index"),
                    ),
                )))
            })?
            .expect("Callback above only returns Some");
        let content_mask = self.content_mask().scale(scale_factor);
        let corner_radii = corner_radii.scale(scale_factor);
        let opacity = self.element_opacity();

        self.core.next_frame.scene.insert_primitive(PolychromeSprite {
            order: 0,
            pad: 0,
            grayscale,
            bounds: bounds
                .map_origin(|origin| origin.floor())
                .map_size(|size| size.ceil()),
            content_mask,
            corner_radii,
            tile,
            opacity,
        });
        Ok(())
    }

    /// Paint a surface into the scene for the next frame at the current z-index.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    #[cfg(target_os = "macos")]
    pub fn paint_surface(&mut self, bounds: Bounds<Pixels>, image_buffer: CVPixelBuffer) {
        use crate::PaintSurface;

        self.core.invalidator.debug_assert_paint();

        let scale_factor = self.scale_factor();
        let bounds = bounds.scale(scale_factor);
        let content_mask = self.content_mask().scale(scale_factor);
        self.core.next_frame.scene.insert_primitive(PaintSurface {
            order: 0,
            bounds,
            content_mask,
            image_buffer,
        });
    }

    /// Removes an image from the sprite atlas.
    pub fn drop_image(&mut self, data: Arc<RenderImage>) -> Result<()> {
        for frame_index in 0..data.frame_count() {
            let params = RenderImageParams {
                image_id: data.id,
                frame_index,
            };

            self.core.sprite_atlas.remove(&params.clone().into());
        }

        Ok(())
    }

    /// Add a node to the layout tree for the current frame. Takes the `Style` of the element for which
    /// layout is being requested, along with the layout ids of any children. This method is called during
    /// calls to the [`Element::request_layout`] trait method and enables any element to participate in layout.
    ///
    /// This method should only be called as part of the request_layout or prepaint phase of element drawing.
    #[must_use]
    pub fn request_layout(
        &mut self,
        style: Style,
        children: impl IntoIterator<Item = LayoutId>,
        cx: &mut App,
    ) -> LayoutId {
        self.core.invalidator.debug_assert_prepaint();

        cx.layout_id_buffer.clear();
        cx.layout_id_buffer.extend(children);
        let rem_size = self.rem_size();
        let scale_factor = self.scale_factor();

        self.core.layout_engine.as_mut().unwrap().request_layout(
            style,
            rem_size,
            scale_factor,
            &cx.layout_id_buffer,
        )
    }

    /// Add a node to the layout tree for the current frame. Instead of taking a `Style` and children,
    /// this variant takes a function that is invoked during layout so you can use arbitrary logic to
    /// determine the element's size. One place this is used internally is when measuring text.
    ///
    /// The given closure is invoked at layout time with the known dimensions and available space and
    /// returns a `Size`.
    ///
    /// This method should only be called as part of the request_layout or prepaint phase of element drawing.
    pub fn request_measured_layout<F>(&mut self, style: Style, measure: F) -> LayoutId
    where
        F: Fn(Size<Option<Pixels>>, Size<AvailableSpace>, &mut Window, &mut App) -> Size<Pixels>
            + 'static,
    {
        self.core.invalidator.debug_assert_prepaint();

        let rem_size = self.rem_size();
        let scale_factor = self.scale_factor();
        self.core.layout_engine
            .as_mut()
            .unwrap()
            .request_measured_layout(style, rem_size, scale_factor, measure)
    }

    /// Compute the layout for the given id within the given available space.
    /// This method is called for its side effect, typically by the framework prior to painting.
    /// After calling it, you can request the bounds of the given layout node id or any descendant.
    ///
    /// This method should only be called as part of the prepaint phase of element drawing.
    pub fn compute_layout(
        &mut self,
        layout_id: LayoutId,
        available_space: Size<AvailableSpace>,
        cx: &mut App,
    ) {
        self.core.invalidator.debug_assert_prepaint();

        let mut layout_engine = self.core.layout_engine.take().unwrap();
        layout_engine.compute_layout(layout_id, available_space, self, cx);
        self.core.layout_engine = Some(layout_engine);
    }

    /// Obtain the bounds computed for the given LayoutId relative to the window. This method will usually be invoked by
    /// GPUI itself automatically in order to pass your element its `Bounds` automatically.
    ///
    /// This method should only be called as part of element drawing.
    pub fn layout_bounds(&mut self, layout_id: LayoutId) -> Bounds<Pixels> {
        self.core.invalidator.debug_assert_prepaint();

        let scale_factor = self.scale_factor();
        let mut bounds = self
            .core.layout_engine
            .as_mut()
            .unwrap()
            .layout_bounds(layout_id, scale_factor)
            .map(Into::into);
        bounds.origin += self.element_offset();
        bounds
    }

    /// This method should be called during `prepaint`. You can use
    /// the returned [Hitbox] during `paint` or in an event handler
    /// to determine whether the inserted hitbox was the topmost.
    ///
    /// This method should only be called as part of the prepaint phase of element drawing.
    pub fn insert_hitbox(&mut self, bounds: Bounds<Pixels>, behavior: HitboxBehavior) -> Hitbox {
        self.core.invalidator.debug_assert_prepaint();

        let content_mask = self.content_mask();
        let mut id = self.core.next_hitbox_id;
        self.core.next_hitbox_id = self.core.next_hitbox_id.next();
        let hitbox = Hitbox {
            id,
            bounds,
            content_mask,
            behavior,
        };
        self.core.next_frame.hitboxes.push(hitbox.clone());
        hitbox
    }

    /// Set a hitbox which will act as a control area of the platform window.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn insert_window_control_hitbox(&mut self, area: WindowControlArea, hitbox: Hitbox) {
        self.core.invalidator.debug_assert_paint();
        self.core.next_frame.window_control_hitboxes.push((area, hitbox));
    }

    /// Sets the key context for the current element. This context will be used to translate
    /// keybindings into actions.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn set_key_context(&mut self, context: KeyContext) {
        self.core.invalidator.debug_assert_paint();
        self.core.next_frame.dispatch_tree.set_key_context(context);
    }

    /// Sets the focus handle for the current element. This handle will be used to manage focus state
    /// and keyboard event dispatch for the element.
    ///
    /// This method should only be called as part of the prepaint phase of element drawing.
    pub fn set_focus_handle(&mut self, focus_handle: &FocusHandle, _: &App) {
        self.core.invalidator.debug_assert_prepaint();
        if focus_handle.is_focused(self) {
            self.core.next_frame.focus = Some(focus_handle.id);
        }
        self.core.next_frame.dispatch_tree.set_focus_id(focus_handle.id);
    }

    /// Sets the view id for the current element, which will be used to manage view caching.
    ///
    /// This method should only be called as part of element prepaint. We plan on removing this
    /// method eventually when we solve some issues that require us to construct editor elements
    /// directly instead of always using editors via views.
    pub fn set_view_id(&mut self, view_id: EntityId) {
        self.core.invalidator.debug_assert_prepaint();
        self.core.next_frame.dispatch_tree.set_view_id(view_id);
    }

    /// Get the entity ID for the currently rendering view
    pub fn current_view(&self) -> EntityId {
        self.core.invalidator.debug_assert_paint_or_prepaint();
        self.core.rendered_entity_stack.last().copied().unwrap()
    }

    pub(crate) fn try_current_view(&self) -> Option<EntityId> {
        self.core.rendered_entity_stack.last().copied()
    }

    /// Reads the nearest inherited value of type `T` without registering a
    /// dependency on the current element or view.
    pub fn read_inherited<T: InheritedValue>(&self) -> Option<T> {
        self.core.inherited_registry.read::<T>()
    }

    pub(crate) fn inherit_inherited<T: InheritedValue>(
        &mut self,
        dependent_element: &GlobalElementId,
        dependent_view: EntityId,
    ) -> Option<T> {
        self.core.inherited_registry
            .inherit::<T>(dependent_element, dependent_view)
    }

    pub(crate) fn inherited_dependency_index(&self) -> usize {
        self.core.inherited_registry.accessed_dependency_index()
    }

    pub(crate) fn inherited_dependencies_since(&self, index: usize) -> Vec<InheritedDependency> {
        self.core.inherited_registry.accessed_dependencies_since(index)
    }

    pub(crate) fn inherited_provider_access_index(&self) -> usize {
        self.core.inherited_registry.accessed_provider_index()
    }

    pub(crate) fn inherited_provider_accesses_since(&self, index: usize) -> Vec<ProviderScopeKey> {
        self.core.inherited_registry.accessed_providers_since(index)
    }

    pub(crate) fn replay_inherited_provider_accesses(&mut self, providers: &[ProviderScopeKey]) {
        for provider in providers {
            self.core.inherited_registry
                .replay_provider_access(provider.clone());
        }
    }

    pub(crate) fn validate_inherited_cache(
        &mut self,
        provider_accesses: &[ProviderScopeKey],
        dependencies: &[InheritedDependency],
        cx: &mut App,
    ) -> bool {
        let dirty_views = self
            .core.inherited_registry
            .validate_cached_dependencies(provider_accesses, dependencies);
        let is_valid = dirty_views.is_empty();
        self.invalidate_inherited_dependents(dirty_views, cx);
        is_valid
    }

    pub(crate) fn replay_inherited_dependencies(
        &mut self,
        dependencies: &[InheritedDependency],
        cx: &mut App,
    ) -> SmallVec<[EntityId; 8]> {
        let mut dirty_views = SmallVec::new();

        for dependency in dependencies {
            for view_id in self
                .core.inherited_registry
                .replay_dependency(dependency.clone())
            {
                if !dirty_views.contains(&view_id) {
                    dirty_views.push(view_id);
                }
            }
        }

        self.invalidate_inherited_dependents(dirty_views.clone(), cx);
        dirty_views
    }

    pub(crate) fn with_inherited_provider<T: InheritedValue, R>(
        &mut self,
        scope_id: &GlobalElementId,
        value: &T,
        cx: &mut App,
        f: impl FnOnce(&mut Self, &mut App) -> R,
    ) -> R {
        let dirty_views = self.core.inherited_registry.provide::<T>(scope_id, value);
        self.invalidate_inherited_dependents(dirty_views, cx);
        self.core.inherited_registry.push_active::<T>(scope_id);

        let result = panic::catch_unwind(AssertUnwindSafe(|| f(self, cx)));

        self.core.inherited_registry.pop_active::<T>(scope_id);

        match result {
            Ok(result) => result,
            Err(payload) => panic::resume_unwind(payload),
        }
    }

    fn invalidate_inherited_dependents<const N: usize>(
        &mut self,
        dirty_views: SmallVec<[EntityId; N]>,
        cx: &mut App,
    ) {
        for view_id in dirty_views {
            self.core.invalidator.invalidate_view(view_id, cx);
        }
    }

    #[inline]
    pub(crate) fn with_rendered_view<R>(
        &mut self,
        id: EntityId,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.core.rendered_entity_stack.push(id);
        let result = f(self);
        self.core.rendered_entity_stack.pop();
        result
    }

    /// Executes the provided function with the specified image cache.
    pub fn with_image_cache<F, R>(&mut self, image_cache: Option<AnyImageCache>, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        if let Some(image_cache) = image_cache {
            self.core.image_cache_stack.push(image_cache);
            let result = f(self);
            self.core.image_cache_stack.pop();
            result
        } else {
            f(self)
        }
    }

    /// Sets an input handler, such as [`ElementInputHandler`][element_input_handler], which interfaces with the
    /// platform to receive textual input with proper integration with concerns such
    /// as IME interactions. This handler will be active for the upcoming frame until the following frame is
    /// rendered.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    ///
    /// [element_input_handler]: crate::ElementInputHandler
    pub fn handle_input(
        &mut self,
        focus_handle: &FocusHandle,
        input_handler: impl InputHandler,
        cx: &App,
    ) {
        self.core.invalidator.debug_assert_paint();

        if focus_handle.is_focused(self) {
            let cx = self.to_async(cx);
            self.core.next_frame
                .input_handlers
                .push(Some(PlatformInputHandler::new(cx, Box::new(input_handler))));
        }
    }

    /// Register a mouse event listener on the window for the next frame. The type of event
    /// is determined by the first parameter of the given listener. When the next frame is rendered
    /// the listener will be cleared.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn on_mouse_event<Event: MouseEvent>(
        &mut self,
        mut listener: impl FnMut(&Event, DispatchPhase, &mut Window, &mut App) + 'static,
    ) {
        self.core.invalidator.debug_assert_paint();

        self.core.next_frame.mouse_listeners.push(Some(Box::new(
            move |event: &dyn Any, phase: DispatchPhase, window: &mut Window, cx: &mut App| {
                if let Some(event) = event.downcast_ref() {
                    listener(event, phase, window, cx)
                }
            },
        )));
    }

    /// Register a key event listener on this node for the next frame. The type of event
    /// is determined by the first parameter of the given listener. When the next frame is rendered
    /// the listener will be cleared.
    ///
    /// This is a fairly low-level method, so prefer using event handlers on elements unless you have
    /// a specific need to register a listener yourself.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn on_key_event<Event: KeyEvent>(
        &mut self,
        listener: impl Fn(&Event, DispatchPhase, &mut Window, &mut App) + 'static,
    ) {
        self.core.invalidator.debug_assert_paint();

        self.core.next_frame.dispatch_tree.on_key_event(Rc::new(
            move |event: &dyn Any, phase, window: &mut Window, cx: &mut App| {
                if let Some(event) = event.downcast_ref::<Event>() {
                    listener(event, phase, window, cx)
                }
            },
        ));
    }

    /// Register a modifiers changed event listener on the window for the next frame.
    ///
    /// This is a fairly low-level method, so prefer using event handlers on elements unless you have
    /// a specific need to register a global listener.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn on_modifiers_changed(
        &mut self,
        listener: impl Fn(&ModifiersChangedEvent, &mut Window, &mut App) + 'static,
    ) {
        self.core.invalidator.debug_assert_paint();

        self.core.next_frame.dispatch_tree.on_modifiers_changed(Rc::new(
            move |event: &ModifiersChangedEvent, window: &mut Window, cx: &mut App| {
                listener(event, window, cx)
            },
        ));
    }

    /// Register a listener to be called when the given focus handle or one of its descendants receives focus.
    /// This does not fire if the given focus handle - or one of its descendants - was previously focused.
    /// Returns a subscription and persists until the subscription is dropped.
    pub fn on_focus_in(
        &mut self,
        handle: &FocusHandle,
        cx: &mut App,
        mut listener: impl FnMut(&mut Window, &mut App) + 'static,
    ) -> Subscription {
        let focus_id = handle.id;
        let (subscription, activate) =
            self.new_focus_listener(Box::new(move |event, window, cx| {
                if event.is_focus_in(focus_id) {
                    listener(window, cx);
                }
                true
            }));
        cx.defer(move |_| activate());
        subscription
    }

    /// Register a listener to be called when the given focus handle or one of its descendants loses focus.
    /// Returns a subscription and persists until the subscription is dropped.
    pub fn on_focus_out(
        &mut self,
        handle: &FocusHandle,
        cx: &mut App,
        mut listener: impl FnMut(FocusOutEvent, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        let focus_id = handle.id;
        let (subscription, activate) =
            self.new_focus_listener(Box::new(move |event, window, cx| {
                if let Some(blurred_id) = event.previous_focus_path.last().copied()
                    && event.is_focus_out(focus_id)
                {
                    let event = FocusOutEvent {
                        blurred: WeakFocusHandle {
                            id: blurred_id,
                            handles: Arc::downgrade(&cx.focus_handles),
                        },
                    };
                    listener(event, window, cx)
                }
                true
            }));
        cx.defer(move |_| activate());
        subscription
    }

    fn reset_cursor_style(&self, cx: &mut App) {
        // Set the cursor only if we're the active window.
        if self.is_window_hovered() {
            let style = self
                .core.rendered_frame
                .cursor_style(self)
                .unwrap_or(CursorStyle::Arrow);
            cx.platform.set_cursor_style(style);
        }
    }

    /// Dispatch a given keystroke as though the user had typed it.
    /// You can create a keystroke with Keystroke::parse("").
    pub fn dispatch_keystroke(&mut self, keystroke: Keystroke, cx: &mut App) -> bool {
        let keystroke = keystroke.with_simulated_ime();
        let result = self.dispatch_event(
            PlatformInput::KeyDown(KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        );
        if !result.propagate {
            return true;
        }

        if let Some(input) = keystroke.key_char
            && let Some(mut input_handler) = self.core.platform_window.take_input_handler()
        {
            input_handler.dispatch_input(&input, self, cx);
            self.core.platform_window.set_input_handler(input_handler);
            return true;
        }

        false
    }

    /// Return a key binding string for an action, to display in the UI. Uses the highest precedence
    /// binding for the action (last binding added to the keymap).
    pub fn keystroke_text_for(&self, action: &dyn Action) -> String {
        self.highest_precedence_binding_for_action(action)
            .map(|binding| {
                binding
                    .keystrokes()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_else(|| action.name().to_string())
    }

    /// Dispatch a mouse or keyboard event on the window.
    #[profiling::function]
    pub fn dispatch_event(&mut self, event: PlatformInput, cx: &mut App) -> DispatchEventResult {
        // Track input modality for focus-visible styling and hover suppression.
        // Hover is suppressed during keyboard modality so that keyboard navigation
        // doesn't show hover highlights on the item under the mouse cursor.
        let old_modality = self.core.last_input_modality;
        self.core.last_input_modality = match &event {
            PlatformInput::KeyDown(_) => InputModality::Keyboard,
            PlatformInput::MouseMove(_) | PlatformInput::MouseDown(_) => InputModality::Mouse,
            _ => self.core.last_input_modality,
        };
        if self.core.last_input_modality != old_modality {
            self.refresh();
        }

        // Handlers may set this to false by calling `stop_propagation`.
        cx.propagate_event = true;
        // Handlers may set this to true by calling `prevent_default`.
        self.core.default_prevented = false;

        let event = match event {
            // Track the mouse position with our own state, since accessing the platform
            // API for the mouse position can only occur on the main thread.
            PlatformInput::MouseMove(mouse_move) => {
                self.core.mouse_position = mouse_move.position;
                self.core.modifiers = mouse_move.modifiers;
                PlatformInput::MouseMove(mouse_move)
            }
            PlatformInput::MouseDown(mouse_down) => {
                self.core.mouse_position = mouse_down.position;
                self.core.modifiers = mouse_down.modifiers;
                PlatformInput::MouseDown(mouse_down)
            }
            PlatformInput::MouseUp(mouse_up) => {
                self.core.mouse_position = mouse_up.position;
                self.core.modifiers = mouse_up.modifiers;
                PlatformInput::MouseUp(mouse_up)
            }
            PlatformInput::MousePressure(mouse_pressure) => {
                PlatformInput::MousePressure(mouse_pressure)
            }
            PlatformInput::MouseExited(mouse_exited) => {
                self.core.modifiers = mouse_exited.modifiers;
                PlatformInput::MouseExited(mouse_exited)
            }
            PlatformInput::ModifiersChanged(modifiers_changed) => {
                self.core.modifiers = modifiers_changed.modifiers;
                self.core.capslock = modifiers_changed.capslock;
                PlatformInput::ModifiersChanged(modifiers_changed)
            }
            PlatformInput::ScrollWheel(scroll_wheel) => {
                self.core.mouse_position = scroll_wheel.position;
                self.core.modifiers = scroll_wheel.modifiers;
                PlatformInput::ScrollWheel(scroll_wheel)
            }
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            PlatformInput::Pinch(pinch) => {
                self.core.mouse_position = pinch.position;
                self.core.modifiers = pinch.core.modifiers;
                PlatformInput::Pinch(pinch)
            }
            // Translate dragging and dropping of external files from the operating system
            // to internal drag and drop events.
            //
            // ADR-011: `FileDropEvent` is now an alias for `ExternalDropEvent`
            // and carries `payload: ExternalDropPayload` instead of `paths`.
            // For the `Paths` variant the active_drag preview behaviour is
            // unchanged (ExternalPaths still has its Render impl). Other
            // payload variants (Urls, Text, Html, Mime, Mixed) currently fall
            // through without a drag preview — wider preview rendering is a
            // future-work item tracked in the rollout plan.
            //
            // The pre-existing pipeline (also pre-ADR) converts `Entered`/
            // `Pending`/`Submit` into synthetic mouse events for downstream
            // hit-test / listener dispatch (the typed payload is consumed
            // here in the `Entered` branch only, to set up `cx.active_drag`
            // for the drag-preview painter). Wiring `on_drop` listeners to
            // observe the original typed `ExternalDropEvent` directly is a
            // follow-up that would replace this conversion with a typed
            // dispatch path.
            PlatformInput::FileDrop(file_drop) => match file_drop {
                ExternalDropEvent::Entered { position, payload } => {
                    self.core.mouse_position = position;
                    if cx.active_drag.is_none() {
                        if let ExternalDropPayload::Paths(paths) = payload {
                            cx.active_drag = Some(AnyDrag {
                                value: Arc::new(paths.clone()),
                                view: cx.new(|_| paths).into(),
                                cursor_offset: position,
                                cursor_style: None,
                            });
                        }
                        // Non-Paths variants: no engine-side drag preview
                        // yet; widgets can paint their own on `Entered`.
                    }
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position,
                        pressed_button: Some(MouseButton::Left),
                        modifiers: Modifiers::default(),
                    })
                }
                ExternalDropEvent::Pending { position } => {
                    self.core.mouse_position = position;
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position,
                        pressed_button: Some(MouseButton::Left),
                        modifiers: Modifiers::default(),
                    })
                }
                ExternalDropEvent::Submit { position } => {
                    cx.activate(true);
                    self.core.mouse_position = position;
                    PlatformInput::MouseUp(MouseUpEvent {
                        button: MouseButton::Left,
                        position,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                    })
                }
                ExternalDropEvent::Exited => {
                    cx.active_drag.take();
                    PlatformInput::FileDrop(ExternalDropEvent::Exited)
                }
            },
            PlatformInput::KeyDown(_) | PlatformInput::KeyUp(_) => event,
            // ADR-009: pass-through. The downstream routing of
            // `EditorCommand` to the focused widget's
            // `InputHandler::handle_editor_command` is TODO — for now the
            // event flows through the dispatch path unchanged, and a
            // future commit wires the focused-handler lookup. The macOS
            // bridge already falls back to the keystroke path when the
            // command is unhandled, so user-visible behavior is unchanged
            // until the routing lands.
            PlatformInput::EditorCommand(_) => event,
        };

        // S07 T6 — Gesture-pass scaffold.
        //
        // Translate the inbound `PlatformInput` into normalized
        // `PointerEvent`s / `PointerSignalEvent`s for downstream
        // consumption by the gesture arena (T15 will wire the arena
        // dispatch + listener-chain reset). For T6 the translation
        // runs and updates the per-`Window` `gesture_pointer_state`
        // (so that delta / pressure / button-state caches stay
        // current), but the produced events are discarded — recognizers
        // do not exist yet (T7–T13).
        //
        // The arena pass MUST be isolated from the existing
        // `dispatch_mouse_event` listener chain by an explicit
        // `cx.propagate_event = true` reset, otherwise a recognizer's
        // `cx.stop_propagation()` would silently break the
        // `cx.active_drag` / `AnyDrag` contract — see
        // `crate::gesture::recognizer` trait docs and the design doc
        // § "Window::dispatch_event integration".
        {
            // Refresh the per-Window content bounds so the sanitizer
            // can clamp out-of-bounds positions (Wayland decoration
            // drag, etc.).
            //
            // **Critical:** [`super::gesture::dispatch::PointerSanitizer::convert`]
            // clamps **client-space** event positions — origin at
            // the window's top-left (0, 0), x grows right, y grows
            // down. The bounds it clamps against MUST be in the
            // same coordinate space.
            //
            // Using [`Self::bounds`] here was a long-standing
            // Windows regression: that method returns the window's
            // **screen** position in global coordinates, so any
            // window not anchored at screen `(0, 0)` saw its mouse
            // events collapsed onto the window's screen origin
            // (every Down/Up/Hover snapped to
            // `(window_screen_x, window_screen_y)`). When the
            // window happened to be at `(0, 0)` the bug was
            // invisible because client and screen coordinates
            // coincide — masking the regression in casual testing
            // and on `interactive_elements` examples that pin the
            // window in the top-left corner.
            //
            // The sanitizer's actual job is "clamp into the content
            // rectangle"; in client space that rectangle is always
            // `origin = (0, 0)`, `size = viewport_size`.
            let bounds = Bounds {
                origin: Point::default(),
                size: self.viewport_size(),
            };
            self.core.gesture_binding.pointer_state_mut().content_bounds = bounds;

            // PointerSignalEvent (Scroll / Scale / ScrollInertiaCancel) explicitly bypasses
            // the gesture arena per the design — those events have no
            // competition semantics. We still normalize and resolve
            // them through `PointerSignalResolver` here so the future
            // typed signal-listener path has a single, testable seam.
            // For compatibility the resolver currently records only
            // the route. The original platform event still reaches the
            // legacy `on_scroll_wheel` / `on_pinch` listener chain via
            // `dispatch_mouse_event` below.
            let mut pointer_signal = {
                let (sanitizer, pointer_state) = self.core.gesture_binding.dispatch_split_mut();
                sanitizer.convert_signal(&event, pointer_state)
            };
            if let Some(signal) = pointer_signal.as_mut() {
                signal.set_window_id(self.core.handle.window_id());
                let route = self
                    .core.gesture_binding
                    .resolve_pointer_signal_to_mouse_listeners(signal);
                let source = signal.source();
                log::trace!(
                    target: "flui::gesture::window",
                    phase = "pointer_signal",
                    pointer_id = signal.pointer_id().raw(),
                    signal_kind = format!("{:?}", signal.signal_kind()),
                    window_id = format!("{:?}", source.window_id.map(|id| id.as_u64())),
                    device_id = format!("{:?}", source.device_id),
                    platform_event_id = format!("{:?}", source.platform_event_id),
                    route = format!("{:?}", route);
                    "resolved pointer signal outside gesture arena"
                );
            }

            let pointer_events = {
                let (sanitizer, pointer_state) = self.core.gesture_binding.dispatch_split_mut();
                sanitizer.convert(&event, pointer_state)
            };

            // Query hit-test for each translated pointer event,
            // synthesize Enter/Exit transitions on hover, register
            // any recognizers parked by `Interactivity::paint` for
            // hitboxes under the pointer, and dispatch through the
            // arena.
            for pe in pointer_events.iter() {
                let hit_test = self.hit_test(pe.position);
                {
                    let (sanitizer, pointer_state) = self.core.gesture_binding.dispatch_split_mut();
                    // Hover Enter/Exit synthesis. The call mutates
                    // `state.prior_hover_hitboxes` so the next frame's
                    // diff is correct, but the returned synthetic
                    // events are not yet dispatched: T15.5 will route
                    // them through arena/hover-listeners. Leaving the
                    // state-mutation in place keeps the diff
                    // bootstrapped — removing it would force the
                    // first dispatched frame after T15 to compare
                    // against an empty set and emit a flood of
                    // spurious Enters. (Copilot review C.)
                    let _hover_events = sanitizer.diff_hover(pe, &hit_test, pointer_state);
                }

                // S07 T15 — recognizer registration. On `Down`, walk
                // the hit-test result front-to-back and drain the
                // recognizers that `Interactivity::paint` parked
                // under each `HitboxId`. Each recognizer joins the
                // arena keyed by `pe.pointer_id` and observes the
                // initiating event via `add_pointer`, then continues
                // to receive the rest of the gesture through the
                // arena's `dispatch` chain below. `Translucent` /
                // `DeferToChild` entries forward through to the next
                // hitbox behind them; `Opaque` ends the walk.
                //
                // S07.5 T3+T6 — registration goes through the
                // `GestureBinding::register_recognizer` seam, which
                // drives the `RecognizerLifecycle` hooks
                // (per-window settings, arena back-channel) and
                // returns `true` when any recognizer for this
                // hitbox asked the arena to enter `hold` mode
                // (DoubleTap). When that happens, the dispatcher
                // calls `arena.hold(pointer_id)` and schedules a
                // `double_tap_timeout`-deferred `arena.release`.
                if matches!(pe.phase, crate::gesture::PointerPhase::Down) {
                    let window_handle = self.core.handle;
                    let mut needs_hold = false;
                    for entry in hit_test.iter() {
                        let recs = self.core.pending_recognizers.remove(&entry.hitbox_id);
                        if let Some(recs) = recs {
                            // Compute hit-target-local position for
                            // this entry. S07.5b: hit-test entries do
                            // not yet store non-identity transforms,
                            // so the local position equals
                            // `pe.position`. S09 paint integration
                            // will populate `entry.transform` and
                            // this inversion will start to do real
                            // work.
                            //
                            // The stored `entry.transform` follows
                            // the Flutter `local → window` convention
                            // (see `HitTestEntry.transform` rustdoc):
                            // invert and apply once per delivery to
                            // recover the per-target local
                            // coordinate.
                            //
                            // **Invertibility contract.** Paint
                            // promises every transform it pushes is
                            // invertible. A `Some(t)` whose
                            // `inverse()` returns `None` is a
                            // paint-side bug (singular `Affine2`):
                            // in dev / test builds we panic via
                            // `debug_assert!` so the failing call
                            // site surfaces in CI; in release we
                            // degrade to identity + `log::warn!`
                            // rather than drop the event. The
                            // rustdoc on `HitTestEntry.transform`
                            // documents this strict-in-dev /
                            // lenient-in-release posture so reviewers
                            // can audit the contract from one place.
                            let local_position = match entry.transform {
                                None => pe.position,
                                Some(t) => match t.inverse() {
                                    Some(inv) => inv.transform_point(pe.position),
                                    None => {
                                        debug_assert!(
                                            false,
                                            "HitTestEntry.transform is non-invertible — paint pushed a singular Affine2 (hitbox_id={:?})",
                                            entry.hitbox_id,
                                        );
                                        log::warn!(
                                            target: "flui::gesture",
                                            "non-invertible HitTestEntry.transform — falling back to identity local_position (paint pushed a singular Affine2; please file a bug)"
                                        );
                                        pe.position
                                    }
                                },
                            };
                            let delivered = crate::gesture::DeliveredEvent::new(pe, local_position);
                            for rec in recs.iter() {
                                // Register first; only then prime the
                                // recognizer's per-pointer state.
                                // Decision D10: a filter-rejected
                                // recognizer never sees `add_pointer`,
                                // so paint-time recognizer instances
                                // re-used across paint cycles cannot
                                // leak stale `down_position` / state
                                // mutation from a rejected
                                // registration. `arena.add` does not
                                // dispatch — `arena.dispatch` for
                                // this Down event runs later in this
                                // function — so swapping the order
                                // is safe under the synchronous
                                // main-thread invariant.
                                let result = self.core.gesture_binding.register_recognizer(
                                    pe.pointer_id,
                                    pe.buttons,
                                    pe.modifiers,
                                    std::rc::Rc::clone(rec),
                                );
                                if let crate::gesture::RegistrationResult::Accepted {
                                    needs_hold: rec_needs_hold,
                                } = result
                                {
                                    rec.borrow_mut().add_pointer(pe.pointer_id, delivered);
                                    needs_hold = needs_hold || rec_needs_hold;
                                }
                            }
                        }
                        if matches!(entry.behavior, crate::gesture::HitTestBehavior::Opaque) {
                            break;
                        }
                    }
                    if needs_hold {
                        self.core.gesture_binding
                            .arena_rc()
                            .borrow_mut()
                            .hold(pe.pointer_id);
                        self.core.gesture_binding.schedule_arena_release(
                            pe.pointer_id,
                            window_handle,
                            cx,
                        );
                    }
                }

                // Arena pass — eager-accept fires; sweep on `Up`
                // declares the first registered the winner; `Cancel`
                // forces all entries `rejected`. The arena is keyed
                // per-pointer; multiple events on the same pointer
                // funnel through the same arena.
                //
                // The `arena_take` / `arena_restore` dance mirrors
                // `dispatch_mouse_event`'s handling of
                // `rendered_frame.mouse_listeners`: extract the arena
                // (replacing with `Default`), dispatch (which needs
                // `&mut self` for `&mut Window`), then restore the
                // arena alongside any entries the dispatched callbacks
                // appended through `GestureBinding::register_recognizer`.
                let mut arena = self.core.gesture_binding.arena_take();
                arena.dispatch(pe.pointer_id, pe, self, cx);
                match pe.phase {
                    crate::gesture::PointerPhase::Up => {
                        arena.sweep(pe.pointer_id, self, cx);
                        // Cancel the held-arena release timer ONLY
                        // when the arena has actually resolved on this
                        // event (a winner was declared, or the arena
                        // was gc'd entirely). DoubleTap's first `Up`
                        // transitions the recognizer to `AwaitSecond`
                        // and the held arena returns no winner — in
                        // that case the timer must keep running so a
                        // missing second tap eventually triggers
                        // `arena.release` after `double_tap_timeout`.
                        // Cancelling unconditionally on every `Up`
                        // (the previous behaviour) dropped the timer
                        // after the first tap and left the arena held
                        // forever, defeating the whole point of T6.
                        let resolution = arena.terminal_resolution(pe.pointer_id);
                        log::trace!(
                            target: "flui::gesture::window",
                            phase = "terminal_resolution",
                            pointer_id = format!("{:?}", pe.pointer_id),
                            winner_declared = resolution.winner_declared,
                            arena_open = resolution.is_open,
                            hold_count = resolution.hold_count,
                            entry_count = resolution.entry_count,
                            active_arena_count = arena.arena_count(),
                            resolved = resolution.resolved;
                            "evaluated terminal gesture arena resolution"
                        );
                        if resolution.resolved {
                            self.core.gesture_binding.cancel_arena_hold(pe.pointer_id);
                            // If a resolved arena is still present
                            // (for example because a losing recognizer
                            // held it), evict it before the next Down
                            // can append a fresh recognizer batch.
                            arena.close_resolved(pe.pointer_id);
                        }
                    }
                    crate::gesture::PointerPhase::Cancel => {
                        arena.cancel(pe.pointer_id, self, cx);
                        self.core.gesture_binding.cancel_arena_hold(pe.pointer_id);
                    }
                    // Device-leave (`MouseExit` translated by
                    // `dispatch::translate_mouse_exited` after S07.5
                    // T8). The pointer is gone — cancel any in-flight
                    // gesture and drop the held-arena timer alongside,
                    // mirroring the `Cancel` branch.
                    crate::gesture::PointerPhase::Removed => {
                        arena.cancel(pe.pointer_id, self, cx);
                        self.core.gesture_binding.cancel_arena_hold(pe.pointer_id);
                    }
                    _ => {}
                }
                // Merge back via `merge_by_pointer_id` so callback-time
                // registrations on the same pointer extend the
                // existing arena entry instead of producing a duplicate
                // `(PointerId, GestureArena)` pair (S07.5 T7).
                let live = self.core.gesture_binding.arena_take();
                arena.merge_by_pointer_id(live);
                self.core.gesture_binding.arena_restore(arena);

                // Boundary reset — guarantees that recognizer
                // `cx.stop_propagation()` calls (forbidden by trait
                // contract; verified by T17 tests) cannot suppress
                // raw `on_mouse_*` listeners, preserving the
                // `cx.active_drag` / `AnyDrag` contract. See the
                // design doc § "Window::dispatch_event integration".
                cx.propagate_event = true;
            }
        }

        if let Some(any_mouse_event) = event.mouse_event() {
            self.dispatch_mouse_event(any_mouse_event, cx);
        } else if let Some(any_key_event) = event.keyboard_event() {
            self.dispatch_key_event(any_key_event, cx);
        }

        if self.core.invalidator.is_dirty() {
            self.core.input_rate_tracker.borrow_mut().record_input();
        }

        DispatchEventResult {
            propagate: cx.propagate_event,
            default_prevented: self.core.default_prevented,
        }
    }

    fn dispatch_mouse_event(&mut self, event: &dyn Any, cx: &mut App) {
        let hit_test = self.core.rendered_frame.hit_test(self.mouse_position());
        if hit_test != self.core.mouse_hit_test {
            self.core.mouse_hit_test = hit_test;
            self.reset_cursor_style(cx);
        }

        #[cfg(any(feature = "inspector", debug_assertions))]
        if self.is_inspector_picking(cx) {
            self.handle_inspector_mouse_event(event, cx);
            // When inspector is picking, all other mouse handling is skipped.
            return;
        }

        let mut mouse_listeners = mem::take(&mut self.core.rendered_frame.mouse_listeners);

        // Capture phase, events bubble from back to front. Handlers for this phase are used for
        // special purposes, such as detecting events outside of a given Bounds.
        for listener in &mut mouse_listeners {
            let listener = listener.as_mut().unwrap();
            listener(event, DispatchPhase::Capture, self, cx);
            if !cx.propagate_event {
                break;
            }
        }

        // Bubble phase, where most normal handlers do their work.
        if cx.propagate_event {
            for listener in mouse_listeners.iter_mut().rev() {
                let listener = listener.as_mut().unwrap();
                listener(event, DispatchPhase::Bubble, self, cx);
                if !cx.propagate_event {
                    break;
                }
            }
        }

        self.core.rendered_frame.mouse_listeners = mouse_listeners;

        if cx.has_active_drag() {
            if event.is::<MouseMoveEvent>() {
                // If this was a mouse move event, redraw the window so that the
                // active drag can follow the mouse cursor.
                self.refresh();
            } else if event.is::<MouseUpEvent>() {
                // If this was a mouse up event, cancel the active drag and redraw
                // the window.
                cx.active_drag = None;
                self.refresh();
            }
        }

        // Auto-release pointer capture on mouse up
        if event.is::<MouseUpEvent>() && self.core.captured_hitbox.is_some() {
            self.core.captured_hitbox = None;
        }
    }

    fn dispatch_key_event(&mut self, event: &dyn Any, cx: &mut App) {
        if self.core.invalidator.is_dirty() {
            self.draw(cx).clear();
        }

        let node_id = self.focus_node_id_in_rendered_frame(self.core.focus);
        let dispatch_path = self.core.rendered_frame.dispatch_tree.dispatch_path(node_id);

        let mut keystroke: Option<Keystroke> = None;

        if let Some(event) = event.downcast_ref::<ModifiersChangedEvent>() {
            if event.modifiers.number_of_modifiers() == 0
                && self.core.pending_modifier.modifiers.number_of_modifiers() == 1
                && !self.core.pending_modifier.saw_keystroke
            {
                let key = match self.core.pending_modifier.modifiers {
                    modifiers if modifiers.shift => Some("shift"),
                    modifiers if modifiers.control => Some("control"),
                    modifiers if modifiers.alt => Some("alt"),
                    modifiers if modifiers.platform => Some("platform"),
                    modifiers if modifiers.function => Some("function"),
                    _ => None,
                };
                if let Some(key) = key {
                    keystroke = Some(Keystroke {
                        key: key.to_string(),
                        key_char: None,
                        modifiers: Modifiers::default(),
                    });
                }
            }

            if self.core.pending_modifier.modifiers.number_of_modifiers() == 0
                && event.modifiers.number_of_modifiers() == 1
            {
                self.core.pending_modifier.saw_keystroke = false
            }
            self.core.pending_modifier.modifiers = event.modifiers
        } else if let Some(key_down_event) = event.downcast_ref::<KeyDownEvent>() {
            self.core.pending_modifier.saw_keystroke = true;
            keystroke = Some(key_down_event.keystroke.clone());
        }

        let Some(keystroke) = keystroke else {
            self.finish_dispatch_key_event(event, dispatch_path, self.context_stack(), cx);
            return;
        };

        cx.propagate_event = true;
        self.dispatch_keystroke_interceptors(event, self.context_stack(), cx);
        if !cx.propagate_event {
            self.finish_dispatch_key_event(event, dispatch_path, self.context_stack(), cx);
            return;
        }

        let mut currently_pending = self.core.pending_input.take().unwrap_or_default();
        if currently_pending.focus.is_some() && currently_pending.focus != self.core.focus {
            currently_pending = PendingInput::default();
        }

        let match_result = self.core.rendered_frame.dispatch_tree.dispatch_key(
            currently_pending.keystrokes,
            keystroke,
            &dispatch_path,
        );

        if !match_result.to_replay.is_empty() {
            self.replay_pending_input(match_result.to_replay, cx);
            cx.propagate_event = true;
        }

        if !match_result.pending.is_empty() {
            currently_pending.timer.take();
            currently_pending.keystrokes = match_result.pending;
            currently_pending.focus = self.core.focus;

            let text_input_requires_timeout = event
                .downcast_ref::<KeyDownEvent>()
                .filter(|key_down| key_down.keystroke.key_char.is_some())
                .and_then(|_| self.core.platform_window.take_input_handler())
                .map_or(false, |mut input_handler| {
                    let accepts = input_handler.accepts_text_input(self, cx);
                    self.core.platform_window.set_input_handler(input_handler);
                    accepts
                });

            currently_pending.needs_timeout |=
                match_result.pending_has_binding || text_input_requires_timeout;

            if currently_pending.needs_timeout {
                currently_pending.timer = Some(self.spawn(cx, async move |cx| {
                    cx.background_executor.timer(Duration::from_secs(1)).await;
                    cx.update(move |window, cx| {
                        let Some(currently_pending) = window
                            .core.pending_input
                            .take()
                            .filter(|pending| pending.focus == window.core.focus)
                        else {
                            return;
                        };

                        let node_id = window.focus_node_id_in_rendered_frame(window.core.focus);
                        let dispatch_path =
                            window.core.rendered_frame.dispatch_tree.dispatch_path(node_id);

                        let to_replay = window
                            .core.rendered_frame
                            .dispatch_tree
                            .flush_dispatch(currently_pending.keystrokes, &dispatch_path);

                        window.pending_input_changed(cx);
                        window.replay_pending_input(to_replay, cx)
                    })
                    .log_err();
                }));
            } else {
                currently_pending.timer = None;
            }
            self.core.pending_input = Some(currently_pending);
            self.pending_input_changed(cx);
            cx.propagate_event = false;
            return;
        }

        let skip_bindings = event
            .downcast_ref::<KeyDownEvent>()
            .filter(|key_down_event| key_down_event.prefer_character_input)
            .map(|_| {
                self.core.platform_window
                    .take_input_handler()
                    .map_or(false, |mut input_handler| {
                        let accepts = input_handler.accepts_text_input(self, cx);
                        self.core.platform_window.set_input_handler(input_handler);
                        // If modifiers are not excessive (e.g. AltGr), and the input handler is accepting text input,
                        // we prefer the text input over bindings.
                        accepts
                    })
            })
            .unwrap_or(false);

        if !skip_bindings {
            for binding in match_result.bindings {
                self.dispatch_action_on_node(node_id, binding.action.as_ref(), cx);
                if !cx.propagate_event {
                    self.dispatch_keystroke_observers(
                        event,
                        Some(binding.action),
                        match_result.context_stack,
                        cx,
                    );
                    self.pending_input_changed(cx);
                    return;
                }
            }
        }

        self.finish_dispatch_key_event(event, dispatch_path, match_result.context_stack, cx);
        self.pending_input_changed(cx);
    }

    fn finish_dispatch_key_event(
        &mut self,
        event: &dyn Any,
        dispatch_path: SmallVec<[DispatchNodeId; 32]>,
        context_stack: Vec<KeyContext>,
        cx: &mut App,
    ) {
        self.dispatch_key_down_up_event(event, &dispatch_path, cx);
        if !cx.propagate_event {
            return;
        }

        self.dispatch_modifiers_changed_event(event, &dispatch_path, cx);
        if !cx.propagate_event {
            return;
        }

        self.dispatch_keystroke_observers(event, None, context_stack, cx);
    }

    pub(crate) fn pending_input_changed(&mut self, cx: &mut App) {
        self.core.pending_input_observers
            .clone()
            .retain(&(), |callback| callback(self, cx));
    }

    fn dispatch_key_down_up_event(
        &mut self,
        event: &dyn Any,
        dispatch_path: &SmallVec<[DispatchNodeId; 32]>,
        cx: &mut App,
    ) {
        // Capture phase
        for node_id in dispatch_path {
            let node = self.core.rendered_frame.dispatch_tree.node(*node_id);

            for key_listener in node.key_listeners.clone() {
                key_listener(event, DispatchPhase::Capture, self, cx);
                if !cx.propagate_event {
                    return;
                }
            }
        }

        // Bubble phase
        for node_id in dispatch_path.iter().rev() {
            // Handle low level key events
            let node = self.core.rendered_frame.dispatch_tree.node(*node_id);
            for key_listener in node.key_listeners.clone() {
                key_listener(event, DispatchPhase::Bubble, self, cx);
                if !cx.propagate_event {
                    return;
                }
            }
        }
    }

    fn dispatch_modifiers_changed_event(
        &mut self,
        event: &dyn Any,
        dispatch_path: &SmallVec<[DispatchNodeId; 32]>,
        cx: &mut App,
    ) {
        let Some(event) = event.downcast_ref::<ModifiersChangedEvent>() else {
            return;
        };
        for node_id in dispatch_path.iter().rev() {
            let node = self.core.rendered_frame.dispatch_tree.node(*node_id);
            for listener in node.modifiers_changed_listeners.clone() {
                listener(event, self, cx);
                if !cx.propagate_event {
                    return;
                }
            }
        }
    }

    /// Determine whether a potential multi-stroke key binding is in progress on this window.
    pub fn has_pending_keystrokes(&self) -> bool {
        self.core.pending_input.is_some()
    }

    pub(crate) fn clear_pending_keystrokes(&mut self) {
        self.core.pending_input.take();
    }

    /// Returns the currently pending input keystrokes that might result in a multi-stroke key binding.
    pub fn pending_input_keystrokes(&self) -> Option<&[Keystroke]> {
        self.core.pending_input
            .as_ref()
            .map(|pending_input| pending_input.keystrokes.as_slice())
    }

    fn replay_pending_input(&mut self, replays: SmallVec<[Replay; 1]>, cx: &mut App) {
        let node_id = self.focus_node_id_in_rendered_frame(self.core.focus);
        let dispatch_path = self.core.rendered_frame.dispatch_tree.dispatch_path(node_id);

        'replay: for replay in replays {
            let event = KeyDownEvent {
                keystroke: replay.keystroke.clone(),
                is_held: false,
                prefer_character_input: true,
            };

            cx.propagate_event = true;
            for binding in replay.bindings {
                self.dispatch_action_on_node(node_id, binding.action.as_ref(), cx);
                if !cx.propagate_event {
                    self.dispatch_keystroke_observers(
                        &event,
                        Some(binding.action),
                        Vec::default(),
                        cx,
                    );
                    continue 'replay;
                }
            }

            self.dispatch_key_down_up_event(&event, &dispatch_path, cx);
            if !cx.propagate_event {
                continue 'replay;
            }
            if let Some(input) = replay.keystroke.key_char.as_ref().cloned()
                && let Some(mut input_handler) = self.core.platform_window.take_input_handler()
            {
                input_handler.dispatch_input(&input, self, cx);
                self.core.platform_window.set_input_handler(input_handler)
            }
        }
    }

    fn focus_node_id_in_rendered_frame(&self, focus_id: Option<FocusId>) -> DispatchNodeId {
        focus_id
            .and_then(|focus_id| {
                self.core.rendered_frame
                    .dispatch_tree
                    .focusable_node_id(focus_id)
            })
            .unwrap_or_else(|| self.core.rendered_frame.dispatch_tree.root_node_id())
    }

    fn dispatch_action_on_node(
        &mut self,
        node_id: DispatchNodeId,
        action: &dyn Action,
        cx: &mut App,
    ) {
        let dispatch_path = self.core.rendered_frame.dispatch_tree.dispatch_path(node_id);

        // Capture phase for global actions.
        cx.propagate_event = true;
        if let Some(mut global_listeners) = cx
            .global_action_listeners
            .remove(&action.as_any().type_id())
        {
            for listener in &global_listeners {
                listener(action.as_any(), DispatchPhase::Capture, cx);
                if !cx.propagate_event {
                    break;
                }
            }

            global_listeners.extend(
                cx.global_action_listeners
                    .remove(&action.as_any().type_id())
                    .unwrap_or_default(),
            );

            cx.global_action_listeners
                .insert(action.as_any().type_id(), global_listeners);
        }

        if !cx.propagate_event {
            return;
        }

        // Capture phase for window actions.
        for node_id in &dispatch_path {
            let node = self.core.rendered_frame.dispatch_tree.node(*node_id);
            for DispatchActionListener {
                action_type,
                listener,
            } in node.action_listeners.clone()
            {
                let any_action = action.as_any();
                if action_type == any_action.type_id() {
                    listener(any_action, DispatchPhase::Capture, self, cx);

                    if !cx.propagate_event {
                        return;
                    }
                }
            }
        }

        // Bubble phase for window actions.
        for node_id in dispatch_path.iter().rev() {
            let node = self.core.rendered_frame.dispatch_tree.node(*node_id);
            for DispatchActionListener {
                action_type,
                listener,
            } in node.action_listeners.clone()
            {
                let any_action = action.as_any();
                if action_type == any_action.type_id() {
                    cx.propagate_event = false; // Actions stop propagation by default during the bubble phase
                    listener(any_action, DispatchPhase::Bubble, self, cx);

                    if !cx.propagate_event {
                        return;
                    }
                }
            }
        }

        // Bubble phase for global actions.
        if let Some(mut global_listeners) = cx
            .global_action_listeners
            .remove(&action.as_any().type_id())
        {
            for listener in global_listeners.iter().rev() {
                cx.propagate_event = false; // Actions stop propagation by default during the bubble phase

                listener(action.as_any(), DispatchPhase::Bubble, cx);
                if !cx.propagate_event {
                    break;
                }
            }

            global_listeners.extend(
                cx.global_action_listeners
                    .remove(&action.as_any().type_id())
                    .unwrap_or_default(),
            );

            cx.global_action_listeners
                .insert(action.as_any().type_id(), global_listeners);
        }
    }

    /// Register the given handler to be invoked whenever the global of the given type
    /// is updated.
    pub fn observe_global<G: Global>(
        &mut self,
        cx: &mut App,
        f: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Subscription {
        let window_handle = self.core.handle;
        let (subscription, activate) = cx.global_observers.insert(
            TypeId::of::<G>(),
            Box::new(move |cx| {
                window_handle
                    .update(cx, |_, window, cx| f(window, cx))
                    .is_ok()
            }),
        );
        cx.defer(move |_| activate());
        subscription
    }

    /// Focus the current window and bring it to the foreground at the platform level.
    pub fn activate_window(&self) {
        self.core.platform_window.activate();
    }

    /// ADR-008: query the `WindowOptions::is_movable` invariant the
    /// window was created with. Callers can branch on this before
    /// programmatic move attempts; the engine also rejects platform-side
    /// move gestures when `false` (Windows `WM_SYSCOMMAND::SC_MOVE`
    /// filter; macOS `NSWindow.setMovable_:` first line; system-menu /
    /// titlebar drag second line is TODO per ADR-008 #2/#3).
    pub fn is_movable(&self) -> bool {
        self.core.is_movable
    }

    /// ADR-008: query the `WindowOptions::is_resizable` invariant. Also
    /// implies `is_maximizable = false` per decision 3 (Cocoa and Win32
    /// both conflate the two via `NSResizableWindowMask` and
    /// `WS_THICKFRAME`/`WS_MAXIMIZEBOX`).
    pub fn is_resizable(&self) -> bool {
        self.core.is_resizable
    }

    /// ADR-008: query the `WindowOptions::is_minimizable` invariant.
    /// `minimize_window` is the gated programmatic path on this Window.
    pub fn is_minimizable(&self) -> bool {
        self.core.is_minimizable
    }

    /// Minimize the current window at the platform level.
    ///
    /// ADR-008 decision 6: gated on the `WindowOptions::is_minimizable`
    /// invariant. If the window was created with `is_minimizable = false`,
    /// this is a no-op and emits a `log::warn!`. Callers that need to
    /// observe rejection without polling can wrap this in a higher-level
    /// API that returns `Result`.
    pub fn minimize_window(&self) {
        if !self.core.is_minimizable {
            log::warn!(
                "ADR-008: ignored programmatic minimize_window() on a window \
                 created with `is_minimizable = false`. The invariant is \
                 binding on programmatic callers — wrap the call in a flag \
                 check or change `WindowOptions::is_minimizable` to true \
                 if minimization should be permitted."
            );
            return;
        }
        self.core.platform_window.minimize();
    }

    /// Toggle full screen status on the current window at the platform level.
    pub fn toggle_fullscreen(&self) {
        self.core.platform_window.toggle_fullscreen();
    }

    /// Updates the IME panel position suggestions for languages like japanese, chinese.
    pub fn invalidate_character_coordinates(&self) {
        self.on_pre_frame(|window, cx| {
            if let Some(mut input_handler) = window.core.platform_window.take_input_handler() {
                if let Some(bounds) = input_handler.selected_bounds(window, cx) {
                    window.core.platform_window.update_ime_position(bounds);
                }
                window.core.platform_window.set_input_handler(input_handler);
            }
        });
    }

    /// Present a platform dialog.
    /// The provided message will be presented, along with buttons for each answer.
    /// When a button is clicked, the returned `Receiver` will receive the index of the clicked button.
    ///
    /// Returns `Err(ReentryError::PromptInProgress)` if another prompt is
    /// already awaiting the user's response — wait for the previous prompt's
    /// `Receiver` to complete before opening a new one. K15 (Phase 0-K) made
    /// this case structured: prior to K15 it produced an `unreachable!`
    /// panic with no machine-readable type.
    pub fn prompt<T>(
        &mut self,
        level: PromptLevel,
        message: &str,
        detail: Option<&str>,
        answers: &[T],
        cx: &mut App,
    ) -> Result<oneshot::Receiver<usize>, crate::reentrancy::ReentryError>
    where
        T: Clone + Into<PromptButton>,
    {
        let prompt_builder = cx.prompt_builder.take();
        let Some(prompt_builder) = prompt_builder else {
            // K15: replace `unreachable!` with structured `ReentryError`.
            return Err(crate::reentrancy::ReentryError::PromptInProgress);
        };

        let answers = answers
            .iter()
            .map(|answer| answer.clone().into())
            .collect::<Vec<_>>();

        let receiver = match &prompt_builder {
            PromptBuilder::Default => self
                .core.platform_window
                .prompt(level, message, detail, &answers)
                .unwrap_or_else(|| {
                    self.build_custom_prompt(&prompt_builder, level, message, detail, &answers, cx)
                }),
            PromptBuilder::Custom(_) => {
                self.build_custom_prompt(&prompt_builder, level, message, detail, &answers, cx)
            }
        };

        cx.prompt_builder = Some(prompt_builder);

        Ok(receiver)
    }

    fn build_custom_prompt(
        &mut self,
        prompt_builder: &PromptBuilder,
        level: PromptLevel,
        message: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
        cx: &mut App,
    ) -> oneshot::Receiver<usize> {
        let (sender, receiver) = oneshot::channel();
        let handle = PromptHandle::new(sender);
        let handle = (prompt_builder)(level, message, detail, answers, handle, self, cx);
        self.core.prompt = Some(handle);
        receiver
    }

    /// Returns the current context stack.
    pub fn context_stack(&self) -> Vec<KeyContext> {
        let node_id = self.focus_node_id_in_rendered_frame(self.core.focus);
        let dispatch_tree = &self.core.rendered_frame.dispatch_tree;
        dispatch_tree
            .dispatch_path(node_id)
            .iter()
            .filter_map(move |&node_id| dispatch_tree.node(node_id).context.clone())
            .collect()
    }

    /// Returns all available actions for the focused element.
    pub fn available_actions(&self, cx: &App) -> Vec<Box<dyn Action>> {
        let node_id = self.focus_node_id_in_rendered_frame(self.core.focus);
        let mut actions = self.core.rendered_frame.dispatch_tree.available_actions(node_id);
        for action_type in cx.global_action_listeners.keys() {
            if let Err(ix) = actions.binary_search_by_key(action_type, |a| a.as_any().type_id()) {
                let action = cx.actions.build_action_type(action_type).ok();
                if let Some(action) = action {
                    actions.insert(ix, action);
                }
            }
        }
        actions
    }

    /// Returns key bindings that invoke an action on the currently focused element. Bindings are
    /// returned in the order they were added. For display, the last binding should take precedence.
    pub fn bindings_for_action(&self, action: &dyn Action) -> Vec<KeyBinding> {
        self.core.rendered_frame
            .dispatch_tree
            .bindings_for_action(action, &self.core.rendered_frame.dispatch_tree.context_stack)
    }

    /// Returns the highest precedence key binding that invokes an action on the currently focused
    /// element. This is more efficient than getting the last result of `bindings_for_action`.
    pub fn highest_precedence_binding_for_action(&self, action: &dyn Action) -> Option<KeyBinding> {
        self.core.rendered_frame
            .dispatch_tree
            .highest_precedence_binding_for_action(
                action,
                &self.core.rendered_frame.dispatch_tree.context_stack,
            )
    }

    /// Returns the key bindings for an action in a context.
    pub fn bindings_for_action_in_context(
        &self,
        action: &dyn Action,
        context: KeyContext,
    ) -> Vec<KeyBinding> {
        let dispatch_tree = &self.core.rendered_frame.dispatch_tree;
        dispatch_tree.bindings_for_action(action, &[context])
    }

    /// Returns the highest precedence key binding for an action in a context. This is more
    /// efficient than getting the last result of `bindings_for_action_in_context`.
    pub fn highest_precedence_binding_for_action_in_context(
        &self,
        action: &dyn Action,
        context: KeyContext,
    ) -> Option<KeyBinding> {
        let dispatch_tree = &self.core.rendered_frame.dispatch_tree;
        dispatch_tree.highest_precedence_binding_for_action(action, &[context])
    }

    /// Returns any bindings that would invoke an action on the given focus handle if it were
    /// focused. Bindings are returned in the order they were added. For display, the last binding
    /// should take precedence.
    pub fn bindings_for_action_in(
        &self,
        action: &dyn Action,
        focus_handle: &FocusHandle,
    ) -> Vec<KeyBinding> {
        let dispatch_tree = &self.core.rendered_frame.dispatch_tree;
        let Some(context_stack) = self.context_stack_for_focus_handle(focus_handle) else {
            return vec![];
        };
        dispatch_tree.bindings_for_action(action, &context_stack)
    }

    /// Returns the highest precedence key binding that would invoke an action on the given focus
    /// handle if it were focused. This is more efficient than getting the last result of
    /// `bindings_for_action_in`.
    pub fn highest_precedence_binding_for_action_in(
        &self,
        action: &dyn Action,
        focus_handle: &FocusHandle,
    ) -> Option<KeyBinding> {
        let dispatch_tree = &self.core.rendered_frame.dispatch_tree;
        let context_stack = self.context_stack_for_focus_handle(focus_handle)?;
        dispatch_tree.highest_precedence_binding_for_action(action, &context_stack)
    }

    /// Find the bindings that can follow the current input sequence for the current context stack.
    pub fn possible_bindings_for_input(&self, input: &[Keystroke]) -> Vec<KeyBinding> {
        self.core.rendered_frame
            .dispatch_tree
            .possible_next_bindings_for_input(input, &self.context_stack())
    }

    fn context_stack_for_focus_handle(
        &self,
        focus_handle: &FocusHandle,
    ) -> Option<Vec<KeyContext>> {
        let dispatch_tree = &self.core.rendered_frame.dispatch_tree;
        let node_id = dispatch_tree.focusable_node_id(focus_handle.id)?;
        let context_stack: Vec<_> = dispatch_tree
            .dispatch_path(node_id)
            .into_iter()
            .filter_map(|node_id| dispatch_tree.node(node_id).context.clone())
            .collect();
        Some(context_stack)
    }

    /// Returns a generic event listener that invokes the given listener with the view and context associated with the given view handle.
    pub fn listener_for<T: 'static, E>(
        &self,
        view: &Entity<T>,
        f: impl Fn(&mut T, &E, &mut Window, &mut Context<T>) + 'static,
    ) -> impl Fn(&E, &mut Window, &mut App) + 'static {
        let view = view.downgrade();
        move |e: &E, window: &mut Window, cx: &mut App| {
            view.update(cx, |view, cx| f(view, e, window, cx)).ok();
        }
    }

    /// Returns a generic handler that invokes the given handler with the view and context associated with the given view handle.
    pub fn handler_for<E: 'static, Callback: Fn(&mut E, &mut Window, &mut Context<E>) + 'static>(
        &self,
        entity: &Entity<E>,
        f: Callback,
    ) -> impl Fn(&mut Window, &mut App) + 'static {
        let entity = entity.downgrade();
        move |window: &mut Window, cx: &mut App| {
            entity.update(cx, |entity, cx| f(entity, window, cx)).ok();
        }
    }

    /// Register a callback that can interrupt the closing of the current window based the returned boolean.
    /// If the callback returns false, the window won't be closed.
    pub fn on_window_should_close(
        &self,
        cx: &App,
        f: impl Fn(&mut Window, &mut App) -> bool + 'static,
    ) {
        let mut cx = self.to_async(cx);
        self.core.platform_window.on_should_close(Box::new(move || {
            cx.update(|window, cx| f(window, cx)).unwrap_or(true)
        }))
    }

    /// Register an action listener on this node for the next frame. The type of action
    /// is determined by the first parameter of the given listener. When the next frame is rendered
    /// the listener will be cleared.
    ///
    /// This is a fairly low-level method, so prefer using action handlers on elements unless you have
    /// a specific need to register a listener yourself.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn on_action(
        &mut self,
        action_type: TypeId,
        listener: impl Fn(&dyn Any, DispatchPhase, &mut Window, &mut App) + 'static,
    ) {
        self.core.invalidator.debug_assert_paint();

        self.core.next_frame
            .dispatch_tree
            .on_action(action_type, Rc::new(listener));
    }

    /// Register a capturing action listener on this node for the next frame if the condition is true.
    /// The type of action is determined by the first parameter of the given listener. When the next
    /// frame is rendered the listener will be cleared.
    ///
    /// This is a fairly low-level method, so prefer using action handlers on elements unless you have
    /// a specific need to register a listener yourself.
    ///
    /// This method should only be called as part of the paint phase of element drawing.
    pub fn on_action_when(
        &mut self,
        condition: bool,
        action_type: TypeId,
        listener: impl Fn(&dyn Any, DispatchPhase, &mut Window, &mut App) + 'static,
    ) {
        self.core.invalidator.debug_assert_paint();

        if condition {
            self.core.next_frame
                .dispatch_tree
                .on_action(action_type, Rc::new(listener));
        }
    }

    /// Read information about the GPU backing this window.
    /// Currently returns None on Mac and Windows.
    pub fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.core.platform_window.gpu_specs()
    }

    /// Perform titlebar double-click action.
    /// This is macOS specific.
    pub fn titlebar_double_click(&self) {
        self.core.platform_window.titlebar_double_click();
    }

    /// Gets the window's title at the platform level.
    /// This is macOS specific.
    pub fn window_title(&self) -> String {
        self.core.platform_window.get_title()
    }

    /// Returns a list of all tabbed windows and their titles.
    /// This is macOS specific.
    pub fn tabbed_windows(&self) -> Option<Vec<SystemWindowTab>> {
        self.core.platform_window.tabbed_windows()
    }

    /// Returns the tab bar visibility.
    /// This is macOS specific.
    pub fn tab_bar_visible(&self) -> bool {
        self.core.platform_window.tab_bar_visible()
    }

    /// Merges all open windows into a single tabbed window.
    /// This is macOS specific.
    pub fn merge_all_windows(&self) {
        self.core.platform_window.merge_all_windows()
    }

    /// Moves the tab to a new containing window.
    /// This is macOS specific.
    pub fn move_tab_to_new_window(&self) {
        self.core.platform_window.move_tab_to_new_window()
    }

    /// Shows or hides the window tab overview.
    /// This is macOS specific.
    pub fn toggle_window_tab_overview(&self) {
        self.core.platform_window.toggle_window_tab_overview()
    }

    /// Sets the tabbing identifier for the window.
    /// This is macOS specific.
    pub fn set_tabbing_identifier(&self, tabbing_identifier: Option<String>) {
        self.core.platform_window
            .set_tabbing_identifier(tabbing_identifier)
    }

    /// Toggles the inspector mode on this window.
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub fn toggle_inspector(&mut self, cx: &mut App) {
        self.core.inspector = match self.core.inspector {
            None => Some(cx.new(|_| Inspector::new())),
            Some(_) => None,
        };
        self.refresh();
    }

    /// Returns true if the window is in inspector mode.
    pub fn is_inspector_picking(&self, _cx: &App) -> bool {
        #[cfg(any(feature = "inspector", debug_assertions))]
        {
            if let Some(inspector) = &self.core.inspector {
                return inspector.read(_cx).is_picking();
            }
        }
        false
    }

    /// Executes the provided function with mutable access to an inspector state.
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub fn with_inspector_state<T: 'static, R>(
        &mut self,
        _inspector_id: Option<&crate::InspectorElementId>,
        cx: &mut App,
        f: impl FnOnce(&mut Option<T>, &mut Self) -> R,
    ) -> R {
        if let Some(inspector_id) = _inspector_id
            && let Some(inspector) = &self.core.inspector
        {
            let inspector = inspector.clone();
            let active_element_id = inspector.read(cx).active_element_id();
            if Some(inspector_id) == active_element_id {
                return inspector.update(cx, |inspector, _cx| {
                    inspector.with_active_element_state(self, f)
                });
            }
        }
        f(&mut None, self)
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(crate) fn build_inspector_element_id(
        &mut self,
        path: crate::InspectorElementPath,
    ) -> crate::InspectorElementId {
        self.core.invalidator.debug_assert_paint_or_prepaint();
        let path = Rc::new(path);
        let next_instance_id = self
            .core.next_frame
            .next_inspector_instance_ids
            .entry(path.clone())
            .or_insert(0);
        let instance_id = *next_instance_id;
        *next_instance_id += 1;
        crate::InspectorElementId { path, instance_id }
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn prepaint_inspector(&mut self, inspector_width: Pixels, cx: &mut App) -> Option<AnyElement> {
        if let Some(inspector) = self.core.inspector.take() {
            let mut inspector_element = AnyView::from(inspector.clone()).into_any_element();
            inspector_element.prepaint_as_root_with_window(
                point(self.core.viewport_size.width - inspector_width, px(0.0)),
                size(inspector_width, self.core.viewport_size.height).into(),
                self,
                cx,
            );
            self.core.inspector = Some(inspector);
            Some(inspector_element)
        } else {
            None
        }
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn paint_inspector(&mut self, mut inspector_element: Option<AnyElement>, cx: &mut App) {
        if let Some(mut inspector_element) = inspector_element {
            inspector_element.paint_with_window(self, cx);
        };
    }

    /// Registers a hitbox that can be used for inspector picking mode, allowing users to select and
    /// inspect UI elements by clicking on them.
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub fn insert_inspector_hitbox(
        &mut self,
        hitbox_id: HitboxId,
        inspector_id: Option<&crate::InspectorElementId>,
        cx: &App,
    ) {
        self.core.invalidator.debug_assert_paint_or_prepaint();
        if !self.is_inspector_picking(cx) {
            return;
        }
        if let Some(inspector_id) = inspector_id {
            self.core.next_frame
                .inspector_hitboxes
                .insert(hitbox_id, inspector_id.clone());
        }
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn paint_inspector_hitbox(&mut self, cx: &App) {
        if let Some(inspector) = self.core.inspector.as_ref() {
            let inspector = inspector.read(cx);
            if let Some((hitbox_id, _)) = self.hovered_inspector_hitbox(inspector, &self.core.next_frame)
                && let Some(hitbox) = self
                    .core.next_frame
                    .hitboxes
                    .iter()
                    .find(|hitbox| hitbox.id == hitbox_id)
            {
                self.paint_quad(crate::fill(hitbox.bounds, crate::rgba(0x61afef4d)));
            }
        }
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn handle_inspector_mouse_event(&mut self, event: &dyn Any, cx: &mut App) {
        let Some(inspector) = self.core.inspector.clone() else {
            return;
        };
        if event.downcast_ref::<MouseMoveEvent>().is_some() {
            inspector.update(cx, |inspector, _cx| {
                if let Some((_, inspector_id)) =
                    self.hovered_inspector_hitbox(inspector, &self.core.rendered_frame)
                {
                    inspector.hover(inspector_id, self);
                }
            });
        } else if event.downcast_ref::<crate::MouseDownEvent>().is_some() {
            inspector.update(cx, |inspector, _cx| {
                if let Some((_, inspector_id)) =
                    self.hovered_inspector_hitbox(inspector, &self.core.rendered_frame)
                {
                    inspector.select(inspector_id, self);
                }
            });
        } else if let Some(event) = event.downcast_ref::<crate::ScrollWheelEvent>() {
            // This should be kept in sync with SCROLL_LINES in x11 platform.
            const SCROLL_LINES: f32 = 3.0;
            const SCROLL_PIXELS_PER_LAYER: f32 = 36.0;
            let delta_y = event
                .delta
                .pixel_delta(px(SCROLL_PIXELS_PER_LAYER / SCROLL_LINES))
                .y;
            if let Some(inspector) = self.core.inspector.clone() {
                inspector.update(cx, |inspector, _cx| {
                    if let Some(depth) = inspector.pick_depth.as_mut() {
                        *depth += f32::from(delta_y) / SCROLL_PIXELS_PER_LAYER;
                        let max_depth = self.core.mouse_hit_test.ids.len() as f32 - 0.5;
                        if *depth < 0.0 {
                            *depth = 0.0;
                        } else if *depth > max_depth {
                            *depth = max_depth;
                        }
                        if let Some((_, inspector_id)) =
                            self.hovered_inspector_hitbox(inspector, &self.core.rendered_frame)
                        {
                            inspector.set_active_element_id(inspector_id, self);
                        }
                    }
                });
            }
        }
    }

    #[cfg(any(feature = "inspector", debug_assertions))]
    fn hovered_inspector_hitbox(
        &self,
        inspector: &Inspector,
        frame: &Frame,
    ) -> Option<(HitboxId, crate::InspectorElementId)> {
        if let Some(pick_depth) = inspector.pick_depth {
            let depth = (pick_depth as i64).try_into().unwrap_or(0);
            let max_skipped = self.core.mouse_hit_test.ids.len().saturating_sub(1);
            let skip_count = (depth as usize).min(max_skipped);
            for hitbox_id in self.core.mouse_hit_test.ids.iter().skip(skip_count) {
                if let Some(inspector_id) = frame.inspector_hitboxes.get(hitbox_id) {
                    return Some((*hitbox_id, inspector_id.clone()));
                }
            }
        }
        None
    }

    /// For testing: set the current modifier keys state.
    /// This does not generate any events.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_modifiers(&mut self, modifiers: Modifiers) {
        self.core.modifiers = modifiers;
    }

    /// For testing: simulate a mouse move event to the given position.
    /// This dispatches the event through the normal event handling path,
    /// which will trigger hover states and tooltips.
    #[cfg(any(test, feature = "test-support"))]
    pub fn simulate_mouse_move(&mut self, position: Point<Pixels>, cx: &mut App) {
        let event = PlatformInput::MouseMove(MouseMoveEvent {
            position,
            modifiers: self.core.modifiers,
            pressed_button: None,
        });
        let _ = self.dispatch_event(event, cx);
    }
}

/// A rectangle to be rendered in the window at the given position and size.
/// Passed as an argument [`Window::paint_quad`].
#[derive(Clone)]
pub struct PaintQuad {
    /// The bounds of the quad within the window.
    pub bounds: Bounds<Pixels>,
    /// The radii of the quad's corners.
    pub corner_radii: Corners<Pixels>,
    /// The background color of the quad.
    pub background: Background,
    /// The widths of the quad's borders.
    pub border_widths: Edges<Pixels>,
    /// The color of the quad's borders.
    pub border_color: Hsla,
    /// The style of the quad's borders.
    pub border_style: BorderStyle,
}

impl PaintQuad {
    /// Sets the corner radii of the quad.
    pub fn corner_radii(self, corner_radii: impl Into<Corners<Pixels>>) -> Self {
        PaintQuad {
            corner_radii: corner_radii.into(),
            ..self
        }
    }

    /// Sets the border widths of the quad.
    pub fn border_widths(self, border_widths: impl Into<Edges<Pixels>>) -> Self {
        PaintQuad {
            border_widths: border_widths.into(),
            ..self
        }
    }

    /// Sets the border color of the quad.
    pub fn border_color(self, border_color: impl Into<Hsla>) -> Self {
        PaintQuad {
            border_color: border_color.into(),
            ..self
        }
    }

    /// Sets the background color of the quad.
    pub fn background(self, background: impl Into<Background>) -> Self {
        PaintQuad {
            background: background.into(),
            ..self
        }
    }
}

/// Creates a quad with the given parameters.
pub fn quad(
    bounds: Bounds<Pixels>,
    corner_radii: impl Into<Corners<Pixels>>,
    background: impl Into<Background>,
    border_widths: impl Into<Edges<Pixels>>,
    border_color: impl Into<Hsla>,
    border_style: BorderStyle,
) -> PaintQuad {
    PaintQuad {
        bounds,
        corner_radii: corner_radii.into(),
        background: background.into(),
        border_widths: border_widths.into(),
        border_color: border_color.into(),
        border_style,
    }
}

/// Creates a filled quad with the given bounds and background color.
pub fn fill(bounds: impl Into<Bounds<Pixels>>, background: impl Into<Background>) -> PaintQuad {
    PaintQuad {
        bounds: bounds.into(),
        corner_radii: (0.).into(),
        background: background.into(),
        border_widths: (0.).into(),
        border_color: transparent_black(),
        border_style: BorderStyle::default(),
    }
}

/// Creates a rectangle outline with the given bounds, border color, and a 1px border width
pub fn outline(
    bounds: impl Into<Bounds<Pixels>>,
    border_color: impl Into<Hsla>,
    border_style: BorderStyle,
) -> PaintQuad {
    PaintQuad {
        bounds: bounds.into(),
        corner_radii: (0.).into(),
        background: transparent_black().into(),
        border_widths: (1.).into(),
        border_color: border_color.into(),
        border_style,
    }
}
