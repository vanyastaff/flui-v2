use crate::frame::profile::{FrameProfile, FrameProfileDetailed};
use crate::frame::tick::{TickOutcome, TickTarget, TickTargetId};
use crate::frame::{DeferPlacement, FramePhase};
use crate::scheduler::Instant;
use std::{
    any::{TypeId, type_name},
    cell::{Cell, RefCell, UnsafeCell},
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    sync::{Arc, atomic::Ordering::SeqCst},
    time::Duration,
};

use anyhow::{Context as _, Result, anyhow};
use futures::{
    Future, FutureExt,
    channel::oneshot,
    future::{LocalBoxFuture, Shared},
};
use itertools::Itertools;
use parking_lot::RwLock;
use slotmap::SlotMap;

pub use async_context::*;
use cell::BorrowState;
pub use cell::{AppCell, AppRef, AppRefMut};
use collections::{FxHashMap, FxHashSet, HashMap, VecDeque};
pub use context::*;
pub use entity_map::*;
#[cfg(any(test, feature = "test-support"))]
pub use headless_app_context::*;
use http_client::{HttpClient, Url};
use smallvec::SmallVec;
#[cfg(any(test, feature = "test-support"))]
pub use test_app::*;
#[cfg(any(test, feature = "test-support"))]
pub use test_context::*;
use util::{ResultExt, debug_panic};
#[cfg(all(target_os = "macos", any(test, feature = "test-support")))]
pub use visual_test_context::*;

#[cfg(any(feature = "inspector", debug_assertions))]
use crate::InspectorElementRegistry;
use crate::{
    Action, ActionBuildError, ActionRegistry, Any, AnyView, AnyWindowHandle, AppContext, Arena,
    ArenaBox, Asset, AssetSource, BackgroundExecutor, Bounds, ClipboardItem, CursorStyle,
    DispatchPhase, DisplayId, EventEmitter, FocusHandle, FocusMap, ForegroundExecutor, Global,
    KeyBinding, KeyContext, Keymap, Keystroke, LayoutId, Menu, MenuItem, OwnedMenu,
    PathPromptOptions, Pixels, Platform, PlatformDisplay, PlatformKeyboardLayout,
    PlatformKeyboardMapper, Point, Priority, PromptBuilder, PromptButton, PromptHandle,
    PromptLevel, Render, RenderImage, RenderablePromptHandle, Reservation, ScreenCaptureSource,
    SharedString, SubscriberSet, Subscription, SvgRenderer, Task, TextRenderingMode, TextSystem,
    ThermalState, Window, WindowAppearance, WindowHandle, WindowId, WindowInvalidator,
    colors::{Colors, GlobalColors},
    hash, init_app_menus,
};

mod async_context;
mod cell;
mod context;
mod entity_map;
#[cfg(any(test, feature = "test-support"))]
mod headless_app_context;
#[cfg(any(test, feature = "test-support"))]
mod test_app;
#[cfg(any(test, feature = "test-support"))]
mod test_context;
#[cfg(all(target_os = "macos", any(test, feature = "test-support")))]
mod visual_test_context;

/// The duration for which futures returned from [Context::on_app_quit] can run before the application fully quits.
pub const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(100);

/// A reference to a GPUI application, typically constructed in the `main` function of your app.
/// You won't interact with this type much outside of initial configuration and startup.
pub struct Application(Rc<AppCell>);

/// Represents an application before it is fully launched. Once your app is
/// configured, you'll start the app with `App::run`.
impl Application {
    /// Builds an app with the default platform for the current OS.
    pub fn new() -> Self {
        Self::with_platform(crate::current_platform(false))
    }

    /// Builds an app with a caller-provided platform implementation.
    pub fn with_platform(platform: Rc<dyn Platform>) -> Self {
        Self(App::new_app(
            platform,
            Arc::new(()),
            Arc::new(NullHttpClient),
        ))
    }

    /// Assigns the source of assets for the application.
    pub fn with_assets(self, asset_source: impl AssetSource) -> Self {
        let mut context_lock = self.0.borrow_mut();
        let asset_source = Arc::new(asset_source);
        context_lock.asset_source = asset_source.clone();
        context_lock.svg_renderer = SvgRenderer::new(asset_source);
        drop(context_lock);
        self
    }

    /// Sets the HTTP client for the application.
    pub fn with_http_client(self, http_client: Arc<dyn HttpClient>) -> Self {
        let mut context_lock = self.0.borrow_mut();
        context_lock.http_client = http_client;
        drop(context_lock);
        self
    }

    /// Configures when the application should automatically quit.
    /// By default, [`QuitMode::Default`] is used.
    pub fn with_quit_mode(self, mode: QuitMode) -> Self {
        self.0.borrow_mut().quit_mode = mode;
        self
    }

    /// Start the application. The provided callback will be called once the
    /// app is fully launched.
    pub fn run<F>(self, on_finish_launching: F)
    where
        F: 'static + FnOnce(&mut App),
    {
        let this = self.0.clone();
        let platform = self.0.borrow().platform.clone();
        platform.run(Box::new(move || {
            let cx = &mut *this.borrow_mut();
            on_finish_launching(cx);
        }));
    }

    /// Register a handler to be invoked when the platform instructs the application
    /// to open one or more URLs.
    pub fn on_open_urls<F>(&self, mut callback: F) -> &Self
    where
        F: 'static + FnMut(Vec<String>),
    {
        self.0.borrow().platform.on_open_urls(Box::new(callback));
        self
    }

    /// Invokes a handler when an already-running application is launched.
    /// On macOS, this can occur when the application icon is double-clicked or the app is launched via the dock.
    pub fn on_reopen<F>(&self, mut callback: F) -> &Self
    where
        F: 'static + FnMut(&mut App),
    {
        let this = Rc::downgrade(&self.0);
        self.0.borrow_mut().platform.on_reopen(Box::new(move || {
            if let Some(app) = this.upgrade() {
                callback(&mut app.borrow_mut());
            }
        }));
        self
    }

    /// Returns a handle to the [`BackgroundExecutor`] associated with this app, which can be used to spawn futures in the background.
    pub fn background_executor(&self) -> BackgroundExecutor {
        self.0.borrow().background_executor.clone()
    }

    /// Returns a handle to the [`ForegroundExecutor`] associated with this app, which can be used to spawn futures in the foreground.
    pub fn foreground_executor(&self) -> ForegroundExecutor {
        self.0.borrow().foreground_executor.clone()
    }

    /// Returns a reference to the [`TextSystem`] associated with this app.
    pub fn text_system(&self) -> Arc<TextSystem> {
        self.0.borrow().text_system.clone()
    }

    /// Returns the file URL of the executable with the specified name in the application bundle
    pub fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf> {
        self.0.borrow().path_for_auxiliary_executable(name)
    }
}

type Handler = Box<dyn FnMut(&mut App) -> bool + 'static>;
type Listener = Box<dyn FnMut(&dyn Any, &mut App) -> bool + 'static>;
pub(crate) type KeystrokeObserver =
    Box<dyn FnMut(&KeystrokeEvent, &mut Window, &mut App) -> bool + 'static>;
type QuitHandler = Box<dyn FnOnce(&mut App) -> LocalBoxFuture<'static, ()> + 'static>;
type WindowClosedHandler = Box<dyn FnMut(&mut App)>;
type ReleaseListener = Box<dyn FnOnce(&mut dyn Any, &mut App) + 'static>;
type NewEntityListener = Box<dyn FnMut(AnyEntity, &mut Option<&mut Window>, &mut App) + 'static>;

/// Defines when the application should automatically quit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuitMode {
    /// Use [`QuitMode::Explicit`] on macOS and [`QuitMode::LastWindowClosed`] on other platforms.
    #[default]
    Default,
    /// Quit automatically when the last window is closed.
    LastWindowClosed,
    /// Quit only when requested via [`App::quit`].
    Explicit,
}

#[doc(hidden)]
#[derive(Clone, PartialEq, Eq)]
pub struct SystemWindowTab {
    pub id: WindowId,
    pub title: SharedString,
    pub handle: AnyWindowHandle,
    pub last_active_at: Instant,
}

impl SystemWindowTab {
    /// Create a new instance of the window tab.
    pub fn new(title: SharedString, handle: AnyWindowHandle) -> Self {
        Self {
            id: handle.id,
            title,
            handle,
            last_active_at: Instant::now(),
        }
    }
}

/// A controller for managing window tabs.
#[derive(Default)]
pub struct SystemWindowTabController {
    visible: Option<bool>,
    tab_groups: FxHashMap<usize, Vec<SystemWindowTab>>,
}

impl Global for SystemWindowTabController {}

impl SystemWindowTabController {
    /// Create a new instance of the window tab controller.
    pub fn new() -> Self {
        Self {
            visible: None,
            tab_groups: FxHashMap::default(),
        }
    }

    /// Initialize the global window tab controller.
    pub fn init(cx: &mut App) {
        cx.set_global(SystemWindowTabController::new());
    }

    /// Get all tab groups.
    pub fn tab_groups(&self) -> &FxHashMap<usize, Vec<SystemWindowTab>> {
        &self.tab_groups
    }

    /// Get the next tab group window handle.
    pub fn get_next_tab_group_window(cx: &mut App, id: WindowId) -> Option<&AnyWindowHandle> {
        let controller = cx.global::<SystemWindowTabController>();
        let current_group = controller
            .tab_groups
            .iter()
            .find_map(|(group, tabs)| tabs.iter().find(|tab| tab.id == id).map(|_| group));

        let current_group = current_group?;
        // TODO: `.keys()` returns arbitrary order, what does "next" mean?
        let mut group_ids: Vec<_> = controller.tab_groups.keys().collect();
        let idx = group_ids.iter().position(|g| *g == current_group)?;
        let next_idx = (idx + 1) % group_ids.len();

        controller
            .tab_groups
            .get(group_ids[next_idx])
            .and_then(|tabs| {
                tabs.iter()
                    .max_by_key(|tab| tab.last_active_at)
                    .or_else(|| tabs.first())
                    .map(|tab| &tab.handle)
            })
    }

    /// Get the previous tab group window handle.
    pub fn get_prev_tab_group_window(cx: &mut App, id: WindowId) -> Option<&AnyWindowHandle> {
        let controller = cx.global::<SystemWindowTabController>();
        let current_group = controller
            .tab_groups
            .iter()
            .find_map(|(group, tabs)| tabs.iter().find(|tab| tab.id == id).map(|_| group));

        let current_group = current_group?;
        // TODO: `.keys()` returns arbitrary order, what does "previous" mean?
        let mut group_ids: Vec<_> = controller.tab_groups.keys().collect();
        let idx = group_ids.iter().position(|g| *g == current_group)?;
        let prev_idx = if idx == 0 {
            group_ids.len() - 1
        } else {
            idx - 1
        };

        controller
            .tab_groups
            .get(group_ids[prev_idx])
            .and_then(|tabs| {
                tabs.iter()
                    .max_by_key(|tab| tab.last_active_at)
                    .or_else(|| tabs.first())
                    .map(|tab| &tab.handle)
            })
    }

    /// Get all tabs in the same window.
    pub fn tabs(&self, id: WindowId) -> Option<&Vec<SystemWindowTab>> {
        self.tab_groups
            .values()
            .find(|tabs| tabs.iter().any(|tab| tab.id == id))
    }

    /// Initialize the visibility of the system window tab controller.
    pub fn init_visible(cx: &mut App, visible: bool) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        if controller.visible.is_none() {
            controller.visible = Some(visible);
        }
    }

    /// Get the visibility of the system window tab controller.
    pub fn is_visible(&self) -> bool {
        self.visible.unwrap_or(false)
    }

    /// Set the visibility of the system window tab controller.
    pub fn set_visible(cx: &mut App, visible: bool) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        controller.visible = Some(visible);
    }

    /// Update the last active of a window.
    pub fn update_last_active(cx: &mut App, id: WindowId) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        for windows in controller.tab_groups.values_mut() {
            for tab in windows.iter_mut() {
                if tab.id == id {
                    tab.last_active_at = Instant::now();
                }
            }
        }
    }

    /// Update the position of a tab within its group.
    pub fn update_tab_position(cx: &mut App, id: WindowId, ix: usize) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        for (_, windows) in controller.tab_groups.iter_mut() {
            if let Some(current_pos) = windows.iter().position(|tab| tab.id == id) {
                if ix < windows.len() && current_pos != ix {
                    let window_tab = windows.remove(current_pos);
                    windows.insert(ix, window_tab);
                }
                break;
            }
        }
    }

    /// Update the title of a tab.
    pub fn update_tab_title(cx: &mut App, id: WindowId, title: SharedString) {
        let controller = cx.global::<SystemWindowTabController>();
        let tab = controller
            .tab_groups
            .values()
            .flat_map(|windows| windows.iter())
            .find(|tab| tab.id == id);

        if tab.map_or(true, |t| t.title == title) {
            return;
        }

        let mut controller = cx.global_mut::<SystemWindowTabController>();
        for windows in controller.tab_groups.values_mut() {
            for tab in windows.iter_mut() {
                if tab.id == id {
                    tab.title = title;
                    return;
                }
            }
        }
    }

    /// Insert a tab into a tab group.
    pub fn add_tab(cx: &mut App, id: WindowId, tabs: Vec<SystemWindowTab>) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        let Some(tab) = tabs.iter().find(|tab| tab.id == id).cloned() else {
            return;
        };

        let mut expected_tab_ids: Vec<_> = tabs
            .iter()
            .filter(|tab| tab.id != id)
            .map(|tab| tab.id)
            .sorted()
            .collect();

        let mut tab_group_id = None;
        for (group_id, group_tabs) in &controller.tab_groups {
            let tab_ids: Vec<_> = group_tabs.iter().map(|tab| tab.id).sorted().collect();
            if tab_ids == expected_tab_ids {
                tab_group_id = Some(*group_id);
                break;
            }
        }

        if let Some(tab_group_id) = tab_group_id {
            if let Some(tabs) = controller.tab_groups.get_mut(&tab_group_id) {
                tabs.push(tab);
            }
        } else {
            let new_group_id = controller.tab_groups.len();
            controller.tab_groups.insert(new_group_id, tabs);
        }
    }

    /// Remove a tab from a tab group.
    pub fn remove_tab(cx: &mut App, id: WindowId) -> Option<SystemWindowTab> {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        let mut removed_tab = None;

        controller.tab_groups.retain(|_, tabs| {
            if let Some(pos) = tabs.iter().position(|tab| tab.id == id) {
                removed_tab = Some(tabs.remove(pos));
            }
            !tabs.is_empty()
        });

        removed_tab
    }

    /// Move a tab to a new tab group.
    pub fn move_tab_to_new_window(cx: &mut App, id: WindowId) {
        let mut removed_tab = Self::remove_tab(cx, id);
        let mut controller = cx.global_mut::<SystemWindowTabController>();

        if let Some(tab) = removed_tab {
            let new_group_id = controller.tab_groups.keys().max().map_or(0, |k| k + 1);
            controller.tab_groups.insert(new_group_id, vec![tab]);
        }
    }

    /// Merge all tab groups into a single group.
    pub fn merge_all_windows(cx: &mut App, id: WindowId) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        let Some(initial_tabs) = controller.tabs(id) else {
            return;
        };

        let initial_tabs_len = initial_tabs.len();
        let mut all_tabs = initial_tabs.clone();

        for (_, mut tabs) in controller.tab_groups.drain() {
            tabs.retain(|tab| !all_tabs[..initial_tabs_len].contains(tab));
            all_tabs.extend(tabs);
        }

        controller.tab_groups.insert(0, all_tabs);
    }

    /// Selects the next tab in the tab group in the trailing direction.
    pub fn select_next_tab(cx: &mut App, id: WindowId) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        let Some(tabs) = controller.tabs(id) else {
            return;
        };

        let current_index = tabs.iter().position(|tab| tab.id == id).unwrap();
        let next_index = (current_index + 1) % tabs.len();

        let _ = &tabs[next_index].handle.update(cx, |_, window, _| {
            window.activate_window();
        });
    }

    /// Selects the previous tab in the tab group in the leading direction.
    pub fn select_previous_tab(cx: &mut App, id: WindowId) {
        let mut controller = cx.global_mut::<SystemWindowTabController>();
        let Some(tabs) = controller.tabs(id) else {
            return;
        };

        let current_index = tabs.iter().position(|tab| tab.id == id).unwrap();
        let previous_index = if current_index == 0 {
            tabs.len() - 1
        } else {
            current_index - 1
        };

        let _ = &tabs[previous_index].handle.update(cx, |_, window, _| {
            window.activate_window();
        });
    }
}

pub(crate) enum GpuiMode {
    #[cfg(any(test, feature = "test-support"))]
    Test {
        skip_drawing: bool,
    },
    Production,
}

impl GpuiMode {
    #[cfg(any(test, feature = "test-support"))]
    pub fn test() -> Self {
        GpuiMode::Test {
            skip_drawing: false,
        }
    }

    #[inline]
    pub(crate) fn skip_drawing(&self) -> bool {
        match self {
            #[cfg(any(test, feature = "test-support"))]
            GpuiMode::Test { skip_drawing } => *skip_drawing,
            GpuiMode::Production => false,
        }
    }
}

/// Contains the state of the full application, and passed as a reference to a variety of callbacks.
/// Other [Context] derefs to this type.
/// You need a reference to an `App` to access the state of a [Entity].
pub struct App {
    pub(crate) this: Weak<AppCell>,
    pub(crate) platform: Rc<dyn Platform>,
    text_system: Arc<TextSystem>,

    pub(crate) actions: Rc<ActionRegistry>,
    pub(crate) active_drag: Option<AnyDrag>,
    pub(crate) background_executor: BackgroundExecutor,
    pub(crate) foreground_executor: ForegroundExecutor,
    pub(crate) entities: EntityMap,
    pub(crate) new_entity_observers: SubscriberSet<TypeId, NewEntityListener>,
    pub(crate) windows: SlotMap<WindowId, Option<Box<Window>>>,
    pub(crate) window_handles: FxHashMap<WindowId, AnyWindowHandle>,
    pub(crate) focus_handles: Arc<FocusMap>,
    pub(crate) keymap: Rc<RefCell<Keymap>>,
    pub(crate) keyboard_layout: Box<dyn PlatformKeyboardLayout>,
    pub(crate) keyboard_mapper: Rc<dyn PlatformKeyboardMapper>,
    pub(crate) global_action_listeners:
        FxHashMap<TypeId, Vec<Rc<dyn Fn(&dyn Any, DispatchPhase, &mut Self)>>>,
    pending_effects: VecDeque<Effect>,

    pub(crate) observers: SubscriberSet<EntityId, Handler>,
    pub(crate) event_listeners: SubscriberSet<EntityId, (TypeId, Listener)>,
    pub(crate) keystroke_observers: SubscriberSet<(), KeystrokeObserver>,
    pub(crate) keystroke_interceptors: SubscriberSet<(), KeystrokeObserver>,
    pub(crate) keyboard_layout_observers: SubscriberSet<(), Handler>,
    pub(crate) thermal_state_observers: SubscriberSet<(), Handler>,
    pub(crate) release_listeners: SubscriberSet<EntityId, ReleaseListener>,
    pub(crate) global_observers: SubscriberSet<TypeId, Handler>,
    pub(crate) quit_observers: SubscriberSet<(), QuitHandler>,
    pub(crate) restart_observers: SubscriberSet<(), Handler>,
    pub(crate) window_closed_observers: SubscriberSet<(), WindowClosedHandler>,

    /// Per-App element arena. This isolates element allocations between different
    /// App instances (important for tests where multiple Apps run concurrently).
    pub(crate) element_arena: RefCell<Arena>,
    /// Per-App event arena.
    pub(crate) event_arena: Arena,

    // Drop globals last. We need to ensure all tasks owned by entities and
    // callbacks are marked cancelled at this point as this will also shutdown
    // the tokio runtime. As any task attempting to spawn a blocking tokio task,
    // might panic.
    pub(crate) globals_by_type: FxHashMap<TypeId, Box<dyn Any>>,

    // assets
    pub(crate) loading_assets: FxHashMap<(TypeId, u64), Box<dyn Any>>,
    asset_source: Arc<dyn AssetSource>,
    pub(crate) svg_renderer: SvgRenderer,
    http_client: Arc<dyn HttpClient>,

    // below is plain data, the drop order is insignificant here
    pub(crate) pending_notifications: FxHashSet<EntityId>,
    pub(crate) pending_global_notifications: FxHashSet<TypeId>,
    pub(crate) restart_path: Option<PathBuf>,
    pub(crate) layout_id_buffer: Vec<LayoutId>, // We recycle this memory across layout requests.
    pub(crate) propagate_event: bool,
    pub(crate) prompt_builder: Option<PromptBuilder>,
    pub(crate) window_invalidators_by_entity:
        FxHashMap<EntityId, FxHashMap<WindowId, WindowInvalidator>>,
    pub(crate) tracked_entities: FxHashMap<WindowId, FxHashSet<EntityId>>,
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(crate) inspector_renderer: Option<crate::InspectorRenderer>,
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub(crate) inspector_element_registry: InspectorElementRegistry,
    #[cfg(any(test, feature = "test-support", debug_assertions))]
    pub(crate) name: Option<&'static str>,
    pub(crate) text_rendering_mode: Rc<Cell<TextRenderingMode>>,

    pub(crate) window_update_stack: Vec<WindowId>,
    /// K15 (Phase 0-K): currently-leased entity for re-entrancy detection.
    /// Set by `App::update_entity` before calling `EntityMap::lease`; cleared on
    /// return. Same-entity nested `update_entity` is detected here BEFORE the
    /// lease; multi-entity cycles (`A → B → A`) are caught by
    /// `EntityMap::double_lease_panic` after this field has been replaced with
    /// the inner entity's id. Both paths now produce the same
    /// `ReentryError::NestedEntityUpdate(_)` Display.
    pub(crate) currently_updating_entity: Option<EntityId>,
    /// K15 (Phase 0-K): selects how re-entry contract violations are reported.
    /// See `flui_core::reentrancy::ReentryMode`. Default is `Strict` in
    /// `cfg(test)` and `Loose` in release.
    pub(crate) reentry_mode: crate::reentrancy::ReentryMode,
    pub(crate) mode: GpuiMode,
    flushing_effects: bool,
    pending_updates: usize,
    /// K04 Task 26: the current frame phase. `FramePhase::Idle` outside
    /// `App::run_frame`; advanced by `App::run_frame` (Task 23) at every phase
    /// transition. Cheap (one byte) — inspector and Framework-tier tests read
    /// it via [`App::current_phase()`].
    pub(crate) current_phase: FramePhase,
    /// K04 Task 17 + Task 25: per-App frame clock. Samples the underlying
    /// scheduler [`Clock`](crate::scheduler::Clock) exactly once at the start
    /// of each `App::run_frame` call (axiom P3). All consumers in that frame —
    /// animation tick (Task 30), `AnimationController::value()` (Task 31),
    /// post-frame callbacks, layout cache hash keys — read the same `Instant`
    /// for the duration of the frame.
    ///
    /// `!Send` (lives on `App`); layered on top of `Arc<dyn Clock>`. Wired by
    /// `App::run_frame` (Task 23) for `begin_frame`/`end_frame`; consumed via
    /// the [`App::frame_clock()`] accessor.
    pub(crate) frame_clock: crate::frame::clock::FrameClock,
    /// K04 Task 27: always-on per-frame telemetry. Populated at the end of
    /// every `App::run_frame` call (or, during the K04 staged rollout, every
    /// `TestApp::advance_frame` call). Read via [`App::frame_profile()`].
    ///
    /// Sized for cheapness (~32 bytes); never allocates. Always available.
    pub(crate) frame_profile: FrameProfile,
    /// K04 Task 27: flag-gated detailed per-frame telemetry. Populated only
    /// when [`App::set_profiling_enabled(true)`](App::set_profiling_enabled).
    /// Default `cfg!(debug_assertions)`.
    ///
    /// Read via [`App::frame_profile_detailed()`] — returns `None` when
    /// profiling is disabled to make the disabled path observably cold.
    pub(crate) frame_profile_detailed: FrameProfileDetailed,
    /// K04 Task 27: gate for [`Self::frame_profile_detailed`]. When `false`,
    /// `App::run_frame` skips per-phase `Duration` measurements — release
    /// builds default to `false` per `docs/promt.md` §3.1 hot-path discipline.
    pub(crate) profiling_enabled: bool,
    /// K04 Task 30: active animation-tick targets.
    ///
    /// The [`FramePhase::AnimationTick`] phase walks this map once per
    /// `run_frame`, leases each target, calls
    /// [`TickTarget::tick`](crate::frame::tick::TickTarget::tick), emits an
    /// `Effect::Notify` for [`TickOutcome::Continue`] entries, and removes
    /// [`TickOutcome::Done`] entries from the set.
    ///
    /// Today (K04) only [`AnimationController`](crate::AnimationController)
    /// implements the trait, so the value type is a typed
    /// `WeakEntity<AnimationController>`. SF08 (async widgets) and follow-up
    /// audio / spring / particle controllers will widen the value type
    /// additively when the trait is unsealed.
    pub(crate) active_animations: FxHashMap<TickTargetId, WeakEntity<crate::AnimationController>>,
    /// K04 Task 35: App-level pre-frame callbacks. Fire at the top of
    /// every `App::run_frame` (across all windows) AFTER per-window
    /// `Window::on_pre_frame` callbacks. Use for cross-window pre-paint
    /// work (input replay, telemetry seed, deferred focus changes that
    /// span multiple windows).
    pub(crate) app_pre_frame_callbacks: SmallVec<[Box<dyn FnOnce(&mut App)>; 4]>,
    /// K04 Task 35: App-level post-frame callbacks. Fire at the bottom of
    /// every `App::run_frame` (across all windows) AFTER per-window
    /// `Window::on_post_frame` callbacks. Use for cross-window
    /// post-paint work (input replay, telemetry export, profiler tick).
    pub(crate) app_post_frame_callbacks: SmallVec<[Box<dyn FnOnce(&mut App)>; 4]>,
    /// K04 Task 21: test-mode auto-redraw flag.
    ///
    /// When `true` (the default under `cfg(test, feature = "test-support")`),
    /// `App::flush_effects` (the legacy Pre-K04 entry) redraws every dirty
    /// window inline once the effect queue empties. This is the behavior the
    /// pre-K04 test suite relies on — most existing tests trigger UI work via
    /// `cx.update(...)` and expect the resulting `cx.notify(...)` to materialize
    /// as a draw before the call returns.
    ///
    /// Tests that need observable phase boundaries (Task 38 phase-order tests
    /// and downstream Framework-tier tests) flip this to `false` and drive
    /// frames explicitly through `TestApp::advance_frame()` (Task 22). K04+1
    /// will flip the default to `false` once Tier-C tests migrate.
    ///
    /// The field is cfg-gated because the pre-K04 auto-redraw block in
    /// `flush_effects_at` is itself cfg-gated to `cfg(any(test, feature = "test-support"))`.
    #[cfg(any(test, feature = "test-support"))]
    pub auto_advance_frames_on_flush: bool,
    quit_mode: QuitMode,
    quitting: bool,

    // We need to ensure the leak detector drops last, after all tasks, callbacks and things have been dropped.
    // Otherwise it may report false positives.
    #[cfg(any(test, feature = "leak-detection"))]
    _ref_counts: Arc<RwLock<EntityRefCounts>>,
}

impl App {
    #[allow(clippy::new_ret_no_self)]
    pub(crate) fn new_app(
        platform: Rc<dyn Platform>,
        asset_source: Arc<dyn AssetSource>,
        http_client: Arc<dyn HttpClient>,
    ) -> Rc<AppCell> {
        let background_executor = platform.background_executor();
        let foreground_executor = platform.foreground_executor();
        assert!(
            background_executor.is_main_thread(),
            "must construct App on main thread"
        );

        let text_system = Arc::new(TextSystem::new(platform.text_system()));
        let entities = EntityMap::new();
        let keyboard_layout = platform.keyboard_layout();
        let keyboard_mapper = platform.keyboard_mapper();

        #[cfg(any(test, feature = "leak-detection"))]
        let _ref_counts = entities.ref_counts_drop_handle();

        let app = Rc::new_cyclic(|this| AppCell {
            app: UnsafeCell::new(App {
                this: this.clone(),
                platform: platform.clone(),
                text_system,
                text_rendering_mode: Rc::new(Cell::new(TextRenderingMode::default())),
                mode: GpuiMode::Production,
                actions: Rc::new(ActionRegistry::default()),
                flushing_effects: false,
                pending_updates: 0,
                current_phase: FramePhase::Idle,
                // K04 Task 17 + Task 25: pull the scheduler's `Arc<dyn Clock>`
                // (the same handle every other component reaches via
                // `cx.background_executor().scheduler().clock()`) and layer
                // `FrameClock` on top.
                frame_clock: crate::frame::clock::FrameClock::new(
                    background_executor.scheduler_executor().scheduler().clock(),
                ),
                // K04 Task 27: always-on profile starts zeroed; populated on
                // every `App::run_frame`.
                frame_profile: FrameProfile::default(),
                frame_profile_detailed: FrameProfileDetailed::default(),
                // K04 Task 27: detailed profiling defaults to debug-only.
                // Production builds stay cold unless `set_profiling_enabled`
                // is explicitly flipped on by the host.
                profiling_enabled: cfg!(debug_assertions),
                // K04 Task 30: active-animation set; populated by
                // `AnimationController::forward` / `reverse` / `animate_*`
                // and drained by [`Self::run_frame`]'s AnimationTick phase.
                active_animations: FxHashMap::default(),
                // K04 Task 35: App-level pre/post-frame callback queues;
                // populated by `App::on_pre_frame` / `App::on_post_frame`,
                // drained by `App::run_frame`.
                app_pre_frame_callbacks: SmallVec::new(),
                app_post_frame_callbacks: SmallVec::new(),
                // K04 Task 21: default `true` under test-support to preserve
                // pre-K04 test-suite behavior. Phase-order tests (Task 38) and
                // future Framework-tier tests flip this to `false`.
                #[cfg(any(test, feature = "test-support"))]
                auto_advance_frames_on_flush: true,
                active_drag: None,
                background_executor,
                foreground_executor,
                svg_renderer: SvgRenderer::new(asset_source.clone()),
                loading_assets: Default::default(),
                asset_source,
                http_client,
                globals_by_type: FxHashMap::default(),
                entities,
                new_entity_observers: SubscriberSet::new(),
                windows: SlotMap::with_key(),
                window_update_stack: Vec::new(),
                currently_updating_entity: None,
                reentry_mode: if cfg!(test) {
                    crate::reentrancy::ReentryMode::Strict
                } else {
                    crate::reentrancy::ReentryMode::Loose
                },
                window_handles: FxHashMap::default(),
                focus_handles: Arc::new(RwLock::new(SlotMap::with_key())),
                keymap: Rc::new(RefCell::new(Keymap::default())),
                keyboard_layout,
                keyboard_mapper,
                global_action_listeners: FxHashMap::default(),
                pending_effects: VecDeque::new(),
                pending_notifications: FxHashSet::default(),
                pending_global_notifications: FxHashSet::default(),
                observers: SubscriberSet::new(),
                tracked_entities: FxHashMap::default(),
                window_invalidators_by_entity: FxHashMap::default(),
                event_listeners: SubscriberSet::new(),
                release_listeners: SubscriberSet::new(),
                keystroke_observers: SubscriberSet::new(),
                keystroke_interceptors: SubscriberSet::new(),
                keyboard_layout_observers: SubscriberSet::new(),
                thermal_state_observers: SubscriberSet::new(),
                global_observers: SubscriberSet::new(),
                quit_observers: SubscriberSet::new(),
                restart_observers: SubscriberSet::new(),
                restart_path: None,
                window_closed_observers: SubscriberSet::new(),
                layout_id_buffer: Default::default(),
                propagate_event: true,
                prompt_builder: Some(PromptBuilder::Default),
                #[cfg(any(feature = "inspector", debug_assertions))]
                inspector_renderer: None,
                #[cfg(any(feature = "inspector", debug_assertions))]
                inspector_element_registry: InspectorElementRegistry::default(),
                quit_mode: QuitMode::default(),
                quitting: false,

                #[cfg(any(test, feature = "test-support", debug_assertions))]
                name: None,
                element_arena: RefCell::new(Arena::new(1024 * 1024)),
                event_arena: Arena::new(1024 * 1024),

                #[cfg(any(test, feature = "leak-detection"))]
                _ref_counts,
            }),
            borrowed: Cell::new(BorrowState::Free),
            _not_send: PhantomData,
        });

        init_app_menus(platform.as_ref(), &app.borrow());
        SystemWindowTabController::init(&mut app.borrow_mut());

        platform.on_keyboard_layout_change(Box::new({
            let app = Rc::downgrade(&app);
            move || {
                if let Some(app) = app.upgrade() {
                    let cx = &mut app.borrow_mut();
                    cx.keyboard_layout = cx.platform.keyboard_layout();
                    cx.keyboard_mapper = cx.platform.keyboard_mapper();
                    cx.keyboard_layout_observers
                        .clone()
                        .retain(&(), move |callback| (callback)(cx));
                }
            }
        }));

        platform.on_thermal_state_change(Box::new({
            let app = Rc::downgrade(&app);
            move || {
                if let Some(app) = app.upgrade() {
                    let cx = &mut app.borrow_mut();
                    cx.thermal_state_observers
                        .clone()
                        .retain(&(), move |callback| (callback)(cx));
                }
            }
        }));

        platform.on_quit(Box::new({
            let cx = Rc::downgrade(&app);
            move || {
                if let Some(cx) = cx.upgrade() {
                    cx.borrow_mut().shutdown();
                }
            }
        }));

        app
    }

    #[doc(hidden)]
    pub fn ref_counts_drop_handle(&self) -> impl Sized + use<> {
        self.entities.ref_counts_drop_handle()
    }

    /// Captures a snapshot of all entities that currently have alive handles.
    ///
    /// The returned [`LeakDetectorSnapshot`] can later be passed to
    /// [`assert_no_new_leaks`](Self::assert_no_new_leaks) to verify that no
    /// entities created after the snapshot are still alive.
    #[cfg(any(test, feature = "leak-detection"))]
    pub fn leak_detector_snapshot(&self) -> LeakDetectorSnapshot {
        self.entities.leak_detector_snapshot()
    }

    /// Asserts that no entities created after `snapshot` still have alive handles.
    ///
    /// Entities that were already tracked at the time of the snapshot are ignored,
    /// even if they still have handles. Only *new* entities (those whose
    /// `EntityId` was not present in the snapshot) are considered leaks.
    ///
    /// # Panics
    ///
    /// Panics if any new entity handles exist. The panic message lists every
    /// leaked entity with its type name, and includes allocation-site backtraces
    /// when `LEAK_BACKTRACE` is set.
    #[cfg(any(test, feature = "leak-detection"))]
    pub fn assert_no_new_leaks(&self, snapshot: &LeakDetectorSnapshot) {
        self.entities.assert_no_new_leaks(snapshot)
    }

    /// Quit the application gracefully. Handlers registered with [`Context::on_app_quit`]
    /// will be given 100ms to complete before exiting.
    pub fn shutdown(&mut self) {
        let mut futures = Vec::new();

        for observer in self.quit_observers.remove(&()) {
            futures.push(observer(self));
        }

        self.windows.clear();
        self.window_handles.clear();
        self.flush_effects();
        self.quitting = true;

        let futures = futures::future::join_all(futures);
        if self
            .foreground_executor
            .block_with_timeout(SHUTDOWN_TIMEOUT, futures)
            .is_err()
        {
            log::error!("timed out waiting on app_will_quit");
        }

        self.quitting = false;
    }

    /// Get the id of the current keyboard layout
    pub fn keyboard_layout(&self) -> &dyn PlatformKeyboardLayout {
        self.keyboard_layout.as_ref()
    }

    /// Get the current keyboard mapper.
    pub fn keyboard_mapper(&self) -> &Rc<dyn PlatformKeyboardMapper> {
        &self.keyboard_mapper
    }

    /// Invokes a handler when the current keyboard layout changes
    pub fn on_keyboard_layout_change<F>(&self, mut callback: F) -> Subscription
    where
        F: 'static + FnMut(&mut App),
    {
        let (subscription, activate) = self.keyboard_layout_observers.insert(
            (),
            Box::new(move |cx| {
                callback(cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// Gracefully quit the application via the platform's standard routine.
    pub fn quit(&self) {
        self.platform.quit();
    }

    /// Schedules all windows in the application to be redrawn. This can be called
    /// multiple times in an update cycle and still result in a single redraw.
    pub fn refresh_windows(&mut self) {
        self.pending_effects.push_back(Effect::RefreshWindows);
    }

    pub(crate) fn update<R>(&mut self, update: impl FnOnce(&mut Self) -> R) -> R {
        self.start_update();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| update(self)));
        match result {
            Ok(result) => {
                let finish = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.finish_update();
                }));
                if let Err(payload) = finish {
                    self.abort_update_after_panic();
                    std::panic::resume_unwind(payload);
                }
                result
            }
            Err(payload) => {
                self.abort_update_after_panic();
                std::panic::resume_unwind(payload);
            }
        }
    }

    pub(crate) fn start_update(&mut self) {
        self.pending_updates += 1;
    }

    fn abort_update_after_panic(&mut self) {
        if self.pending_updates > 0 {
            self.pending_updates -= 1;
        }
        self.flushing_effects = false;
    }

    pub(crate) fn finish_update(&mut self) {
        if !self.flushing_effects && self.pending_updates == 1 {
            self.flushing_effects = true;
            self.flush_effects();
            self.flushing_effects = false;
        }
        self.pending_updates -= 1;
    }

    #[cfg(test)]
    pub(crate) fn pending_updates_for_test(&self) -> usize {
        self.pending_updates
    }

    #[cfg(test)]
    pub(crate) fn flushing_effects_for_test(&self) -> bool {
        self.flushing_effects
    }

    /// Arrange a callback to be invoked when the given entity calls `notify` on its respective context.
    pub fn observe<W>(
        &mut self,
        entity: &Entity<W>,
        mut on_notify: impl FnMut(Entity<W>, &mut App) + 'static,
    ) -> Subscription
    where
        W: 'static,
    {
        self.observe_internal(entity, move |e, cx| {
            on_notify(e, cx);
            true
        })
    }

    pub(crate) fn detect_accessed_entities<R>(
        &mut self,
        callback: impl FnOnce(&mut App) -> R,
    ) -> (R, FxHashSet<EntityId>) {
        let accessed_entities_start = self.entities.accessed_entities.get_mut().clone();
        let result = callback(self);
        let entities_accessed_in_callback = self
            .entities
            .accessed_entities
            .get_mut()
            .difference(&accessed_entities_start)
            .copied()
            .collect::<FxHashSet<EntityId>>();
        (result, entities_accessed_in_callback)
    }

    pub(crate) fn record_entities_accessed(
        &mut self,
        window_handle: AnyWindowHandle,
        invalidator: WindowInvalidator,
        entities: &FxHashSet<EntityId>,
    ) {
        let mut tracked_entities =
            std::mem::take(self.tracked_entities.entry(window_handle.id).or_default());
        for entity in tracked_entities.iter() {
            self.window_invalidators_by_entity
                .entry(*entity)
                .and_modify(|windows| {
                    windows.remove(&window_handle.id);
                });
        }
        for entity in entities.iter() {
            self.window_invalidators_by_entity
                .entry(*entity)
                .or_default()
                .insert(window_handle.id, invalidator.clone());
        }
        tracked_entities.clear();
        tracked_entities.extend(entities.iter().copied());
        self.tracked_entities
            .insert(window_handle.id, tracked_entities);
    }

    pub(crate) fn new_observer(&mut self, key: EntityId, value: Handler) -> Subscription {
        let (subscription, activate) = self.observers.insert(key, value);
        self.defer(move |_| activate());
        subscription
    }

    pub(crate) fn observe_internal<W>(
        &mut self,
        entity: &Entity<W>,
        mut on_notify: impl FnMut(Entity<W>, &mut App) -> bool + 'static,
    ) -> Subscription
    where
        W: 'static,
    {
        let entity_id = entity.entity_id();
        let handle = entity.downgrade();
        self.new_observer(
            entity_id,
            Box::new(move |cx| {
                if let Some(entity) = handle.upgrade() {
                    on_notify(entity, cx)
                } else {
                    false
                }
            }),
        )
    }

    /// Arrange for the given callback to be invoked whenever the given entity emits an event of a given type.
    /// The callback is provided a handle to the emitting entity and a reference to the emitted event.
    pub fn subscribe<T, Event>(
        &mut self,
        entity: &Entity<T>,
        mut on_event: impl FnMut(Entity<T>, &Event, &mut App) + 'static,
    ) -> Subscription
    where
        T: 'static + EventEmitter<Event>,
        Event: 'static,
    {
        self.subscribe_internal(entity, move |entity, event, cx| {
            on_event(entity, event, cx);
            true
        })
    }

    pub(crate) fn new_subscription(
        &mut self,
        key: EntityId,
        value: (TypeId, Listener),
    ) -> Subscription {
        let (subscription, activate) = self.event_listeners.insert(key, value);
        self.defer(move |_| activate());
        subscription
    }
    pub(crate) fn subscribe_internal<T, Evt>(
        &mut self,
        entity: &Entity<T>,
        mut on_event: impl FnMut(Entity<T>, &Evt, &mut App) -> bool + 'static,
    ) -> Subscription
    where
        T: 'static + EventEmitter<Evt>,
        Evt: 'static,
    {
        let entity_id = entity.entity_id();
        let handle = entity.downgrade();
        self.new_subscription(
            entity_id,
            (
                TypeId::of::<Evt>(),
                Box::new(move |event, cx| {
                    let event: &Evt = event.downcast_ref().expect("invalid event type");
                    if let Some(entity) = handle.upgrade() {
                        on_event(entity, event, cx)
                    } else {
                        false
                    }
                }),
            ),
        )
    }

    /// Returns handles to all open windows in the application.
    /// Each handle could be downcast to a handle typed for the root view of that window.
    /// To find all windows of a given type, you could filter on
    pub fn windows(&self) -> Vec<AnyWindowHandle> {
        self.windows
            .keys()
            .flat_map(|window_id| self.window_handles.get(&window_id).copied())
            .collect()
    }

    /// Returns the window handles ordered by their appearance on screen, front to back.
    ///
    /// The first window in the returned list is the active/topmost window of the application.
    ///
    /// This method returns None if the platform doesn't implement the method yet.
    pub fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        self.platform.window_stack()
    }

    /// Returns a handle to the window that is currently focused at the platform level, if one exists.
    pub fn active_window(&self) -> Option<AnyWindowHandle> {
        self.platform.active_window()
    }

    /// Opens a new window with the given option and the root view returned by the given function.
    /// The function is invoked with a `Window`, which can be used to interact with window-specific
    /// functionality.
    pub fn open_window<V: 'static + Render>(
        &mut self,
        options: crate::WindowOptions,
        build_root_view: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
    ) -> anyhow::Result<WindowHandle<V>> {
        self.update(|cx| {
            let id = cx.windows.insert(None);
            let handle = WindowHandle::new(id);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                match Window::new(handle.into(), options, cx) {
                    Ok(mut window) => {
                        // SAFETY-CONTRACT(K15): no re-entry check needed here —
                        // `id` was just allocated by `cx.windows.insert(None)` a
                        // few lines above, so no other code path holds it on
                        // `window_update_stack`. The push/pop pair is purely for
                        // bookkeeping (e.g., `Effect::EntityCreated` consults the
                        // stack to associate new entities with the current
                        // window).
                        let stack_len = cx.window_update_stack.len();
                        cx.window_update_stack.push(id);
                        let root_view =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                build_root_view(&mut window, cx)
                            }));
                        cx.window_update_stack.truncate(stack_len);
                        let root_view = match root_view {
                            Ok(root_view) => root_view,
                            Err(payload) => std::panic::resume_unwind(payload),
                        };
                        window.root.replace(root_view.into());
                        window.defer(cx, |window: &mut Window, cx| window.appearance_changed(cx));

                        // allow a window to draw at least once before returning
                        // this didn't cause any issues on non windows platforms as it seems we always won the race to on_request_frame
                        // on windows we quite frequently lose the race and return a window that has never rendered, which leads to a crash
                        // where DispatchTree::root_node_id asserts on empty nodes
                        let clear = window.draw(cx);
                        clear.clear();

                        let window_handle = window.handle;
                        cx.windows.get_mut(id).unwrap().replace(Box::new(window));
                        cx.window_handles.insert(id, window_handle);
                        Ok(handle)
                    }
                    Err(e) => {
                        cx.windows.remove(id);
                        Err(e)
                    }
                }
            }));
            match result {
                Ok(result) => result,
                Err(payload) => {
                    cx.window_handles.remove(&id);
                    cx.windows.remove(id);
                    std::panic::resume_unwind(payload);
                }
            }
        })
    }

    /// Instructs the platform to activate the application by bringing it to the foreground.
    pub fn activate(&self, ignoring_other_apps: bool) {
        self.platform.activate(ignoring_other_apps);
    }

    /// Hide the application at the platform level.
    pub fn hide(&self) {
        self.platform.hide();
    }

    /// Hide other applications at the platform level.
    pub fn hide_other_apps(&self) {
        self.platform.hide_other_apps();
    }

    /// Unhide other applications at the platform level.
    pub fn unhide_other_apps(&self) {
        self.platform.unhide_other_apps();
    }

    /// Returns the list of currently active displays.
    pub fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        self.platform.displays()
    }

    /// Returns the primary display that will be used for new windows.
    pub fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.platform.primary_display()
    }

    /// Returns whether `screen_capture_sources` may work.
    pub fn is_screen_capture_supported(&self) -> bool {
        self.platform.is_screen_capture_supported()
    }

    /// Returns a list of available screen capture sources.
    pub fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
        self.platform.screen_capture_sources()
    }

    /// Returns the display with the given ID, if one exists.
    pub fn find_display(&self, id: DisplayId) -> Option<Rc<dyn PlatformDisplay>> {
        self.displays()
            .iter()
            .find(|display| display.id() == id)
            .cloned()
    }

    /// Returns the current thermal state of the system.
    pub fn thermal_state(&self) -> ThermalState {
        self.platform.thermal_state()
    }

    /// Invokes a handler when the thermal state changes
    pub fn on_thermal_state_change<F>(&self, mut callback: F) -> Subscription
    where
        F: 'static + FnMut(&mut App),
    {
        let (subscription, activate) = self.thermal_state_observers.insert(
            (),
            Box::new(move |cx| {
                callback(cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// Returns the appearance of the application's windows.
    pub fn window_appearance(&self) -> WindowAppearance {
        self.platform.window_appearance()
    }

    /// Reads data from the platform clipboard.
    pub fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.platform.read_from_clipboard()
    }

    /// Sets the text rendering mode for the application.
    pub fn set_text_rendering_mode(&mut self, mode: TextRenderingMode) {
        self.text_rendering_mode.set(mode);
    }

    /// Returns the current text rendering mode for the application.
    pub fn text_rendering_mode(&self) -> TextRenderingMode {
        self.text_rendering_mode.get()
    }

    /// Writes data to the platform clipboard.
    pub fn write_to_clipboard(&self, item: ClipboardItem) {
        self.platform.write_to_clipboard(item)
    }

    /// Reads data from the primary selection buffer.
    /// Only available on Linux.
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    pub fn read_from_primary(&self) -> Option<ClipboardItem> {
        self.platform.read_from_primary()
    }

    /// Writes data to the primary selection buffer.
    /// Only available on Linux.
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    pub fn write_to_primary(&self, item: ClipboardItem) {
        self.platform.write_to_primary(item)
    }

    /// Reads data from macOS's "Find" pasteboard.
    ///
    /// Used to share the current search string between apps.
    ///
    /// https://developer.apple.com/documentation/appkit/nspasteboard/name-swift.struct/find
    #[cfg(target_os = "macos")]
    pub fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        self.platform.read_from_find_pasteboard()
    }

    /// Writes data to macOS's "Find" pasteboard.
    ///
    /// Used to share the current search string between apps.
    ///
    /// https://developer.apple.com/documentation/appkit/nspasteboard/name-swift.struct/find
    #[cfg(target_os = "macos")]
    pub fn write_to_find_pasteboard(&self, item: ClipboardItem) {
        self.platform.write_to_find_pasteboard(item)
    }

    /// Writes credentials to the platform keychain.
    pub fn write_credentials(
        &self,
        url: &str,
        username: &str,
        password: &[u8],
    ) -> Task<Result<()>> {
        self.platform.write_credentials(url, username, password)
    }

    /// Reads credentials from the platform keychain.
    pub fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        self.platform.read_credentials(url)
    }

    /// Deletes credentials from the platform keychain.
    pub fn delete_credentials(&self, url: &str) -> Task<Result<()>> {
        self.platform.delete_credentials(url)
    }

    /// Directs the platform's default browser to open the given URL.
    pub fn open_url(&self, url: &str) {
        self.platform.open_url(url);
    }

    /// Registers the given URL scheme (e.g. `zed` for `zed://` urls) to be
    /// opened by the current app.
    ///
    /// On some platforms (e.g. macOS) you may be able to register URL schemes
    /// as part of app distribution, but this method exists to let you register
    /// schemes at runtime.
    pub fn register_url_scheme(&self, scheme: &str) -> Task<Result<()>> {
        self.platform.register_url_scheme(scheme)
    }

    /// Returns the full pathname of the current app bundle.
    ///
    /// Returns an error if the app is not being run from a bundle.
    pub fn app_path(&self) -> Result<PathBuf> {
        self.platform.app_path()
    }

    /// On Linux, returns the name of the compositor in use.
    ///
    /// Returns an empty string on other platforms.
    pub fn compositor_name(&self) -> &'static str {
        self.platform.compositor_name()
    }

    /// Returns the file URL of the executable with the specified name in the application bundle
    pub fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf> {
        self.platform.path_for_auxiliary_executable(name)
    }

    /// Displays a platform modal for selecting paths.
    ///
    /// When one or more paths are selected, they'll be relayed asynchronously via the returned oneshot channel.
    /// If cancelled, a `None` will be relayed instead.
    /// May return an error on Linux if the file picker couldn't be opened.
    pub fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        self.platform.prompt_for_paths(options)
    }

    /// Displays a platform modal for selecting a new path where a file can be saved.
    ///
    /// The provided directory will be used to set the initial location.
    /// When a path is selected, it is relayed asynchronously via the returned oneshot channel.
    /// If cancelled, a `None` will be relayed instead.
    /// May return an error on Linux if the file picker couldn't be opened.
    pub fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        self.platform.prompt_for_new_path(directory, suggested_name)
    }

    /// Reveals the specified path at the platform level, such as in Finder on macOS.
    pub fn reveal_path(&self, path: &Path) {
        self.platform.reveal_path(path)
    }

    /// Opens the specified path with the system's default application.
    pub fn open_with_system(&self, path: &Path) {
        self.platform.open_with_system(path)
    }

    /// Returns whether the user has configured scrollbars to auto-hide at the platform level.
    pub fn should_auto_hide_scrollbars(&self) -> bool {
        self.platform.should_auto_hide_scrollbars()
    }

    /// Restarts the application.
    pub fn restart(&mut self) {
        self.restart_observers
            .clone()
            .retain(&(), |observer| observer(self));
        self.platform.restart(self.restart_path.take())
    }

    /// Sets the path to use when restarting the application.
    pub fn set_restart_path(&mut self, path: PathBuf) {
        self.restart_path = Some(path);
    }

    /// Returns the HTTP client for the application.
    pub fn http_client(&self) -> Arc<dyn HttpClient> {
        self.http_client.clone()
    }

    /// Sets the HTTP client for the application.
    pub fn set_http_client(&mut self, new_client: Arc<dyn HttpClient>) {
        self.http_client = new_client;
    }

    /// Configures when the application should automatically quit.
    /// By default, [`QuitMode::Default`] is used.
    pub fn set_quit_mode(&mut self, mode: QuitMode) {
        self.quit_mode = mode;
    }

    /// Selects how the runtime reports re-entrancy contract violations
    /// (K15, Phase 0-K). The default is
    /// [`ReentryMode::Strict`](crate::reentrancy::ReentryMode::Strict) in
    /// `cfg(test)` and
    /// [`ReentryMode::Loose`](crate::reentrancy::ReentryMode::Loose) in
    /// release. Tests typically leave this at the test-default `Strict` so
    /// silent re-entry bugs surface as `error!` log events.
    pub fn set_reentry_mode(&mut self, mode: crate::reentrancy::ReentryMode) {
        self.reentry_mode = mode;
    }

    /// Returns the SVG renderer used by the application.
    pub fn svg_renderer(&self) -> SvgRenderer {
        self.svg_renderer.clone()
    }

    pub(crate) fn push_effect(&mut self, effect: Effect) {
        match &effect {
            Effect::Notify { emitter } => {
                if !self.pending_notifications.insert(*emitter) {
                    return;
                }
            }
            Effect::NotifyGlobalObservers { global_type } => {
                if !self.pending_global_notifications.insert(*global_type) {
                    return;
                }
            }
            _ => {}
        };

        self.pending_effects.push_back(effect);
    }

    /// Called at the end of [`App::update`] to complete any side effects
    /// such as notifying observers, emitting events, etc.
    ///
    /// Pre-K04 legacy entry: drains every placement with no deadline,
    /// preserving observable behavior for every existing callsite. Routes
    /// through [`App::flush_effects_at`] with [`FlushScope::Legacy`].
    ///
    /// Once Task 23 wires `App::run_frame`, phase-aware boundaries will call
    /// [`App::flush_effects_at`] directly with [`FlushScope::Phase(_)`].
    fn flush_effects(&mut self) {
        self.flush_effects_at(FlushScope::Legacy, None);
    }

    /// K04 phase-aware deadline-aware drain.
    ///
    /// Drains `pending_effects` according to the `scope`. Non-[`Effect::Defer`]
    /// variants always drain (they have no placement). [`Effect::Defer`] effects
    /// are filtered: only those whose placement is admissible at the current
    /// scope drain; the rest are carried over for a future phase boundary.
    ///
    /// FIFO is preserved *within* each placement — the carry-over rebuild walks
    /// the carry queue in reverse so that the original insertion order survives.
    ///
    /// When `deadline` is `Some(dl)` and `Instant::now() > dl`, the drain breaks
    /// and emits a single `WARN` log; any remaining effects (admissible or not)
    /// stay in `pending_effects` for the next phase boundary. This implements
    /// the "break-and-requeue" policy from `docs/promt.md` §3.1 / K04 design
    /// decision D7. Pre-K04 legacy callers pass `deadline: None` and are never
    /// time-budgeted.
    ///
    /// Per-flush behavior:
    ///
    /// - The dedup invariants on `Notify` / `NotifyGlobalObservers` (first-insert
    ///   wins; `pending_notifications` / `pending_global_notifications` sets) are
    ///   preserved verbatim because the dedup state lives outside this function.
    /// - `event_arena.clear()` runs at the end of a [`FlushScope::Legacy`] drain
    ///   only. Phase-aware drains (Task 23+) leave the arena to `run_frame`'s
    ///   end-of-frame cleanup.
    /// - The `cfg(test, feature = "test-support")` auto-redraw block runs only
    ///   under [`FlushScope::Legacy`]; Task 21 will gate it behind
    ///   `App::auto_advance_frames_on_flush`.
    ///
    /// K15 coexistence: this function holds the same re-entry guarantees as the
    /// pre-K04 `flush_effects` — callers wrap it with `flushing_effects` /
    /// `pending_updates` so a Defer callback that issues a nested `update_window`
    /// continues to follow the K15 contract.
    pub(crate) fn flush_effects_at(&mut self, scope: FlushScope, deadline: Option<Instant>) {
        use collections::VecDeque;

        // Effects whose placement does not match the current scope. Drained at
        // a future phase boundary. Sized to zero allocations in the common case
        // (no carry-over); grows as needed.
        let mut carry: VecDeque<Effect> = VecDeque::new();
        let mut budget_exhausted = false;

        loop {
            self.release_dropped_entities();
            self.release_dropped_focus_handles();

            // Deadline check (axiom P4): only the `EffectFlush` budget enforces
            // break-and-requeue. Non-effect phases pass `deadline: None`.
            if let Some(dl) = deadline {
                if Instant::now() > dl {
                    budget_exhausted = true;
                    break;
                }
            }

            if let Some(effect) = self.pending_effects.pop_front() {
                // Placement filter (Defer only — other effects always drain).
                if let Effect::Defer { placement, .. } = &effect {
                    if !scope.admits(*placement) {
                        carry.push_back(effect);
                        continue;
                    }
                }

                match effect {
                    Effect::Notify { emitter } => {
                        self.apply_notify_effect(emitter);
                    }

                    Effect::Emit {
                        emitter,
                        event_type,
                        event,
                    } => self.apply_emit_effect(emitter, event_type, &*event),

                    Effect::RefreshWindows => {
                        self.apply_refresh_effect();
                    }

                    Effect::NotifyGlobalObservers { global_type } => {
                        self.apply_notify_global_observers_effect(global_type);
                    }

                    Effect::Defer {
                        placement: _placement,
                        callback,
                    } => {
                        self.apply_defer_effect(callback);
                    }
                    Effect::EntityCreated {
                        entity,
                        tid,
                        window,
                    } => {
                        self.apply_entity_created_effect(entity, tid, window);
                    }
                }
            } else {
                // No admissible effects remain in the queue. Under Legacy scope,
                // run the test-mode auto-redraw block when the
                // `auto_advance_frames_on_flush` flag (K04 Task 21) is set;
                // under a Phase scope, skip — `App::run_frame` (Task 23) owns
                // the redraw.
                #[cfg(any(test, feature = "test-support"))]
                if matches!(scope, FlushScope::Legacy) && self.auto_advance_frames_on_flush {
                    for window in self
                        .windows
                        .values()
                        .filter_map(|window| {
                            let window = window.as_deref()?;
                            window.invalidator.is_dirty().then_some(window.handle)
                        })
                        .collect::<Vec<_>>()
                    {
                        self.update_window(window, |_, window, cx| window.draw(cx).clear())
                            .unwrap();
                    }
                }

                if self.pending_effects.is_empty() {
                    // Truly empty for this scope. Under Legacy, clear the event
                    // arena (matches pre-K04 behavior). Under Phase, leave the
                    // arena to `run_frame`'s end-of-frame cleanup (Task 23).
                    if matches!(scope, FlushScope::Legacy) {
                        self.event_arena.clear();
                    }
                    break;
                }
                // The test-mode redraw may have pushed new effects via
                // `cx.notify(_)`; loop back to drain them.
            }
        }

        // Restore the carry-over to the front of `pending_effects`, preserving
        // FIFO order within each placement. `push_front` reverses, so we walk
        // the carry in reverse to put the first carried effect at the head.
        if !carry.is_empty() {
            for effect in carry.into_iter().rev() {
                self.pending_effects.push_front(effect);
            }
        }

        if budget_exhausted {
            // Single rate-limited WARN per overrun (one per phase per frame).
            // Per `docs/promt.md` §3.1: dispatch/tick/paint paths must not log
            // per element or per frame — this `WARN` is the only allowed
            // committed log on the effect-flush path.
            log::warn!(
                "flui-core: effect-flush exceeded budget at {:?} boundary; \
                 remainder deferred to next phase boundary",
                scope
            );
        }
    }

    /// Repeatedly called during `flush_effects` to release any entities whose
    /// reference count has become zero. We invoke any release observers before dropping
    /// each entity.
    fn release_dropped_entities(&mut self) {
        loop {
            let dropped = self.entities.take_dropped();
            if dropped.is_empty() {
                break;
            }

            for (entity_id, mut entity) in dropped {
                self.observers.remove(&entity_id);
                self.event_listeners.remove(&entity_id);
                for release_callback in self.release_listeners.remove(&entity_id) {
                    release_callback(entity.as_mut(), self);
                }
            }
        }
    }

    /// Repeatedly called during `flush_effects` to handle a focused handle being dropped.
    fn release_dropped_focus_handles(&mut self) {
        self.focus_handles
            .clone()
            .write()
            .retain(|handle_id, focus| {
                if focus.ref_count.load(SeqCst) == 0 {
                    for window_handle in self.windows() {
                        window_handle
                            .update(self, |_, window, _| {
                                if window.focus == Some(handle_id) {
                                    window.blur();
                                }
                            })
                            .unwrap();
                    }
                    false
                } else {
                    true
                }
            });
    }

    fn apply_notify_effect(&mut self, emitter: EntityId) {
        self.pending_notifications.remove(&emitter);

        self.observers
            .clone()
            .retain(&emitter, |handler| handler(self));
    }

    fn apply_emit_effect(&mut self, emitter: EntityId, event_type: TypeId, event: &dyn Any) {
        self.event_listeners
            .clone()
            .retain(&emitter, |(stored_type, handler)| {
                if *stored_type == event_type {
                    handler(event, self)
                } else {
                    true
                }
            });
    }

    fn apply_refresh_effect(&mut self) {
        for window in self.windows.values_mut() {
            if let Some(window) = window.as_deref_mut() {
                window.refreshing = true;
                window.invalidator.set_dirty(true);
            }
        }
    }

    fn apply_notify_global_observers_effect(&mut self, type_id: TypeId) {
        self.pending_global_notifications.remove(&type_id);
        self.global_observers
            .clone()
            .retain(&type_id, |observer| observer(self));
    }

    fn apply_defer_effect(&mut self, callback: Box<dyn FnOnce(&mut Self) + 'static>) {
        callback(self);
    }

    fn apply_entity_created_effect(
        &mut self,
        entity: AnyEntity,
        tid: TypeId,
        window: Option<WindowId>,
    ) {
        self.new_entity_observers.clone().retain(&tid, |observer| {
            if let Some(id) = window {
                self.update_window_id(id, {
                    let entity = entity.clone();
                    |_, window, cx| (observer)(entity, &mut Some(window), cx)
                })
                .expect("All windows should be off the stack when flushing effects");
            } else {
                (observer)(entity.clone(), &mut None, self)
            }
            true
        });
    }

    fn update_window_id<T, F>(&mut self, id: WindowId, update: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        // K15 (Phase 0-K): same-window re-entry detection. Check BEFORE
        // taking the window from storage, so on `Err` we don't have to
        // restore it. Different-window re-entry is allowed (independent
        // borrow domains).
        if self.window_update_stack.contains(&id) {
            let err = crate::reentrancy::ReentryError::NestedWindowUpdate(id);
            crate::reentrancy::log_reentry(self.reentry_mode, &err);
            return Err(anyhow::Error::from(err))
                .context("nested update_window for the same window");
        }
        self.update(|cx| {
            let mut window = cx.windows.get_mut(id)?.take()?;

            let root_view = window.root.clone().unwrap();
            let window_id = window.handle.id;

            let stack_len = cx.window_update_stack.len();
            cx.window_update_stack.push(window_id);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                update(root_view, &mut window, cx)
            }));
            // K15: pop FIRST so `window_closed_observers` (dispatched below)
            // see the closing window's id ABSENT from the stack, matching
            // pre-K15 semantics. Do NOT relocate this pop.
            cx.window_update_stack.truncate(stack_len);

            let result = match result {
                Ok(result) => result,
                Err(payload) => {
                    cx.windows
                        .get_mut(id)
                        .expect("taken window slot must still exist")
                        .replace(window);
                    std::panic::resume_unwind(payload);
                }
            };

            if window.removed {
                cx.window_handles.remove(&id);
                cx.windows.remove(id);

                cx.window_closed_observers.clone().retain(&(), |callback| {
                    callback(cx);
                    true
                });

                let quit_on_empty = match cx.quit_mode {
                    QuitMode::Explicit => false,
                    QuitMode::LastWindowClosed => true,
                    QuitMode::Default => cfg!(not(target_os = "macos")),
                };

                if quit_on_empty && cx.windows.is_empty() {
                    cx.quit();
                }
            } else {
                cx.windows.get_mut(id)?.replace(window);
            }

            Some(result)
        })
        .context("window not found")
    }

    /// Creates an `AsyncApp`, which can be cloned and has a static lifetime
    /// so it can be held across `await` points.
    pub fn to_async(&self) -> AsyncApp {
        AsyncApp {
            app: self.this.clone(),
            background_executor: self.background_executor.clone(),
            foreground_executor: self.foreground_executor.clone(),
        }
    }

    /// Obtains a reference to the executor, which can be used to spawn futures.
    pub fn background_executor(&self) -> &BackgroundExecutor {
        &self.background_executor
    }

    /// Obtains a reference to the executor, which can be used to spawn futures.
    pub fn foreground_executor(&self) -> &ForegroundExecutor {
        if self.quitting {
            panic!("Can't spawn on main thread after on_app_quit")
        };
        &self.foreground_executor
    }

    /// Spawns the future returned by the given function on the main thread. The closure will be invoked
    /// with [AsyncApp], which allows the application state to be accessed across await points.
    #[track_caller]
    pub fn spawn<AsyncFn, R>(&self, f: AsyncFn) -> Task<R>
    where
        AsyncFn: AsyncFnOnce(&mut AsyncApp) -> R + 'static,
        R: 'static,
    {
        if self.quitting {
            debug_panic!("Can't spawn on main thread after on_app_quit")
        };

        let mut cx = self.to_async();

        self.foreground_executor
            .spawn(async move { f(&mut cx).await }.boxed_local())
    }

    /// Spawns the future returned by the given function on the main thread with
    /// the given priority. The closure will be invoked with [AsyncApp], which
    /// allows the application state to be accessed across await points.
    pub fn spawn_with_priority<AsyncFn, R>(&self, priority: Priority, f: AsyncFn) -> Task<R>
    where
        AsyncFn: AsyncFnOnce(&mut AsyncApp) -> R + 'static,
        R: 'static,
    {
        if self.quitting {
            debug_panic!("Can't spawn on main thread after on_app_quit")
        };

        let mut cx = self.to_async();

        self.foreground_executor
            .spawn_with_priority(priority, async move { f(&mut cx).await }.boxed_local())
    }

    /// K04 Task 26: returns the current frame phase.
    ///
    /// Returns [`FramePhase::Idle`] outside of `App::run_frame` (i.e. when no
    /// frame is in flight). Advanced by `App::run_frame` (Task 23) at every
    /// phase transition.
    ///
    /// Cheap (one field read). Used by:
    ///
    /// - Inspector (K22) for read-only phase queries.
    /// - Framework-tier tests (Task 38 phase-order tests, SF05) to assert
    ///   phase invariants without inspecting internal fields.
    /// - K04 panic-safety paths to know which phase to wind down from.
    ///
    /// A subscribing variant (`App::observe_phase(...)`) is reserved for K22
    /// inspector intro but NOT shipped in K04.
    pub fn current_phase(&self) -> FramePhase {
        self.current_phase
    }

    /// K04 (Task 17 surface): returns a reference to the per-App
    /// [`FrameClock`](crate::frame::clock::FrameClock).
    ///
    /// Read-only. Animation tick (Task 30), `AnimationController::value()`
    /// (Task 31), post-frame callbacks, layout cache hash keys, and any
    /// time-sensitive phase code reads `Instant` values from this clock
    /// instead of `Instant::now()` (axiom P3).
    ///
    /// Outside a frame, [`FrameClock::in_frame()`] returns `false`. Calling
    /// [`FrameClock::now()`] outside a frame triggers a debug assertion in
    /// `cfg(debug_assertions)` and returns the last-sampled value in release.
    pub fn frame_clock(&self) -> &crate::frame::clock::FrameClock {
        &self.frame_clock
    }

    /// K04 Task 27: returns the always-on per-frame telemetry recorded for the
    /// most recent `App::run_frame` (or `TestApp::advance_frame`) call.
    ///
    /// Before the first frame, returns a [`FrameProfile::default()`] — every
    /// field is zero / empty. After the first frame, fields reflect that
    /// frame's measurements.
    ///
    /// Cheap (one `Copy` of ~32 bytes); safe to read on every frame. Detailed
    /// per-phase `Duration` data lives on the flag-gated
    /// [`Self::frame_profile_detailed`].
    pub fn frame_profile(&self) -> &FrameProfile {
        &self.frame_profile
    }

    /// K04 Task 27: returns the detailed per-phase telemetry if profiling is
    /// enabled (default `cfg!(debug_assertions)`), otherwise `None`.
    ///
    /// Enable explicitly via [`Self::set_profiling_enabled(true)`](Self::set_profiling_enabled)
    /// from a host that needs the full per-phase breakdown.
    pub fn frame_profile_detailed(&self) -> Option<&FrameProfileDetailed> {
        self.profiling_enabled
            .then_some(&self.frame_profile_detailed)
    }

    /// K04 Task 27: toggles detailed per-phase profiling. Default
    /// `cfg!(debug_assertions)` — debug builds collect timings, release builds
    /// stay cold unless explicitly opted in.
    ///
    /// Flipping from `true` to `false` does NOT clear the most recently
    /// recorded [`FrameProfileDetailed`]; subsequent `frame_profile_detailed()`
    /// calls simply return `None` until profiling is re-enabled.
    pub fn set_profiling_enabled(&mut self, enabled: bool) {
        self.profiling_enabled = enabled;
    }

    /// K04 Task 27: returns whether detailed per-phase profiling is enabled.
    pub fn profiling_enabled(&self) -> bool {
        self.profiling_enabled
    }

    /// K04 Task 35: schedule an App-wide callback to run at the start of the
    /// next [`Self::run_frame`]'s [`PreFrame`](crate::frame::FramePhase::PreFrame)
    /// phase, AFTER per-window `Window::on_pre_frame` callbacks. Use this
    /// for cross-window setup (input replay, telemetry seed, focus moves
    /// across windows).
    pub fn on_pre_frame(&mut self, callback: impl FnOnce(&mut App) + 'static) {
        self.app_pre_frame_callbacks.push(Box::new(callback));
    }

    /// K04 Task 35: schedule an App-wide callback to run in the current
    /// frame's [`PostFrame`](crate::frame::FramePhase::PostFrame) phase,
    /// AFTER per-window `Window::on_post_frame` callbacks. Use for
    /// cross-window post-paint work (telemetry export, observers that
    /// span multiple windows).
    ///
    /// Per axiom P5, callbacks scheduled via this API MUST NOT mutate
    /// elements directly. Queue mutations via
    /// `cx.defer_to(DeferPlacement::NextFrameStart, ...)` instead.
    pub fn on_post_frame(&mut self, callback: impl FnOnce(&mut App) + 'static) {
        self.app_post_frame_callbacks.push(Box::new(callback));
    }

    /// K04 Task 23: seven-phase frame entry.
    ///
    /// Walks the K04 phase pipeline for a single frame on the given window:
    ///
    /// ```text
    /// PreFrame → AnimationTick → Build (no-op) → Layout → Prepaint → Paint → PostFrame
    /// ```
    ///
    /// Between every phase boundary the placement-aware effect drain
    /// ([`App::flush_effects_at`] with [`FlushScope::Phase`]) drains any
    /// `Effect::Defer` whose [`DeferPlacement`] matches the boundary. Other
    /// effect variants (`Notify`, `Emit`, ...) drain regardless of placement.
    ///
    /// # Frame clock (axiom P3)
    ///
    /// Calls [`FrameClock::begin_frame()`](crate::frame::clock::FrameClock::begin_frame)
    /// at the start of `PreFrame` and [`FrameClock::end_frame()`](crate::frame::clock::FrameClock::end_frame)
    /// at the end of `PostFrame`. Every consumer that reads
    /// `cx.frame_clock().now()` inside the seven phases sees the same
    /// `Instant`. The `frame_index` increments on every successful call;
    /// panicking phases call [`abort_frame_after_panic`](Self::abort_frame_after_panic)
    /// which flips `in_frame()` back to `false` but leaves `frame_index`
    /// and `last_sampled` "stuck dirty" (panic-safety contract D9).
    ///
    /// # K04 staged rollout
    ///
    /// As of K04 Phase 2 Task 23 this is invokable but NOT yet wired into the
    /// platform `on_request_frame` callback at `window.rs:1257-1314`. The
    /// production draw path still flows through `window.draw()` inline. The
    /// only K04 consumer is [`TestApp::advance_frame`](crate::TestApp::advance_frame).
    /// A follow-up spec migrates the platform callback to call `run_frame`
    /// (deferred to avoid coupling the K04 contract to platform-side
    /// thermal / input-rate machinery — see Task 14 design note).
    ///
    /// # Errors
    ///
    /// Returns `Err` only when the window handle is not found. Phase panics
    /// are caught and reported via the returned [`FrameOutcome`]; the App
    /// stays usable.
    pub fn run_frame(&mut self, handle: AnyWindowHandle) -> Result<FrameOutcome> {
        use std::panic::AssertUnwindSafe;

        // Sanity: nested `run_frame` is forbidden — K04 axiom P5 routes
        // re-entry through `cx.defer_to(...)`, not nested frame entry.
        debug_assert_eq!(
            self.current_phase,
            FramePhase::Idle,
            "App::run_frame called while a frame is already in flight"
        );

        // Sample the wall clock once for the whole frame measurement. Detailed
        // per-phase durations are captured only when `profiling_enabled`; the
        // always-on `FrameProfile` only records the total.
        let frame_start = Instant::now();
        self.frame_clock.begin_frame();

        // Reset detailed profile at the start of the frame so per-phase
        // durations from the previous frame don't bleed through.
        if self.profiling_enabled {
            self.frame_profile_detailed.reset();
        }

        // Walk phases. We catch panics inside the body so a panicking phase
        // does not poison the App. The `AssertUnwindSafe` is safe here because
        // every borrowed field is `&mut self` (single-threaded), and
        // `abort_frame_after_panic` restores the invariants downstream.
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            // K04 Tasks 32/33/35: PreFrame body —
            //   (1) drain `request_next_frame` flags (Task 32),
            //   (2) drain per-window `Window::on_pre_frame` callbacks (Task 33),
            //   (3) drain App-level `App::on_pre_frame` callbacks (Task 35).
            self.run_phase(FramePhase::PreFrame, |app| {
                app.drain_request_next_frame_flags();
                app.drain_pre_frame_callbacks();
            });

            // K04 Task 30: walk the active animation set, tick each target
            // with the per-frame `FrameClock::now()`, and emit notifications
            // for continuing targets so dependent views re-render.
            self.run_phase(FramePhase::AnimationTick, |app| {
                app.drive_animation_tick();
            });

            // Build is reserved as a no-op slot for SF05 (`BuildOwner::flush_dirty()`).
            // Enter and exit immediately; no effects drain because phase-boundary
            // drains happen in `run_phase`.
            self.run_phase(FramePhase::Build, |_app| { /* reserved */ });

            self.run_phase(
                FramePhase::Layout,
                |_app| { /* layout body lands when K20 layout cache wires up */ },
            );

            // Prepaint + Paint: the legacy `Window::draw` body does both today.
            // K04 Task 24 keeps `Window::DrawPhase` as a strict sub-state of
            // these two phases; until SF06 splits the prepaint pass out, we
            // call `window.draw()` inside the `Paint` phase and let the
            // `Prepaint` phase run boundary-only effect drains.
            self.run_phase(
                FramePhase::Prepaint,
                |_app| { /* prepaint body lands when DrawPhase split lands */ },
            );

            let primitive_count = self
                .update_window_id(handle.id, |_, window, cx| {
                    cx.current_phase = FramePhase::Paint;
                    let _arena_clear_needed = window.draw(cx);
                    // Best-effort primitive count from the rendered scene.
                    // `rendered_frame.scene` is the canonical source; surface
                    // exposed via a single read at the end of Paint.
                    window.rendered_frame.scene.len()
                })
                .ok()
                .unwrap_or(0);
            // `run_phase` advances `current_phase` itself; the inline override
            // above is necessary because `window.draw()` is run via
            // `update_window_id`, which can re-enter `flush_effects`. Reset to
            // Paint after the draw so the post-Paint boundary drain runs with
            // the correct phase.
            self.current_phase = FramePhase::Paint;
            self.flush_effects_at(FlushScope::Phase(FramePhase::Paint), None);

            // K04 Tasks 34/35: drain Window-level post-frame callbacks for
            // every open window. App-level App::on_post_frame callbacks
            // also drain here (added by Task 35 alongside the Window-level
            // variant).
            self.run_phase(FramePhase::PostFrame, |app| {
                app.drain_post_frame_callbacks();
            });

            primitive_count
        }));

        // Restore Idle regardless of outcome.
        let outcome = match result {
            Ok(primitive_count) => {
                self.current_phase = FramePhase::Idle;
                self.frame_clock.end_frame();
                FrameOutcome {
                    frame_index: self.frame_clock.frame_index(),
                    panicked_phase: None,
                    primitive_count: u32::try_from(primitive_count).unwrap_or(u32::MAX),
                }
            }
            Err(_panic) => {
                // K04 panic-safety contract (axiom P8): wind down the phase
                // machine and clear in-flight scratch, but leave `frame_clock`
                // and animation-set "stuck dirty" so the next frame can
                // recover. `current_phase` at the moment of panic is recorded
                // for the outcome — `abort_frame_after_panic` resets it to
                // Idle.
                let panicked_phase = self.current_phase;
                self.abort_frame_after_panic(panicked_phase);
                FrameOutcome {
                    frame_index: self.frame_clock.frame_index(),
                    panicked_phase: Some(panicked_phase),
                    primitive_count: 0,
                }
            }
        };

        // Populate the always-on profile. Heavy detailed measurements (per-phase
        // Durations, overrun magnitudes) are populated by `run_phase` when
        // `profiling_enabled`; here we only record the cheap fields.
        self.frame_profile.frame_index = outcome.frame_index;
        self.frame_profile.frame_duration_total = frame_start.elapsed();
        self.frame_profile.primitive_count = outcome.primitive_count;
        self.frame_profile.active_animations = 0; // populated by Task 30 (animation tick wiring)

        Ok(outcome)
    }

    /// K04 Task 23 internal: run one phase body bracketed by phase-aware
    /// effect drains and (when profiling is enabled) duration measurements.
    ///
    /// Called by [`Self::run_frame`] for every phase. Maintains the invariants
    /// listed in the [`FramePhase`] discriminant docstrings:
    ///
    /// - `current_phase` is set to `phase` for the duration of `body`.
    /// - Pre-phase boundary drain: admissible `DeferPlacement`s at this phase
    ///   drain BEFORE `body` runs (so a `cx.defer_to(NextFrameStart, …)`
    ///   queued during the previous frame's `PostFrame` fires at the start of
    ///   `PreFrame`).
    /// - Post-phase boundary drain: re-runs after `body` to catch effects the
    ///   body itself queued at the same placement.
    fn run_phase<F: FnOnce(&mut App)>(&mut self, phase: FramePhase, body: F) {
        self.current_phase = phase;

        // Pre-phase boundary drain.
        self.flush_effects_at(FlushScope::Phase(phase), None);

        let phase_start = self.profiling_enabled.then(Instant::now);

        body(self);

        // Post-phase boundary drain captures effects queued by `body`.
        self.flush_effects_at(FlushScope::Phase(phase), None);

        if let Some(start) = phase_start {
            // Record per-phase duration for `FrameProfileDetailed`. The
            // array index matches `FramePhase::as_index()`.
            let dur = start.elapsed();
            let idx = phase.as_index() as usize;
            if idx < FramePhase::COUNT {
                self.frame_profile_detailed.per_phase[idx] = dur;
            }
        }
    }

    /// K04 Task 32 internal: drain every open window's `request_next_frame`
    /// flag and mark the invalidator dirty for any window that had the flag
    /// set. Called from the `PreFrame` phase body so the test-driven
    /// `App::run_frame` path (which bypasses the platform `on_request_frame`
    /// callback) still observes `request_animation_frame` requests.
    ///
    /// Idempotent: a window with the flag already cleared is a no-op.
    fn drain_request_next_frame_flags(&mut self) {
        // Collect window IDs to avoid borrowing `self.windows` while we
        // mutate the invalidator. `windows` is `SlotMap` — small in practice.
        let ids: Vec<WindowId> = self
            .windows
            .iter()
            .filter_map(|(id, slot)| slot.as_ref().map(|_| id))
            .collect();
        for id in ids {
            if let Some(Some(window)) = self.windows.get(id)
                && window.request_next_frame.replace(false)
            {
                window.invalidator.set_dirty(true);
            }
        }
    }

    /// K04 Tasks 33/35 internal: drain per-window pre-frame callbacks
    /// (`Window::on_pre_frame`) and then App-level pre-frame callbacks
    /// (`App::on_pre_frame`). Per-window first so App-level callbacks can
    /// observe the resolved per-window state.
    fn drain_pre_frame_callbacks(&mut self) {
        // Per-window pre-frame callbacks. The legacy storage is
        // `Rc<RefCell<Vec<FrameCallback>>>` — drain by taking the vec.
        let ids: Vec<WindowId> = self
            .windows
            .iter()
            .filter_map(|(id, slot)| slot.as_ref().map(|_| id))
            .collect();
        for id in ids {
            let drained: Vec<_> = match self.windows.get(id) {
                Some(Some(window)) => RefCell::borrow_mut(&window.next_frame_callbacks)
                    .drain(..)
                    .collect(),
                _ => continue,
            };
            if drained.is_empty() {
                continue;
            }
            let _ = self.update_window_id(id, |_, window, cx| {
                for callback in drained {
                    callback(window, cx);
                }
            });
        }

        // App-level pre-frame callbacks. `mem::take` ensures callbacks that
        // re-queue (push a new `on_pre_frame`) fire in the NEXT frame, not
        // this one — avoids an unbounded same-frame loop.
        let pending = std::mem::take(&mut self.app_pre_frame_callbacks);
        for cb in pending {
            cb(self);
        }
    }

    /// K04 Tasks 34/35 internal: drain per-window post-frame callbacks
    /// (`Window::on_post_frame`) followed by App-level post-frame callbacks
    /// (`App::on_post_frame`).
    fn drain_post_frame_callbacks(&mut self) {
        let ids: Vec<WindowId> = self
            .windows
            .iter()
            .filter_map(|(id, slot)| slot.as_ref().map(|_| id))
            .collect();
        for id in ids {
            let drained: SmallVec<[_; 4]> = match self.windows.get(id) {
                Some(Some(window)) => RefCell::borrow_mut(&window.post_frame_callbacks)
                    .drain(..)
                    .collect(),
                _ => continue,
            };
            if drained.is_empty() {
                continue;
            }
            let _ = self.update_window_id(id, |_, window, cx| {
                for callback in drained {
                    callback(window, cx);
                }
            });
        }

        let pending = std::mem::take(&mut self.app_post_frame_callbacks);
        for cb in pending {
            cb(self);
        }
    }

    /// K04 Task 30 internal: walk [`Self::active_animations`], tick each
    /// target with the per-frame [`FrameClock::now()`](crate::frame::clock::FrameClock::now),
    /// emit `Effect::Notify` for continuing targets, and drop `Done` entries.
    ///
    /// Called only from [`Self::run_frame`] inside the `AnimationTick` phase.
    /// Bypasses `update_entity` / `start_update` to avoid triggering a legacy
    /// effect flush mid-phase — phase-aware drains are owned by `run_phase`.
    ///
    /// Updates [`FrameProfile::active_animations`] with the number of targets
    /// visited (after dropping dead weaks).
    fn drive_animation_tick(&mut self) {
        // Early-out: most frames have no active animations, and we want this
        // path to be cheap. `is_empty` is O(1) on `FxHashMap`.
        if self.active_animations.is_empty() {
            return;
        }

        let now = self.frame_clock.now();
        // Take the map so we can lease individual entities (lease holds
        // `&mut self.entities`, which conflicts with iterating
        // `self.active_animations` borrowed from `self`). Re-insert at the
        // end with only the continuing targets.
        let active = std::mem::take(&mut self.active_animations);
        let mut keep: FxHashMap<TickTargetId, WeakEntity<crate::AnimationController>> =
            FxHashMap::default();
        keep.reserve(active.len());

        let mut visited: usize = 0;

        for (id, weak) in active {
            let Some(entity) = weak.upgrade() else {
                // Entity dropped between frames — silently evict.
                continue;
            };
            visited += 1;

            let mut lease = self.entities.lease(&entity);
            let outcome: TickOutcome = lease.tick(now);
            self.entities.end_lease(lease);

            match outcome {
                TickOutcome::Continue => {
                    // Notify so observers (animated views, listener
                    // subscriptions) re-render on the next phase boundary.
                    // Effect::Notify is deduplicated on insert, so repeat
                    // notifies in the same frame collapse to one observable
                    // event per entity.
                    let emitter = entity.entity_id();
                    self.push_effect(Effect::Notify { emitter });
                    keep.insert(id, entity.downgrade());
                }
                TickOutcome::Done => {
                    // Drop the entry; the controller can re-register itself
                    // when a new segment starts (forward / reverse / animate_*).
                }
            }
        }

        self.active_animations = keep;
        self.frame_profile.active_animations = visited;
    }

    /// K04 Task 25: panic-safe phase wind-down.
    ///
    /// Mirrors [`App::abort_update_after_panic`]: restores App-level state so
    /// the App stays usable after a panic inside a frame phase. Called from the
    /// `catch_unwind` boundary in `App::run_frame` (Task 23) when a phase body
    /// panics.
    ///
    /// What is restored (axiom P8):
    ///
    /// - `current_phase = FramePhase::Idle` — phase machine is reset.
    /// - `flushing_effects = false` — drain-loop guard cleared.
    /// - `pending_updates` decremented if positive (mirrors
    ///   `abort_update_after_panic`).
    /// - `frame_clock` marks the frame as ended via
    ///   [`FrameClock::abort_frame()`](crate::frame::clock::FrameClock::abort_frame),
    ///   preserving `last_sampled` and `frame_index` for post-mortem telemetry
    ///   but flipping `in_frame()` back to `false`.
    ///
    /// What is left "stuck dirty" — drains naturally on the next frame:
    ///
    /// - Active animation set (controllers tick again next frame).
    /// - `pending_effects` queue (drains at the next phase boundary).
    /// - Window invalidator (already dirty → forces redraw).
    ///
    /// Window-level cleanup of `next_frame` (the in-flight scene buffer) is
    /// the responsibility of `App::run_frame`'s panic catch path (Task 23),
    /// not this method — `App` cannot reach into a specific window's frame
    /// buffer without knowing which window was panicking.
    pub(crate) fn abort_frame_after_panic(&mut self, _phase: FramePhase) {
        // Note: the `_phase` argument is informational for now — the cleanup
        // is uniform across phases. Task 23 may use it for log context.
        self.current_phase = FramePhase::Idle;
        self.flushing_effects = false;
        if self.pending_updates > 0 {
            self.pending_updates -= 1;
        }
        self.frame_clock.abort_frame();
    }

    /// Schedules the given function to be run at the end of the current effect cycle, allowing entities
    /// that are currently on the stack to be returned to the app.
    ///
    /// Observable behavior is preserved by routing through K04
    /// [`DeferPlacement::EndOfUpdate`] — the placement that drains at every
    /// phase boundary. Callers that want a specific later phase (next frame
    /// start, post-frame, idle) should use [`App::defer_to`] instead.
    pub fn defer(&mut self, f: impl FnOnce(&mut App) + 'static) {
        self.push_effect(Effect::Defer {
            placement: DeferPlacement::EndOfUpdate,
            callback: Box::new(f),
        });
    }

    /// K04 placement-aware deferred callback. Schedules `f` to run at the next
    /// `placement` boundary (next phase boundary for `EndOfUpdate`, next
    /// `PreFrame` for `NextFrameStart`, next `PostFrame` for `PostFrame`, next
    /// `Idle` for `Idle`).
    ///
    /// As of K04 Phase 2 Task 18 the API is wired but `flush_effects` does not
    /// yet filter by placement (Task 20). For now every placement drains
    /// identically to `EndOfUpdate`. Task 20 will make placements observable;
    /// downstream code that calls `defer_to` today will pick up the corrected
    /// drain semantics automatically when Task 20 lands.
    pub fn defer_to(&mut self, placement: DeferPlacement, f: impl FnOnce(&mut App) + 'static) {
        self.push_effect(Effect::Defer {
            placement,
            callback: Box::new(f),
        });
    }

    /// Accessor for the application's asset source, which is provided when constructing the `App`.
    pub fn asset_source(&self) -> &Arc<dyn AssetSource> {
        &self.asset_source
    }

    /// Accessor for the text system.
    pub fn text_system(&self) -> &Arc<TextSystem> {
        &self.text_system
    }

    /// Check whether a global of the given type has been assigned.
    pub fn has_global<G: Global>(&self) -> bool {
        self.globals_by_type.contains_key(&TypeId::of::<G>())
    }

    /// Access the global of the given type. Panics if a global for that type has not been assigned.
    #[track_caller]
    pub fn global<G: Global>(&self) -> &G {
        self.globals_by_type
            .get(&TypeId::of::<G>())
            .map(|any_state| any_state.downcast_ref::<G>().unwrap())
            .with_context(|| format!("no state of type {} exists", type_name::<G>()))
            .unwrap()
    }

    /// Access the global of the given type if a value has been assigned.
    pub fn try_global<G: Global>(&self) -> Option<&G> {
        self.globals_by_type
            .get(&TypeId::of::<G>())
            .map(|any_state| any_state.downcast_ref::<G>().unwrap())
    }

    /// Access the global of the given type mutably. Panics if a global for that type has not been assigned.
    #[track_caller]
    pub fn global_mut<G: Global>(&mut self) -> &mut G {
        let global_type = TypeId::of::<G>();
        self.push_effect(Effect::NotifyGlobalObservers { global_type });
        self.globals_by_type
            .get_mut(&global_type)
            .and_then(|any_state| any_state.downcast_mut::<G>())
            .with_context(|| format!("no state of type {} exists", type_name::<G>()))
            .unwrap()
    }

    /// Access the global of the given type mutably. A default value is assigned if a global of this type has not
    /// yet been assigned.
    pub fn default_global<G: Global + Default>(&mut self) -> &mut G {
        let global_type = TypeId::of::<G>();
        self.push_effect(Effect::NotifyGlobalObservers { global_type });
        self.globals_by_type
            .entry(global_type)
            .or_insert_with(|| Box::<G>::default())
            .downcast_mut::<G>()
            .unwrap()
    }

    /// Sets the value of the global of the given type.
    pub fn set_global<G: Global>(&mut self, global: G) {
        let global_type = TypeId::of::<G>();
        self.push_effect(Effect::NotifyGlobalObservers { global_type });
        self.globals_by_type.insert(global_type, Box::new(global));
    }

    /// Clear all stored globals. Does not notify global observers.
    #[cfg(any(test, feature = "test-support"))]
    pub fn clear_globals(&mut self) {
        self.globals_by_type.drain();
    }

    /// Remove the global of the given type from the app context. Does not notify global observers.
    pub fn remove_global<G: Global>(&mut self) -> G {
        let global_type = TypeId::of::<G>();
        self.push_effect(Effect::NotifyGlobalObservers { global_type });
        *self
            .globals_by_type
            .remove(&global_type)
            .unwrap_or_else(|| panic!("no global added for {}", std::any::type_name::<G>()))
            .downcast()
            .unwrap()
    }

    /// Register a callback to be invoked when a global of the given type is updated.
    pub fn observe_global<G: Global>(
        &mut self,
        mut f: impl FnMut(&mut Self) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.global_observers.insert(
            TypeId::of::<G>(),
            Box::new(move |cx| {
                f(cx);
                true
            }),
        );
        self.defer(move |_| activate());
        subscription
    }

    /// Move the global of the given type to the stack.
    #[track_caller]
    pub(crate) fn lease_global<G: Global>(&mut self) -> GlobalLease<G> {
        GlobalLease::new(
            self.globals_by_type
                .remove(&TypeId::of::<G>())
                .with_context(|| format!("no global registered of type {}", type_name::<G>()))
                .unwrap(),
        )
    }

    /// Restore the global of the given type after it is moved to the stack.
    pub(crate) fn end_global_lease<G: Global>(&mut self, lease: GlobalLease<G>) {
        let global_type = TypeId::of::<G>();

        self.push_effect(Effect::NotifyGlobalObservers { global_type });
        self.globals_by_type.insert(global_type, lease.global);
    }

    pub(crate) fn new_entity_observer(
        &self,
        key: TypeId,
        value: NewEntityListener,
    ) -> Subscription {
        let (subscription, activate) = self.new_entity_observers.insert(key, value);
        activate();
        subscription
    }

    /// Arrange for the given function to be invoked whenever a view of the specified type is created.
    /// The function will be passed a mutable reference to the view along with an appropriate context.
    pub fn observe_new<T: 'static>(
        &self,
        on_new: impl 'static + Fn(&mut T, Option<&mut Window>, &mut Context<T>),
    ) -> Subscription {
        self.new_entity_observer(
            TypeId::of::<T>(),
            Box::new(
                move |any_entity: AnyEntity, window: &mut Option<&mut Window>, cx: &mut App| {
                    any_entity
                        .downcast::<T>()
                        .unwrap()
                        .update(cx, |entity_state, cx| {
                            on_new(entity_state, window.as_deref_mut(), cx)
                        })
                },
            ),
        )
    }

    /// Observe the release of a entity. The callback is invoked after the entity
    /// has no more strong references but before it has been dropped.
    pub fn observe_release<T>(
        &self,
        handle: &Entity<T>,
        on_release: impl FnOnce(&mut T, &mut App) + 'static,
    ) -> Subscription
    where
        T: 'static,
    {
        let (subscription, activate) = self.release_listeners.insert(
            handle.entity_id(),
            Box::new(move |entity, cx| {
                let entity = entity.downcast_mut().expect("invalid entity type");
                on_release(entity, cx)
            }),
        );
        activate();
        subscription
    }

    /// Observe the release of a entity. The callback is invoked after the entity
    /// has no more strong references but before it has been dropped.
    pub fn observe_release_in<T>(
        &self,
        handle: &Entity<T>,
        window: &Window,
        on_release: impl FnOnce(&mut T, &mut Window, &mut App) + 'static,
    ) -> Subscription
    where
        T: 'static,
    {
        let window_handle = window.handle;
        self.observe_release(handle, move |entity, cx| {
            let _ = window_handle.update(cx, |_, window, cx| on_release(entity, window, cx));
        })
    }

    /// Register a callback to be invoked when a keystroke is received by the application
    /// in any window. Note that this fires after all other action and event mechanisms have resolved
    /// and that this API will not be invoked if the event's propagation is stopped.
    pub fn observe_keystrokes(
        &mut self,
        mut f: impl FnMut(&KeystrokeEvent, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        fn inner(
            keystroke_observers: &SubscriberSet<(), KeystrokeObserver>,
            handler: KeystrokeObserver,
        ) -> Subscription {
            let (subscription, activate) = keystroke_observers.insert((), handler);
            activate();
            subscription
        }

        inner(
            &self.keystroke_observers,
            Box::new(move |event, window, cx| {
                f(event, window, cx);
                true
            }),
        )
    }

    /// Register a callback to be invoked when a keystroke is received by the application
    /// in any window. Note that this fires _before_ all other action and event mechanisms have resolved
    /// unlike [`App::observe_keystrokes`] which fires after. This means that `cx.stop_propagation` calls
    /// within interceptors will prevent action dispatch
    pub fn intercept_keystrokes(
        &mut self,
        mut f: impl FnMut(&KeystrokeEvent, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        fn inner(
            keystroke_interceptors: &SubscriberSet<(), KeystrokeObserver>,
            handler: KeystrokeObserver,
        ) -> Subscription {
            let (subscription, activate) = keystroke_interceptors.insert((), handler);
            activate();
            subscription
        }

        inner(
            &self.keystroke_interceptors,
            Box::new(move |event, window, cx| {
                f(event, window, cx);
                true
            }),
        )
    }

    /// Register key bindings.
    pub fn bind_keys(&mut self, bindings: impl IntoIterator<Item = KeyBinding>) {
        self.keymap.borrow_mut().add_bindings(bindings);
        self.pending_effects.push_back(Effect::RefreshWindows);
    }

    /// Clear all key bindings in the app.
    pub fn clear_key_bindings(&mut self) {
        self.keymap.borrow_mut().clear();
        self.pending_effects.push_back(Effect::RefreshWindows);
    }

    /// Get all key bindings in the app.
    pub fn key_bindings(&self) -> Rc<RefCell<Keymap>> {
        self.keymap.clone()
    }

    /// Register a global handler for actions invoked via the keyboard. These handlers are run at
    /// the end of the bubble phase for actions, and so will only be invoked if there are no other
    /// handlers or if they called `cx.propagate()`.
    pub fn on_action<A: Action>(
        &mut self,
        listener: impl Fn(&A, &mut Self) + 'static,
    ) -> &mut Self {
        self.global_action_listeners
            .entry(TypeId::of::<A>())
            .or_default()
            .push(Rc::new(move |action, phase, cx| {
                if phase == DispatchPhase::Bubble {
                    let action = action.downcast_ref().unwrap();
                    listener(action, cx)
                }
            }));
        self
    }

    /// Event handlers propagate events by default. Call this method to stop dispatching to
    /// event handlers with a lower z-index (mouse) or higher in the tree (keyboard). This is
    /// the opposite of [`Self::propagate`]. It's also possible to cancel a call to [`Self::propagate`] by
    /// calling this method before effects are flushed.
    pub fn stop_propagation(&mut self) {
        self.propagate_event = false;
    }

    /// Action handlers stop propagation by default during the bubble phase of action dispatch
    /// dispatching to action handlers higher in the element tree. This is the opposite of
    /// [`Self::stop_propagation`]. It's also possible to cancel a call to [`Self::stop_propagation`] by calling
    /// this method before effects are flushed.
    pub fn propagate(&mut self) {
        self.propagate_event = true;
    }

    /// Build an action from some arbitrary data, typically a keymap entry.
    pub fn build_action(
        &self,
        name: &str,
        data: Option<serde_json::Value>,
    ) -> std::result::Result<Box<dyn Action>, ActionBuildError> {
        self.actions.build_action(name, data)
    }

    /// Get all action names that have been registered. Note that registration only allows for
    /// actions to be built dynamically, and is unrelated to binding actions in the element tree.
    pub fn all_action_names(&self) -> &[&'static str] {
        self.actions.all_action_names()
    }

    /// Returns key bindings that invoke the given action on the currently focused element, without
    /// checking context. Bindings are returned in the order they were added. For display, the last
    /// binding should take precedence.
    pub fn all_bindings_for_input(&self, input: &[Keystroke]) -> Vec<KeyBinding> {
        RefCell::borrow(&self.keymap).all_bindings_for_input(input)
    }

    /// Get all non-internal actions that have been registered, along with their schemas.
    pub fn action_schemas(
        &self,
        generator: &mut schemars::SchemaGenerator,
    ) -> Vec<(&'static str, Option<schemars::Schema>)> {
        self.actions.action_schemas(generator)
    }

    /// Get the schema for a specific action by name.
    /// Returns `None` if the action is not found.
    /// Returns `Some(None)` if the action exists but has no schema.
    /// Returns `Some(Some(schema))` if the action exists and has a schema.
    pub fn action_schema_by_name(
        &self,
        name: &str,
        generator: &mut schemars::SchemaGenerator,
    ) -> Option<Option<schemars::Schema>> {
        self.actions.action_schema_by_name(name, generator)
    }

    /// Get a map from a deprecated action name to the canonical name.
    pub fn deprecated_actions_to_preferred_actions(&self) -> &HashMap<&'static str, &'static str> {
        self.actions.deprecated_aliases()
    }

    /// Get a map from an action name to the deprecation messages.
    pub fn action_deprecation_messages(&self) -> &HashMap<&'static str, &'static str> {
        self.actions.deprecation_messages()
    }

    /// Get a map from an action name to the documentation.
    pub fn action_documentation(&self) -> &HashMap<&'static str, &'static str> {
        self.actions.documentation()
    }

    /// Register a callback to be invoked when the application is about to quit.
    /// It is not possible to cancel the quit event at this point.
    pub fn on_app_quit<Fut>(
        &self,
        mut on_quit: impl FnMut(&mut App) -> Fut + 'static,
    ) -> Subscription
    where
        Fut: 'static + Future<Output = ()>,
    {
        let (subscription, activate) = self.quit_observers.insert(
            (),
            Box::new(move |cx| {
                let future = on_quit(cx);
                future.boxed_local()
            }),
        );
        activate();
        subscription
    }

    /// Register a callback to be invoked when the application is about to restart.
    ///
    /// These callbacks are called before any `on_app_quit` callbacks.
    pub fn on_app_restart(&self, mut on_restart: impl 'static + FnMut(&mut App)) -> Subscription {
        let (subscription, activate) = self.restart_observers.insert(
            (),
            Box::new(move |cx| {
                on_restart(cx);
                true
            }),
        );
        activate();
        subscription
    }

    /// Register a callback to be invoked when a window is closed
    /// The window is no longer accessible at the point this callback is invoked.
    pub fn on_window_closed(&self, mut on_closed: impl FnMut(&mut App) + 'static) -> Subscription {
        let (subscription, activate) = self.window_closed_observers.insert((), Box::new(on_closed));
        activate();
        subscription
    }

    pub(crate) fn clear_pending_keystrokes(&mut self) {
        for window in self.windows() {
            window
                .update(self, |_, window, cx| {
                    if window.pending_input_keystrokes().is_some() {
                        window.clear_pending_keystrokes();
                        window.pending_input_changed(cx);
                    }
                })
                .ok();
        }
    }

    /// Checks if the given action is bound in the current context, as defined by the app's current focus,
    /// the bindings in the element tree, and any global action listeners.
    pub fn is_action_available(&mut self, action: &dyn Action) -> bool {
        let mut action_available = false;
        if let Some(window) = self.active_window()
            && let Ok(window_action_available) =
                window.update(self, |_, window, cx| window.is_action_available(action, cx))
        {
            action_available = window_action_available;
        }

        action_available
            || self
                .global_action_listeners
                .contains_key(&action.as_any().type_id())
    }

    /// Sets the menu bar for this application. This will replace any existing menu bar.
    pub fn set_menus(&self, menus: impl IntoIterator<Item = Menu>) {
        let menus: Vec<Menu> = menus.into_iter().collect();
        self.platform.set_menus(menus, &self.keymap.borrow());
    }

    /// Gets the menu bar for this application.
    pub fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        self.platform.get_menus()
    }

    /// Sets the right click menu for the app icon in the dock
    pub fn set_dock_menu(&self, menus: Vec<MenuItem>) {
        self.platform.set_dock_menu(menus, &self.keymap.borrow())
    }

    /// Performs the action associated with the given dock menu item, only used on Windows for now.
    pub fn perform_dock_menu_action(&self, action: usize) {
        self.platform.perform_dock_menu_action(action);
    }

    /// Adds given path to the bottom of the list of recent paths for the application.
    /// The list is usually shown on the application icon's context menu in the dock,
    /// and allows to open the recent files via that context menu.
    /// If the path is already in the list, it will be moved to the bottom of the list.
    pub fn add_recent_document(&self, path: &Path) {
        self.platform.add_recent_document(path);
    }

    /// Updates the jump list with the updated list of recent paths for the application, only used on Windows for now.
    /// Note that this also sets the dock menu on Windows.
    pub fn update_jump_list(
        &self,
        menus: Vec<MenuItem>,
        entries: Vec<SmallVec<[PathBuf; 2]>>,
    ) -> Task<Vec<SmallVec<[PathBuf; 2]>>> {
        self.platform.update_jump_list(menus, entries)
    }

    /// Dispatch an action to the currently active window or global action handler
    /// See [`crate::Action`] for more information on how actions work
    pub fn dispatch_action(&mut self, action: &dyn Action) {
        if let Some(active_window) = self.active_window() {
            active_window
                .update(self, |_, window, cx| {
                    window.dispatch_action(action.boxed_clone(), cx)
                })
                .log_err();
        } else {
            self.dispatch_global_action(action);
        }
    }

    fn dispatch_global_action(&mut self, action: &dyn Action) {
        self.propagate_event = true;

        if let Some(mut global_listeners) = self
            .global_action_listeners
            .remove(&action.as_any().type_id())
        {
            for listener in &global_listeners {
                listener(action.as_any(), DispatchPhase::Capture, self);
                if !self.propagate_event {
                    break;
                }
            }

            global_listeners.extend(
                self.global_action_listeners
                    .remove(&action.as_any().type_id())
                    .unwrap_or_default(),
            );

            self.global_action_listeners
                .insert(action.as_any().type_id(), global_listeners);
        }

        if self.propagate_event
            && let Some(mut global_listeners) = self
                .global_action_listeners
                .remove(&action.as_any().type_id())
        {
            for listener in global_listeners.iter().rev() {
                listener(action.as_any(), DispatchPhase::Bubble, self);
                if !self.propagate_event {
                    break;
                }
            }

            global_listeners.extend(
                self.global_action_listeners
                    .remove(&action.as_any().type_id())
                    .unwrap_or_default(),
            );

            self.global_action_listeners
                .insert(action.as_any().type_id(), global_listeners);
        }
    }

    /// Is there currently something being dragged?
    pub fn has_active_drag(&self) -> bool {
        self.active_drag.is_some()
    }

    /// Gets the cursor style of the currently active drag operation.
    pub fn active_drag_cursor_style(&self) -> Option<CursorStyle> {
        self.active_drag.as_ref().and_then(|drag| drag.cursor_style)
    }

    /// Stops active drag and clears any related effects.
    pub fn stop_active_drag(&mut self, window: &mut Window) -> bool {
        if self.active_drag.is_some() {
            self.active_drag = None;
            window.refresh();
            true
        } else {
            false
        }
    }

    /// Sets the cursor style for the currently active drag operation.
    pub fn set_active_drag_cursor_style(
        &mut self,
        cursor_style: CursorStyle,
        window: &mut Window,
    ) -> bool {
        if let Some(ref mut drag) = self.active_drag {
            drag.cursor_style = Some(cursor_style);
            window.refresh();
            true
        } else {
            false
        }
    }

    /// Set the prompt renderer for GPUI. This will replace the default or platform specific
    /// prompts with this custom implementation.
    pub fn set_prompt_builder(
        &mut self,
        renderer: impl Fn(
            PromptLevel,
            &str,
            Option<&str>,
            &[PromptButton],
            PromptHandle,
            &mut Window,
            &mut App,
        ) -> RenderablePromptHandle
        + 'static,
    ) {
        self.prompt_builder = Some(PromptBuilder::Custom(Box::new(renderer)));
    }

    /// Reset the prompt builder to the default implementation.
    pub fn reset_prompt_builder(&mut self) {
        self.prompt_builder = Some(PromptBuilder::Default);
    }

    /// Remove an asset from GPUI's cache
    pub fn remove_asset<A: Asset>(&mut self, source: &A::Source) {
        let asset_id = (TypeId::of::<A>(), hash(source));
        self.loading_assets.remove(&asset_id);
    }

    /// Asynchronously load an asset, if the asset hasn't finished loading this will return None.
    ///
    /// Note that the multiple calls to this method will only result in one `Asset::load` call at a
    /// time, and the results of this call will be cached
    pub fn fetch_asset<A: Asset>(&mut self, source: &A::Source) -> (Shared<Task<A::Output>>, bool) {
        let asset_id = (TypeId::of::<A>(), hash(source));
        let mut is_first = false;
        let task = self
            .loading_assets
            .remove(&asset_id)
            .map(|boxed_task| *boxed_task.downcast::<Shared<Task<A::Output>>>().unwrap())
            .unwrap_or_else(|| {
                is_first = true;
                let future = A::load(source.clone(), self);

                self.background_executor().spawn(future).shared()
            });

        self.loading_assets.insert(asset_id, Box::new(task.clone()));

        (task, is_first)
    }

    /// Obtain a new [`FocusHandle`], which allows you to track and manipulate the keyboard focus
    /// for elements rendered within this window.
    #[track_caller]
    pub fn focus_handle(&self) -> FocusHandle {
        FocusHandle::new(&self.focus_handles)
    }

    /// Tell GPUI that an entity has changed and observers of it should be notified.
    pub fn notify(&mut self, entity_id: EntityId) {
        let window_invalidators = mem::take(
            self.window_invalidators_by_entity
                .entry(entity_id)
                .or_default(),
        );

        if window_invalidators.is_empty() {
            if self.pending_notifications.insert(entity_id) {
                self.pending_effects
                    .push_back(Effect::Notify { emitter: entity_id });
            }
        } else {
            for invalidator in window_invalidators.values() {
                invalidator.invalidate_view(entity_id, self);
            }
        }

        self.window_invalidators_by_entity
            .insert(entity_id, window_invalidators);
    }

    /// Returns the name for this [`App`].
    #[cfg(any(test, feature = "test-support", debug_assertions))]
    pub fn get_name(&self) -> Option<&'static str> {
        self.name
    }

    /// Returns `true` if the platform file picker supports selecting a mix of files and directories.
    pub fn can_select_mixed_files_and_dirs(&self) -> bool {
        self.platform.can_select_mixed_files_and_dirs()
    }

    /// Removes an image from the sprite atlas on all windows.
    ///
    /// If the current window is being updated, it will be removed from `App.windows`, you can use `current_window` to specify the current window.
    /// This is a no-op if the image is not in the sprite atlas.
    pub fn drop_image(&mut self, image: Arc<RenderImage>, current_window: Option<&mut Window>) {
        // remove the texture from all other windows
        for window in self.windows.values_mut().flatten() {
            _ = window.drop_image(image.clone());
        }

        // remove the texture from the current window
        if let Some(window) = current_window {
            _ = window.drop_image(image);
        }
    }

    /// Sets the renderer for the inspector.
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub fn set_inspector_renderer(&mut self, f: crate::InspectorRenderer) {
        self.inspector_renderer = Some(f);
    }

    /// Registers a renderer specific to an inspector state.
    #[cfg(any(feature = "inspector", debug_assertions))]
    pub fn register_inspector_element<T: 'static, R: crate::IntoElement>(
        &mut self,
        f: impl 'static + Fn(crate::InspectorElementId, &T, &mut Window, &mut App) -> R,
    ) {
        self.inspector_element_registry.register(f);
    }

    /// Initializes gpui's default colors for the application.
    ///
    /// These colors can be accessed through `cx.default_colors()`.
    pub fn init_colors(&mut self) {
        self.set_global(GlobalColors(Arc::new(Colors::default())));
    }

    /// Returns the current platform brightness preference (light or dark).
    ///
    /// Falls back to `Brightness::Light` if `SystemBrightness` has not been set.
    pub fn platform_brightness(&self) -> crate::Brightness {
        self.try_global::<crate::SystemBrightness>()
            .map(|sb| sb.0)
            .unwrap_or(crate::Brightness::Light)
    }

    /// Get the system locale.
    ///
    /// Falls back to `Locale::default()` (English) if `SystemLocale` has not been set.
    pub fn locale(&self) -> crate::Locale {
        self.try_global::<crate::SystemLocale>()
            .map(|sl| sl.0.clone())
            .unwrap_or_default()
    }

    /// Get the text direction for the current locale.
    pub fn text_direction(&self) -> crate::TextDirection {
        crate::TextDirection::from_language(&self.locale().language)
    }
}

impl AppContext for App {
    /// Builds an entity that is owned by the application.
    ///
    /// The given function will be invoked with a [`Context`] and must return an object representing the entity. An
    /// [`Entity`] handle will be returned, which can be used to access the entity in a context.
    fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T> {
        self.update(|cx| {
            let slot = cx.entities.reserve();
            let handle = slot.clone();
            let entity = build_entity(&mut Context::new_context(cx, slot.downgrade()));

            cx.push_effect(Effect::EntityCreated {
                entity: handle.clone().into_any(),
                tid: TypeId::of::<T>(),
                window: cx.window_update_stack.last().cloned(),
            });

            cx.entities.insert(slot, entity);
            handle
        })
    }

    fn reserve_entity<T: 'static>(&mut self) -> Reservation<T> {
        Reservation(self.entities.reserve())
    }

    fn insert_entity<T: 'static>(
        &mut self,
        reservation: Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T> {
        self.update(|cx| {
            let slot = reservation.0;
            let entity = build_entity(&mut Context::new_context(cx, slot.downgrade()));
            cx.entities.insert(slot, entity)
        })
    }

    /// Updates the entity referenced by the given handle. The function is passed a mutable reference to the
    /// entity along with a `Context` for the entity.
    fn update_entity<T: 'static, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R {
        let id = handle.entity_id();
        // K15 (Phase 0-K): same-entity re-entry detection. Multi-entity cycles
        // (A → B → A) are caught by `EntityMap::double_lease_panic` after this
        // field has been replaced with the inner entity's id; both paths
        // produce the same `ReentryError::NestedEntityUpdate(_)` Display.
        if self.currently_updating_entity == Some(id) {
            let err = crate::reentrancy::ReentryError::NestedEntityUpdate(id);
            crate::reentrancy::log_reentry(self.reentry_mode, &err);
            // ROADMAP K15: "Either queue (preferred) or panic with structured
            // error (acceptable)." `update_entity` returns generic `R`, so
            // queueing is not viable; panic with the structured Display.
            std::panic::panic_any(err);
        }
        self.update(|cx| {
            // Save and replace `currently_updating_entity` so multi-entity
            // chains (A calls B, B's lease may itself call C, etc.) restore
            // correctly on return. Top-level enters with `None`, restores to
            // `None`. K15 contract.
            //
            // K07 closes K15 Known Limitation #6 by catching panics inside
            // this update frame long enough to restore the entity id and
            // return the leased entity before resuming the original unwind.
            let previous_entity = cx.currently_updating_entity.replace(id);
            let mut entity = cx.entities.lease(handle);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut context = Context::new_context(cx, handle.downgrade());
                update(&mut entity, &mut context)
            }));
            cx.currently_updating_entity = previous_entity;
            cx.entities.end_lease(entity);
            match result {
                Ok(result) => result,
                Err(payload) => std::panic::resume_unwind(payload),
            }
        })
    }

    fn as_mut<'a, T>(&'a mut self, handle: &Entity<T>) -> GpuiBorrow<'a, T>
    where
        T: 'static,
    {
        GpuiBorrow::new(handle.clone(), self)
    }

    fn read_entity<T, R>(&self, handle: &Entity<T>, read: impl FnOnce(&T, &App) -> R) -> R
    where
        T: 'static,
    {
        let entity = self.entities.read(handle);
        read(entity, self)
    }

    fn update_window<T, F>(&mut self, handle: AnyWindowHandle, update: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        self.update_window_id(handle.id, update)
    }

    fn read_window<T, R>(
        &self,
        window: &WindowHandle<T>,
        read: impl FnOnce(Entity<T>, &App) -> R,
    ) -> Result<R>
    where
        T: 'static,
    {
        let window = self
            .windows
            .get(window.id)
            .context("window not found")?
            .as_deref()
            .expect("attempted to read a window that is already on the stack");

        let root_view = window.root.clone().unwrap();
        let view = root_view
            .downcast::<T>()
            .map_err(|_| anyhow!("root view's type has changed"))?;

        Ok(read(view, self))
    }

    fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static,
    {
        self.background_executor.spawn(future)
    }

    fn read_global<G, R>(&self, callback: impl FnOnce(&G, &App) -> R) -> R
    where
        G: Global,
    {
        let mut g = self.global::<G>();
        callback(g, self)
    }
}

/// K04 Task 23 return value from [`App::run_frame`]. Reports the index of the
/// frame that was just run, the panicking phase (if any), and the scene
/// primitive count produced by `Paint`.
///
/// # Stability
///
/// `#[non_exhaustive]` — future telemetry (e.g. dropped-frame estimate,
/// vsync drift) lands additively as new fields.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameOutcome {
    /// Monotonic frame index — matches [`FrameClock::frame_index()`](crate::frame::clock::FrameClock::frame_index)
    /// at the moment the frame finished (or panicked).
    pub frame_index: u64,

    /// `Some(phase)` if `App::run_frame` caught a panic inside `phase`'s body.
    /// `None` for a normal frame.
    pub panicked_phase: Option<FramePhase>,

    /// Scene primitive count produced during `Paint`. `0` when the frame
    /// panicked or the target window was missing.
    pub primitive_count: u32,
}

/// K04 drain scope for [`App::flush_effects_at`]. Selects which [`DeferPlacement`]s
/// drain at the current call.
///
/// Two flavors:
/// - [`FlushScope::Legacy`] — drain every placement, no boundary filter. Used by
///   pre-K04 callers (currently `App::finish_update` via [`App::flush_effects`]).
///   Preserves pre-K04 observable behavior until Task 23 wires `App::run_frame`.
/// - [`FlushScope::Phase`] — drain only placements admissible at the given
///   [`FramePhase`] boundary. Used by [`App::run_frame`] (Task 23) to implement
///   the per-phase drain table from K04 design decision D1.
///
/// Admission rules (per spec D1 / D5):
///
/// | Boundary | Drains placements |
/// |---|---|
/// | `Legacy` | every placement |
/// | `Phase(_)` | `EndOfUpdate` always |
/// | `Phase(PreFrame)` | + `NextFrameStart` |
/// | `Phase(PostFrame)` | + `PostFrame` |
/// | `Phase(Idle)` | + `Idle` |
/// | other phases | only `EndOfUpdate` |
#[derive(Copy, Clone, Debug)]
pub(crate) enum FlushScope {
    /// Pre-K04 legacy entry. Drains every placement, no deadline. Used by
    /// `App::finish_update` until Task 23 wires `App::run_frame`.
    Legacy,
    /// Phase-aware entry. Drains only placements admissible at `boundary`.
    /// Used by `App::run_frame` from Task 23 onward.
    //
    // Constructor wired by Task 23 (`App::run_frame`). The dead-code warning
    // until then is intentional — the variant is part of the K04 staged rollout.
    #[allow(dead_code)]
    Phase(FramePhase),
}

impl FlushScope {
    /// Returns `true` if `placement` should drain at this scope.
    pub(crate) fn admits(self, placement: DeferPlacement) -> bool {
        match self {
            FlushScope::Legacy => true,
            FlushScope::Phase(boundary) => match (boundary, placement) {
                // EndOfUpdate drains at every phase boundary.
                (_, DeferPlacement::EndOfUpdate) => true,
                // NextFrameStart drains in PreFrame only.
                (FramePhase::PreFrame, DeferPlacement::NextFrameStart) => true,
                // PostFrame drains in PostFrame only.
                (FramePhase::PostFrame, DeferPlacement::PostFrame) => true,
                // Idle drains in Idle only.
                (FramePhase::Idle, DeferPlacement::Idle) => true,
                _ => false,
            },
        }
    }
}

/// These effects are processed at the end of each application update cycle.
pub(crate) enum Effect {
    Notify {
        emitter: EntityId,
    },
    Emit {
        emitter: EntityId,
        event_type: TypeId,
        event: ArenaBox<dyn Any>,
    },
    RefreshWindows,
    NotifyGlobalObservers {
        global_type: TypeId,
    },
    /// K04 placement-aware deferred callback. `placement` selects which phase
    /// boundary drains the callback; the default `DeferPlacement::EndOfUpdate`
    /// preserves pre-K04 observable behavior for every `App::defer(f)` callsite.
    /// `App::defer_to(placement, f)` is the new placement-aware constructor.
    ///
    /// As of K04 Phase 2 Task 19 the field is captured but not yet filtered on
    /// drain — the per-phase drain (Task 20) is the consumer that will route
    /// callbacks to the matching phase boundary. Until then, all placements
    /// drain identically to `EndOfUpdate`.
    Defer {
        placement: DeferPlacement,
        callback: Box<dyn FnOnce(&mut App) + 'static>,
    },
    EntityCreated {
        entity: AnyEntity,
        tid: TypeId,
        window: Option<WindowId>,
    },
}

impl std::fmt::Debug for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Effect::Notify { emitter } => write!(f, "Notify({})", emitter),
            Effect::Emit { emitter, .. } => write!(f, "Emit({:?})", emitter),
            Effect::RefreshWindows => write!(f, "RefreshWindows"),
            Effect::NotifyGlobalObservers { global_type } => {
                write!(f, "NotifyGlobalObservers({:?})", global_type)
            }
            Effect::Defer { placement, .. } => write!(f, "Defer({:?}, ..)", placement),
            Effect::EntityCreated { entity, .. } => write!(f, "EntityCreated({:?})", entity),
        }
    }
}

/// Wraps a global variable value during `update_global` while the value has been moved to the stack.
pub(crate) struct GlobalLease<G: Global> {
    global: Box<dyn Any>,
    global_type: PhantomData<G>,
}

impl<G: Global> GlobalLease<G> {
    fn new(global: Box<dyn Any>) -> Self {
        GlobalLease {
            global,
            global_type: PhantomData,
        }
    }
}

impl<G: Global> Deref for GlobalLease<G> {
    type Target = G;

    fn deref(&self) -> &Self::Target {
        self.global.downcast_ref().unwrap()
    }
}

impl<G: Global> DerefMut for GlobalLease<G> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.global.downcast_mut().unwrap()
    }
}

/// Contains state associated with an active drag operation, started by dragging an element
/// within the window or by dragging into the app from the underlying platform.
pub struct AnyDrag {
    /// The view used to render this drag
    pub view: AnyView,

    /// The value of the dragged item, to be dropped
    pub value: Arc<dyn Any>,

    /// This is used to render the dragged item in the same place
    /// on the original element that the drag was initiated
    pub cursor_offset: Point<Pixels>,

    /// The cursor style to use while dragging
    pub cursor_style: Option<CursorStyle>,
}

/// Contains state associated with a tooltip. You'll only need this struct if you're implementing
/// tooltip behavior on a custom element. Otherwise, use [Div::tooltip](crate::Interactivity::tooltip).
#[derive(Clone)]
pub struct AnyTooltip {
    /// The view used to display the tooltip
    pub view: AnyView,

    /// The absolute position of the mouse when the tooltip was deployed.
    pub mouse_position: Point<Pixels>,

    /// Given the bounds of the tooltip, checks whether the tooltip should still be visible and
    /// updates its state accordingly. This is needed atop the hovered element's mouse move handler
    /// to handle the case where the element is not painted (e.g. via use of `visible_on_hover`).
    pub check_visible_and_update: Rc<dyn Fn(Bounds<Pixels>, &mut Window, &mut App) -> bool>,
}

/// A keystroke event, and potentially the associated action
#[derive(Debug)]
pub struct KeystrokeEvent {
    /// The keystroke that occurred
    pub keystroke: Keystroke,

    /// The action that was resolved for the keystroke, if any
    pub action: Option<Box<dyn Action>>,

    /// The context stack at the time
    pub context_stack: Vec<KeyContext>,
}

struct NullHttpClient;

impl HttpClient for NullHttpClient {
    fn type_name(&self) -> &'static str {
        "NullHttpClient"
    }

    fn send(
        &self,
        _req: http_client::Request<http_client::AsyncBody>,
    ) -> futures::future::BoxFuture<
        'static,
        anyhow::Result<http_client::Response<http_client::AsyncBody>>,
    > {
        async move {
            anyhow::bail!("No HttpClient available");
        }
        .boxed()
    }

    fn user_agent(&self) -> Option<&http_client::http::HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }
}

/// A mutable reference to an entity owned by GPUI
pub struct GpuiBorrow<'a, T> {
    inner: Option<Lease<T>>,
    app: &'a mut App,
}

impl<'a, T: 'static> GpuiBorrow<'a, T> {
    fn new(inner: Entity<T>, app: &'a mut App) -> Self {
        app.start_update();
        let lease = app.entities.lease(&inner);
        Self {
            inner: Some(lease),
            app,
        }
    }
}

impl<'a, T: 'static> std::borrow::Borrow<T> for GpuiBorrow<'a, T> {
    fn borrow(&self) -> &T {
        self.inner.as_ref().unwrap().borrow()
    }
}

impl<'a, T: 'static> std::borrow::BorrowMut<T> for GpuiBorrow<'a, T> {
    fn borrow_mut(&mut self) -> &mut T {
        self.inner.as_mut().unwrap().borrow_mut()
    }
}

impl<'a, T: 'static> std::ops::Deref for GpuiBorrow<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().unwrap()
    }
}

impl<'a, T: 'static> std::ops::DerefMut for GpuiBorrow<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.inner.as_mut().unwrap()
    }
}

impl<'a, T> Drop for GpuiBorrow<'a, T> {
    fn drop(&mut self) {
        let lease = self.inner.take().unwrap();
        self.app.notify(lease.id);
        self.app.entities.end_lease(lease);
        self.app.finish_update();
    }
}

#[cfg(test)]
mod test {
    use std::{cell::RefCell, rc::Rc};

    use crate::{AppContext, TestAppContext};

    #[test]
    fn test_gpui_borrow() {
        let cx = TestAppContext::single();
        let observation_count = Rc::new(RefCell::new(0));

        let state = cx.update(|cx| {
            let state = cx.new(|_| false);
            cx.observe(&state, {
                let observation_count = observation_count.clone();
                move |_, _| {
                    let mut count = observation_count.borrow_mut();
                    *count += 1;
                }
            })
            .detach();

            state
        });

        cx.update(|cx| {
            // Calling this like this so that we don't clobber the borrow_mut above
            *std::borrow::BorrowMut::borrow_mut(&mut state.as_mut(cx)) = true;
        });

        cx.update(|cx| {
            state.write(cx, false);
        });

        assert_eq!(*observation_count.borrow(), 2);
    }
}
