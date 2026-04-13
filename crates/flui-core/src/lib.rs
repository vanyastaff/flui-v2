#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
#![allow(clippy::type_complexity)] // Not useful, GPUI makes heavy use of callbacks
#![allow(clippy::collapsible_else_if)] // False positives in platform specific code
#![allow(unused_mut)] // False positives in platform specific code

extern crate self as flui_core;
extern crate flui_macros;

#[macro_use]
mod action;
/// Animation primitives: tweens, curves, controllers, and physics simulations.
pub mod animation;
mod app;

mod arena;
mod asset_cache;
mod assets;
mod bounds_tree;
mod brightness;
mod color;
/// The default colors used by GPUI.
pub mod colors;
mod element;
mod elements;
mod executor;
mod platform_scheduler;
pub(crate) use platform_scheduler::PlatformScheduler;
mod geometry;
mod global;
mod input;
mod inspector;
mod interactive;
mod key_dispatch;
mod keymap;
mod locale;
mod local_util;
mod media_query;
mod path_builder;
mod platform;
mod platform_brightness;
pub mod prelude;
mod provider;
/// Profiling utilities for task timing and thread performance tracking.
pub mod profiler;
#[cfg(any(target_os = "windows", target_os = "linux", target_family = "wasm"))]
#[expect(missing_docs)]
pub mod queue;
mod scene;
/// The scheduler module provides task scheduling, execution, and timing primitives.
pub mod scheduler;
mod shared_string;
mod shared_uri;
mod style;
mod styled;
mod subscription;
mod svg_renderer;
mod tab_stop;
mod taffy;
#[cfg(any(test, feature = "test-support"))]
pub mod test;

#[cfg(test)]
mod lock_tests;
mod text_system;
mod view;
mod window;

#[cfg(any(test, feature = "test-support"))]
pub use proptest;

#[cfg(doc)]
pub mod _ownership_and_data_flow;

/// Do not touch, here be dragons for use by gpui_macros and such.
#[doc(hidden)]
pub mod private {
    pub use anyhow;
    pub use inventory;
    pub use schemars;
    pub use serde;
    pub use serde_json;
}

mod seal {
    /// A mechanism for restricting implementations of a trait to only those in GPUI.
    /// See: <https://predr.ag/blog/definitive-guide-to-sealed-traits-in-rust/>
    pub trait Sealed {}
}

pub use action::*;
pub use animation::*;
pub use anyhow::Result;
pub use app::*;
pub(crate) use arena::*;
pub use asset_cache::*;
pub use assets::*;
pub use brightness::*;
pub use provider::{InheritedValue, Provider, read, try_read};
pub use color::*;
pub use ctor::ctor;
pub use element::*;
pub use elements::*;
pub use executor::*;
pub use geometry::*;
pub use global::*;
pub use flui_macros::{
    AppContext, IntoElement, Render, VisualContext, derive_inspector_reflection, register_action,
    test,
};
pub use http_client;
pub use input::*;
pub use inspector::*;
pub use interactive::*;
use key_dispatch::*;
pub use keymap::*;
pub use locale::*;
pub use local_util::{FutureExt, Timeout, command};
pub use media_query::*;
pub use path_builder::*;
// Explicit re-exports from the `platform` module (replacing the former
// `pub use platform::*;` glob). See spec S01a.3 for rationale. Any new
// `pub` item in `platform.rs` must be explicitly routed through this
// block before it becomes part of the `flui_core` public surface.
// This is the prerequisite for S02 extracting the platform subtree into
// the sibling `flui-platform` crate.
//
// NOTE: the other `pub use <mod>::*;` lines in this file intentionally
// remain glob-re-exports. They will be converted case-by-case if and
// when their modules are extracted; S01a.3 is scoped to `platform::*`
// only.

// Core platform traits
pub use platform::{
    InputHandler, Platform, PlatformAtlas, PlatformDisplay, PlatformTextSystem, PlatformWindow,
    ScreenCaptureFrame, ScreenCaptureSource, ScreenCaptureStream, SourceMetadata,
};

// Display, window, and input types
pub use platform::{
    AtlasKey, AtlasTextureId, AtlasTextureKind, AtlasTextureList, AtlasTile, ClipboardEntry,
    ClipboardItem, ClipboardString, CursorStyle, Decorations, DisplayId, Image, ImageFormat,
    NoopTextSystem, PathPromptOptions, PlatformInputHandler, PromptButton, PromptLevel,
    RequestFrameOptions, ResizeEdge, TextRenderingMode, ThermalState, TileId, Tiling,
    TitlebarOptions, UTF16Selection, WindowAppearance, WindowBackgroundAppearance, WindowBounds,
    WindowControls, WindowDecorations, WindowKind, WindowOptions, WindowParams,
};

// Free functions
pub use platform::{application, background_executor, current_platform, headless};
pub use platform::get_gamma_correction_ratios;

// Crate-internal re-exports (formerly caught by the pub use platform::*
// glob; these are pub(crate) fns in the platform subtree that other
// parts of the crate reach via crate::X).
pub(crate) use platform::init_app_menus;

// app_menu (originally glob-re-exported via `platform::app_menu::*`)
pub use platform::{
    Menu, MenuItem, OsAction, OsMenu, OwnedMenu, OwnedMenuItem, OwnedOsMenu, SystemMenuType,
};

// keyboard (originally glob-re-exported via `platform::keyboard::*`)
pub use platform::{DummyKeyboardMapper, PlatformKeyboardLayout, PlatformKeyboardMapper};

// keystroke (originally glob-re-exported via `platform::keystroke::*`)
pub use platform::{
    AsKeystroke, Capslock, InvalidKeystrokeError, KEYSTROKE_PARSE_EXPECTED_MESSAGE,
    KeybindingKeystroke, Keystroke, Modifiers,
};

// Macro-support plumbing. All four items carry `#[doc(hidden)]` at
// their definition sites (`PlatformDispatcher`, `RunnableVariant`,
// `TimerResolutionGuard` in `platform.rs`; `RunnableMeta` in
// `scheduler/mod.rs`), so the re-export does not need to repeat the
// annotation — rustdoc propagates hidden through re-exports.
pub use platform::{PlatformDispatcher, RunnableMeta, RunnableVariant, TimerResolutionGuard};

// Test-support-only items (public)
#[cfg(any(test, feature = "test-support"))]
pub use platform::{
    PlatformHeadlessRenderer, TestDispatcher, TestScreenCaptureSource, TestScreenCaptureStream,
};
#[cfg(feature = "test-support")]
pub use platform::current_headless_renderer;

// Test-support-only items (crate-internal — used by app/test_context and
// other flui-core tests). TestWindow is `pub struct` at the definition
// site but demoted to pub(crate) by the `pub(crate) use window::*;`
// in platform/test.rs.
#[cfg(any(test, feature = "test-support"))]
pub(crate) use platform::{TestDisplay, TestPlatform, TestWindow};

// Per-target platform impls
#[cfg(target_os = "macos")]
pub use platform::MacPlatform;
#[cfg(target_os = "windows")]
pub use platform::WindowsPlatform;
#[cfg(all(target_os = "macos", any(test, feature = "test-support")))]
pub use platform::VisualTestPlatform;

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub use platform::guess_compositor;

#[cfg(target_family = "wasm")]
pub use platform::{single_threaded_web, web_init};

// Linux + Wayland-only module — asymmetric by design.
// `flui_core::layer_shell::*` is reachable only on that target.
#[cfg(all(target_os = "linux", feature = "wayland"))]
pub use platform::layer_shell;
pub use platform_brightness::*;
pub use profiler::*;
#[cfg(any(target_os = "windows", target_os = "linux", target_family = "wasm"))]
pub use queue::{PriorityQueueReceiver, PriorityQueueSender};
pub use refineable::*;
pub use scene::*;
pub use shared_string::*;
pub use shared_uri::*;
use std::{any::Any, future::Future};
pub use style::*;
pub use styled::*;
pub use subscription::*;
pub use svg_renderer::*;
pub(crate) use tab_stop::*;
use taffy::TaffyLayoutEngine;
pub use taffy::{AvailableSpace, LayoutId};
#[cfg(any(test, feature = "test-support"))]
pub use test::*;
pub use text_system::*;
pub use util::arc_cow::ArcCow;
pub use view::*;
pub use window::*;

/// The context trait, allows the different contexts in GPUI to be used
/// interchangeably for certain operations.
pub trait AppContext {
    /// Create a new entity in the app context.
    #[expect(
        clippy::wrong_self_convention,
        reason = "`App::new` is an ubiquitous function for creating entities"
    )]
    fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T>;

    /// Reserve a slot for a entity to be inserted later.
    /// The returned [Reservation] allows you to obtain the [EntityId] for the future entity.
    fn reserve_entity<T: 'static>(&mut self) -> Reservation<T>;

    /// Insert a new entity in the app context based on a [Reservation] previously obtained from [`reserve_entity`].
    ///
    /// [`reserve_entity`]: Self::reserve_entity
    fn insert_entity<T: 'static>(
        &mut self,
        reservation: Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T>;

    /// Update a entity in the app context.
    fn update_entity<T, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R
    where
        T: 'static;

    /// Update a entity in the app context.
    fn as_mut<'a, T>(&'a mut self, handle: &Entity<T>) -> GpuiBorrow<'a, T>
    where
        T: 'static;

    /// Read a entity from the app context.
    fn read_entity<T, R>(&self, handle: &Entity<T>, read: impl FnOnce(&T, &App) -> R) -> R
    where
        T: 'static;

    /// Update a window for the given handle.
    fn update_window<T, F>(&mut self, window: AnyWindowHandle, f: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T;

    /// Read a window off of the application context.
    fn read_window<T, R>(
        &self,
        window: &WindowHandle<T>,
        read: impl FnOnce(Entity<T>, &App) -> R,
    ) -> Result<R>
    where
        T: 'static;

    /// Spawn a future on a background thread
    fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static;

    /// Read a global from this app context
    fn read_global<G, R>(&self, callback: impl FnOnce(&G, &App) -> R) -> R
    where
        G: Global;
}

/// Returned by [Context::reserve_entity] to later be passed to [Context::insert_entity].
/// Allows you to obtain the [EntityId] for a entity before it is created.
pub struct Reservation<T>(pub(crate) Slot<T>);

impl<T: 'static> Reservation<T> {
    /// Returns the [EntityId] that will be associated with the entity once it is inserted.
    pub fn entity_id(&self) -> EntityId {
        self.0.entity_id()
    }
}

/// This trait is used for the different visual contexts in GPUI that
/// require a window to be present.
pub trait VisualContext: AppContext {
    /// The result type for window operations.
    type Result<T>;

    /// Returns the handle of the window associated with this context.
    fn window_handle(&self) -> AnyWindowHandle;

    /// Update a view with the given callback
    fn update_window_entity<T: 'static, R>(
        &mut self,
        entity: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Window, &mut Context<T>) -> R,
    ) -> Self::Result<R>;

    /// Create a new entity, with access to `Window`.
    fn new_window_entity<T: 'static>(
        &mut self,
        build_entity: impl FnOnce(&mut Window, &mut Context<T>) -> T,
    ) -> Self::Result<Entity<T>>;

    /// Replace the root view of a window with a new view.
    fn replace_root_view<V>(
        &mut self,
        build_view: impl FnOnce(&mut Window, &mut Context<V>) -> V,
    ) -> Self::Result<Entity<V>>
    where
        V: 'static + Render;

    /// Focus a entity in the window, if it implements the [`Focusable`] trait.
    fn focus<V>(&mut self, entity: &Entity<V>) -> Self::Result<()>
    where
        V: Focusable;
}

/// A trait for tying together the types of a GPUI entity and the events it can
/// emit.
pub trait EventEmitter<E: Any>: 'static {}

/// A helper trait for auto-implementing certain methods on contexts that
/// can be used interchangeably.
pub trait BorrowAppContext {
    /// Set a global value on the context.
    fn set_global<T: Global>(&mut self, global: T);
    /// Updates the global state of the given type.
    fn update_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global;
    /// Updates the global state of the given type, creating a default if it didn't exist before.
    fn update_default_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global + Default;
}

impl<C> BorrowAppContext for C
where
    C: std::borrow::BorrowMut<App>,
{
    fn set_global<G: Global>(&mut self, global: G) {
        self.borrow_mut().set_global(global)
    }

    #[track_caller]
    fn update_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global,
    {
        let mut global = self.borrow_mut().lease_global::<G>();
        let result = f(&mut global, self);
        self.borrow_mut().end_global_lease(global);
        result
    }

    fn update_default_global<G, R>(&mut self, f: impl FnOnce(&mut G, &mut Self) -> R) -> R
    where
        G: Global + Default,
    {
        self.borrow_mut().default_global::<G>();
        self.update_global(f)
    }
}

/// Information about the GPU GPUI is running on.
#[derive(Default, Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct GpuSpecs {
    /// Whether the GPU is really a fake (like `llvmpipe`) running on the CPU.
    pub is_software_emulated: bool,
    /// The name of the device, as reported by Vulkan.
    pub device_name: String,
    /// The name of the driver, as reported by Vulkan.
    pub driver_name: String,
    /// Further information about the driver, as reported by Vulkan.
    pub driver_info: String,
}
