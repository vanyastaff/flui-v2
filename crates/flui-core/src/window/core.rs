//! Private state container for [`Window`](super::Window) — A10a PR 1.0 foundation.
//!
//! See spec: `docs/superpowers/specs/2026-05-13-A10-xl-file-split-design.md`.
//! Policy: `docs/research/adr/ADR-021-xl-file-split-discipline.md` Practice 1.
//!
//! # Purpose
//!
//! `WindowCore` is the private struct that owns the fields of [`Window`]. It exists so
//! future sibling submodules under `window/` (focus, hitbox, draw, layout, event_dispatch,
//! etc., landing in PRs 1.3-1.11) can be split across files without each new file gaining
//! direct read/write access to every other cluster's private state. Sibling modules see
//! only what `pub(super)` exposes here; the rest of the crate continues to reach `Window`
//! via its existing public API.
//!
//! # Contract (binding — ADR-021 Practice 1)
//!
//! - **Visibility:** `pub(super) struct WindowCore`. Never `pub`, never `pub(crate)`.
//! - **Embedding:** [`Window`] holds `pub(super) core: WindowCore` as a **plain field**.
//!   - `impl Deref<Target = WindowCore> for Window` is **prohibited** — auto-deref would
//!     leak `WindowCore`'s method-resolution surface to any caller holding `&Window`
//!     (even outside `window/`), defeating the `pub(super)` boundary. Audited by
//!     `rust-api-migration-auditor` on 2026-05-13.
//!   - `Box<WindowCore>` / `Arc<WindowCore>` / `Rc<WindowCore>` are **prohibited**. Several
//!     `Rc<Cell<bool>>` / `Rc<RefCell<...>>` fields inside `WindowCore` (`active`,
//!     `needs_present`, `input_rate_tracker`, ...) are cloned and shared with platform
//!     callbacks; the wrapper layout must not relocate them across the heap, otherwise
//!     `Rc::ptr_eq` comparisons in platform code silently break.
//! - **Field access from `impl Window` blocks:** use `self.core.<field>` explicitly.
//! - **No new `pub` symbols:** PR 1.0 is API-neutral. Public API guarantees are verified
//!   via `cargo public-api diff` against `main`.

// Defensive note for future contributors: this module is named `core`, which shadows
// Rust's built-in `core` crate name inside this file's local scope. Macros emitted by
// `slotmap`, `serde`, `derive_more`, etc. use leading-`::` paths (`::core::option::Option`)
// and remain unaffected. If you need to reference the standard `core` crate from within
// `window/core.rs` (rare), use the absolute path `::core::*`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use collections::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use crate::provider::registry::InheritedRegistry;
use crate::{
    AnyImageCache, AnyView, AnyWindowHandle, Bounds, Capslock, ContentMask, DisplayId,
    ElementIdStack, EntityId, Modifiers, Pixels, PlatformAtlas, PlatformWindow, Point,
    RenderablePromptHandle, Size, SubscriberSet, TaffyLayoutEngine, TextRenderingMode,
    TextStyleRefinement, WindowAppearance, WindowTextSystem,
};

#[cfg(any(feature = "inspector", debug_assertions))]
use crate::{Entity, Inspector};

// Items defined in the parent `window.rs` (private to module `crate::window`,
// accessible to child `crate::window::core` per Rust's child-sees-parent privacy rule).
use super::{
    AnyObserver, AnyWindowFocusListener, FocusId, Frame, FrameCallback, HitTest, HitboxId,
    InputModality, InputRateTracker, ModifierState, PendingInput, TooltipBounds, TooltipId,
    WindowInvalidator,
};

/// Crate-internal state container holding all fields of [`Window`].
///
/// **PR 1.0 amendment to ADR-021 Practice 1**: this is `pub(crate)` (not `pub(super)`)
/// because callers in `crate::app`, `crate::view`, `crate::element`, etc. access
/// `Window`'s previously-`pub(crate)` fields. Re-tightening to `pub(super)` requires
/// introducing ~30 accessor methods on `Window`; deferred and tracked alongside K06's
/// ownership-shard redesign. Downstream crates still cannot name `WindowCore` because
/// the type is `pub(crate)`.
///
/// **DO NOT** make this `pub`. **DO NOT** add `impl Deref<Target = WindowCore>` for
/// `Window`. **DO NOT** wrap in `Box`/`Arc`/`Rc`. See module-level docs for rationale.
pub(crate) struct WindowCore {
    pub(crate) handle: AnyWindowHandle,
    pub(crate) invalidator: WindowInvalidator,
    pub(crate) removed: bool,
    pub(crate) platform_window: Box<dyn PlatformWindow>,
    pub(super) display_id: Option<DisplayId>,
    pub(super) sprite_atlas: Arc<dyn PlatformAtlas>,
    pub(super) text_system: Arc<WindowTextSystem>,
    pub(super) text_rendering_mode: Rc<Cell<TextRenderingMode>>,
    pub(super) rem_size: Pixels,
    /// The stack of override values for the window's rem size.
    ///
    /// This is used by `with_rem_size` to allow rendering an element tree with
    /// a given rem size.
    pub(super) rem_size_override_stack: SmallVec<[Pixels; 8]>,
    pub(crate) viewport_size: Size<Pixels>,
    pub(super) layout_engine: Option<TaffyLayoutEngine>,
    pub(crate) root: Option<AnyView>,
    pub(crate) element_id_stack: ElementIdStack,
    pub(crate) text_style_stack: Vec<TextStyleRefinement>,
    pub(crate) rendered_entity_stack: Vec<EntityId>,
    pub(crate) element_offset_stack: Vec<Point<Pixels>>,
    pub(crate) element_opacity: f32,
    pub(crate) content_mask_stack: Vec<ContentMask<Pixels>>,
    pub(crate) requested_autoscroll: Option<Bounds<Pixels>>,
    pub(crate) image_cache_stack: Vec<AnyImageCache>,
    pub(crate) inherited_registry: InheritedRegistry,
    pub(crate) rendered_frame: Frame,
    pub(crate) next_frame: Frame,
    pub(super) next_hitbox_id: HitboxId,
    pub(crate) next_tooltip_id: TooltipId,
    pub(crate) tooltip_bounds: Option<TooltipBounds>,
    /// K04 Task 36: per-window pre-frame callbacks. Migrated from
    /// `Rc<RefCell<Vec<FrameCallback>>>` to `RefCell<SmallVec<[_; 4]>>`
    /// directly on the `Window`. The `Rc` clone was needed before K04
    /// because the platform `on_request_frame` callback held one; with
    /// `App::run_frame` and the platform callback both reaching `Window`
    /// via `handle.update(...)`, the indirection is unnecessary.
    ///
    /// Sized to 4 inline because typical pre-frame queues are short
    /// (animation-frame requests, deferred focus, scroll restore) —
    /// frames that queue more than 4 callbacks already spill to the heap.
    pub(crate) next_frame_callbacks: RefCell<SmallVec<[FrameCallback; 4]>>,
    /// K04 Task 34: post-frame callbacks anchored at `complete_frame`.
    ///
    /// Drained by the [`PostFrame`](crate::frame::FramePhase::PostFrame)
    /// phase of [`App::run_frame`](crate::App::run_frame), AFTER `window.draw()`
    /// has produced the scene. Use for telemetry export, inspector readout,
    /// deferred post-paint settle work.
    ///
    /// Storage is `SmallVec<[_; 4]>` (per Task 36 hot-path rule) wrapped in
    /// `RefCell` for interior mutability — the platform / `run_frame` path
    /// has shared access to `&Window`.
    pub(crate) post_frame_callbacks: RefCell<SmallVec<[FrameCallback; 4]>>,
    /// K04 Task 32: idempotent next-frame request flag.
    ///
    /// `Window::request_animation_frame` sets this to `true`; the platform
    /// `on_request_frame` callback drains the flag at the start of each
    /// frame by setting `invalidator` dirty. Multiple calls in the
    /// same frame coalesce — there is exactly one observable frame request
    /// regardless of how many callers hit the API.
    pub(crate) request_next_frame: Cell<bool>,
    pub(crate) dirty_views: FxHashSet<EntityId>,
    pub(super) focus_listeners: SubscriberSet<(), AnyWindowFocusListener>,
    pub(crate) focus_lost_listeners: SubscriberSet<(), AnyObserver>,
    pub(super) default_prevented: bool,
    pub(super) mouse_position: Point<Pixels>,
    pub(super) mouse_hit_test: HitTest,
    /// Per-frame map populated by `Interactivity::paint` (T14)
    /// recording each hitbox's `HitTestBehavior` (Opaque /
    /// Translucent / DeferToChild). `Window::hit_test` queries this
    /// map; entries default to `HitTestBehavior::Opaque` when the
    /// hitbox is not associated with an `Interactivity` (painted-only
    /// case). Cleared at the start of each frame (alongside
    /// `mouse_hit_test`).
    pub(crate) hit_test_behaviors:
        FxHashMap<HitboxId, crate::gesture::HitTestBehavior>,
    /// Per-frame map of recognizers registered by
    /// `Interactivity::paint`, keyed by `HitboxId`. The dispatcher
    /// drains entries on `PointerPhase::Down`: for each hitbox in the
    /// hit-test result, the registered recognizers are inserted into
    /// the per-pointer arena and their `add_pointer` is called.
    /// Storing as `Rc<RefCell<Box<dyn ...>>>` lets the same instance
    /// live both in this map (during paint→dispatch transit) and in
    /// the arena (during the in-flight gesture). Cleared at the start
    /// of each frame.
    pub(crate) pending_recognizers: FxHashMap<
        HitboxId,
        SmallVec<[Rc<RefCell<Box<dyn crate::gesture::GestureRecognizer>>>; 4]>,
    >,
    /// Per-`Window` gesture arena + settings + sanitizer + pointer-state
    /// cache. Single source of truth for the gesture subsystem (S07.5 T2
    /// consolidated the previously direct `gesture_sanitizer` and
    /// `gesture_pointer_state` fields into here so there is exactly one
    /// owner of gesture state per `Window`).
    pub(crate) gesture_binding: crate::gesture::GestureBinding,
    pub(super) modifiers: Modifiers,
    pub(super) capslock: Capslock,
    pub(super) scale_factor: f32,
    pub(crate) bounds_observers: SubscriberSet<(), AnyObserver>,
    /// ADR-007: observers fired when this window's bound display id changes
    /// (window moves between outputs) or when that display's scale factor
    /// changes. Distinct from `bounds_observers` which fires on size only.
    pub(crate) display_change_observers: SubscriberSet<(), AnyObserver>,
    /// ADR-008: `WindowOptions` invariants kept on the Window so the
    /// programmatic gates on `minimize_window` / future maximize/resize
    /// APIs can reject mutations that violate the requested invariants
    /// even when the platform layer would allow them.
    pub(crate) is_movable: bool,
    pub(crate) is_resizable: bool,
    pub(crate) is_minimizable: bool,
    pub(super) appearance: WindowAppearance,
    pub(crate) appearance_observers: SubscriberSet<(), AnyObserver>,
    pub(super) active: Rc<Cell<bool>>,
    pub(super) hovered: Rc<Cell<bool>>,
    pub(crate) needs_present: Rc<Cell<bool>>,
    /// Tracks recent input event timestamps to determine if input is arriving at a high rate.
    /// Used to selectively enable VRR optimization only when input rate exceeds 60fps.
    pub(crate) input_rate_tracker: Rc<RefCell<InputRateTracker>>,
    pub(super) last_input_modality: InputModality,
    pub(crate) refreshing: bool,
    pub(crate) activation_observers: SubscriberSet<(), AnyObserver>,
    pub(crate) focus: Option<FocusId>,
    pub(super) focus_enabled: bool,
    pub(super) pending_input: Option<PendingInput>,
    pub(super) pending_modifier: ModifierState,
    pub(crate) pending_input_observers: SubscriberSet<(), AnyObserver>,
    pub(super) prompt: Option<RenderablePromptHandle>,
    pub(crate) client_inset: Option<Pixels>,
    /// The hitbox that has captured the pointer, if any.
    /// While captured, mouse events route to this hitbox regardless of hit testing.
    pub(super) captured_hitbox: Option<HitboxId>,
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(super) inspector: Option<Entity<Inspector>>,
}
