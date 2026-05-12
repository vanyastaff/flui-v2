//! A clean testing API for GPUI applications.
//!
//! `TestApp` provides a simpler alternative to `TestAppContext` with:
//! - Automatic effect flushing after updates
//! - Clean window creation and inspection
//! - Input simulation helpers
//!
//! # Example
//! ```ignore
//! #[test]
//! fn test_my_view() {
//!     let mut app = TestApp::new();
//!
//!     let mut window = app.open_window(|window, cx| {
//!         MyView::new(window, cx)
//!     });
//!
//!     window.update(|view, window, cx| {
//!         view.do_something(cx);
//!     });
//!
//!     // Check rendered state
//!     assert_eq!(window.title(), Some("Expected Title"));
//! }
//! ```

use crate::{
    AnyWindowHandle, App, AppCell, AppContext, AsyncApp, BackgroundExecutor, BorrowAppContext,
    Bounds, ClipboardItem, Context, Entity, ForegroundExecutor, FrameOutcome, Global, InputEvent,
    Keystroke, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Platform,
    PlatformTextSystem, Point, Render, Size, Task, TestDispatcher, TestPlatform, TextSystem,
    Window, WindowBounds, WindowHandle, WindowOptions, app::GpuiMode, frame::profile::FrameProfile,
};
use std::{future::Future, rc::Rc, sync::Arc, time::Duration};

/// A test application context with a clean API.
///
/// Unlike `TestAppContext`, `TestApp` automatically flushes effects after
/// each update and provides simpler window management.
pub struct TestApp {
    app: Rc<AppCell>,
    platform: Rc<TestPlatform>,
    background_executor: BackgroundExecutor,
    foreground_executor: ForegroundExecutor,
    #[allow(dead_code)]
    dispatcher: TestDispatcher,
    text_system: Arc<TextSystem>,
}

impl TestApp {
    /// Create a new test application.
    pub fn new() -> Self {
        Self::with_seed(0)
    }

    /// Create a new test application with a specific random seed.
    pub fn with_seed(seed: u64) -> Self {
        Self::build(seed, None, Arc::new(()))
    }

    /// Create a new test application with a custom text system for real font shaping.
    pub fn with_text_system(text_system: Arc<dyn PlatformTextSystem>) -> Self {
        Self::build(0, Some(text_system), Arc::new(()))
    }

    /// Create a new test application with a custom text system and asset source.
    pub fn with_text_system_and_assets(
        text_system: Arc<dyn PlatformTextSystem>,
        asset_source: Arc<dyn crate::AssetSource>,
    ) -> Self {
        Self::build(0, Some(text_system), asset_source)
    }

    fn build(
        seed: u64,
        platform_text_system: Option<Arc<dyn PlatformTextSystem>>,
        asset_source: Arc<dyn crate::AssetSource>,
    ) -> Self {
        let dispatcher = TestDispatcher::new(seed);
        let arc_dispatcher = Arc::new(dispatcher.clone());
        let background_executor = BackgroundExecutor::new(arc_dispatcher.clone());
        let foreground_executor = ForegroundExecutor::new(arc_dispatcher);
        let platform = match platform_text_system.clone() {
            Some(ts) => TestPlatform::with_text_system(
                background_executor.clone(),
                foreground_executor.clone(),
                ts,
            ),
            None => TestPlatform::new(background_executor.clone(), foreground_executor.clone()),
        };
        let http_client = http_client::FakeHttpClient::with_404_response();
        let text_system = Arc::new(TextSystem::new(
            platform_text_system.unwrap_or_else(|| platform.text_system.clone()),
        ));

        let app = App::new_app(platform.clone(), asset_source, http_client);
        app.borrow_mut().mode = GpuiMode::test();

        Self {
            app,
            platform,
            background_executor,
            foreground_executor,
            dispatcher,
            text_system,
        }
    }

    /// Run a closure with mutable access to the App context.
    /// Automatically runs until parked after the closure completes.
    pub fn update<R>(&mut self, f: impl FnOnce(&mut App) -> R) -> R {
        let result = {
            let mut app = self.app.borrow_mut();
            app.update(f)
        };
        self.run_until_parked();
        result
    }

    /// Run a closure with read-only access to the App context.
    pub fn read<R>(&self, f: impl FnOnce(&App) -> R) -> R {
        let app = self.app.borrow();
        f(&app)
    }

    /// Create a new entity in the app.
    pub fn new_entity<T: 'static>(
        &mut self,
        build: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T> {
        self.update(|cx| cx.new(build))
    }

    /// Update an entity.
    pub fn update_entity<T: 'static, R>(
        &mut self,
        entity: &Entity<T>,
        f: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R {
        self.update(|cx| entity.update(cx, f))
    }

    /// Read an entity.
    pub fn read_entity<T: 'static, R>(
        &self,
        entity: &Entity<T>,
        f: impl FnOnce(&T, &App) -> R,
    ) -> R {
        self.read(|cx| f(entity.read(cx), cx))
    }

    /// Open a test window with the given root view, using maximized bounds.
    pub fn open_window<V: Render + 'static>(
        &mut self,
        build_view: impl FnOnce(&mut Window, &mut Context<V>) -> V,
    ) -> TestAppWindow<V> {
        let bounds = self.read(|cx| Bounds::maximized(None, cx));
        let handle = self.update(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| build_view(window, cx)),
            )
            .unwrap()
        });

        TestAppWindow {
            handle,
            app: self.app.clone(),
            platform: self.platform.clone(),
            background_executor: self.background_executor.clone(),
        }
    }

    /// Open a test window with specific options.
    pub fn open_window_with_options<V: Render + 'static>(
        &mut self,
        options: WindowOptions,
        build_view: impl FnOnce(&mut Window, &mut Context<V>) -> V,
    ) -> TestAppWindow<V> {
        let handle = self.update(|cx| {
            cx.open_window(options, |window, cx| cx.new(|cx| build_view(window, cx)))
                .unwrap()
        });

        TestAppWindow {
            handle,
            app: self.app.clone(),
            platform: self.platform.clone(),
            background_executor: self.background_executor.clone(),
        }
    }

    /// Run pending tasks until there's nothing left to do.
    pub fn run_until_parked(&self) {
        self.background_executor.run_until_parked();
    }

    /// Advance the simulated clock by the given duration.
    pub fn advance_clock(&self, duration: Duration) {
        self.background_executor.advance_clock(duration);
    }

    /// Spawn a future on the foreground executor.
    pub fn spawn<Fut, R>(&self, f: impl FnOnce(AsyncApp) -> Fut) -> Task<R>
    where
        Fut: Future<Output = R> + 'static,
        R: 'static,
    {
        self.foreground_executor.spawn(f(self.to_async()))
    }

    /// Spawn a future on the background executor.
    pub fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static,
    {
        self.background_executor.spawn(future)
    }

    /// Get an async handle to the app.
    pub fn to_async(&self) -> AsyncApp {
        AsyncApp {
            app: Rc::downgrade(&self.app),
            background_executor: self.background_executor.clone(),
            foreground_executor: self.foreground_executor.clone(),
        }
    }

    /// Get the background executor.
    pub fn background_executor(&self) -> &BackgroundExecutor {
        &self.background_executor
    }

    /// Get the foreground executor.
    pub fn foreground_executor(&self) -> &ForegroundExecutor {
        &self.foreground_executor
    }

    /// Get the text system.
    pub fn text_system(&self) -> &Arc<TextSystem> {
        &self.text_system
    }

    /// Check if a global of the given type exists.
    pub fn has_global<G: Global>(&self) -> bool {
        self.read(|cx| cx.has_global::<G>())
    }

    /// Set a global value.
    pub fn set_global<G: Global>(&mut self, global: G) {
        self.update(|cx| cx.set_global(global));
    }

    /// Read a global value.
    pub fn read_global<G: Global, R>(&self, f: impl FnOnce(&G, &App) -> R) -> R {
        self.read(|cx| f(cx.global(), cx))
    }

    /// Update a global value.
    pub fn update_global<G: Global, R>(&mut self, f: impl FnOnce(&mut G, &mut App) -> R) -> R {
        self.update(|cx| cx.update_global(f))
    }

    // Platform simulation methods

    /// Write text to the simulated clipboard.
    pub fn write_to_clipboard(&self, item: ClipboardItem) {
        self.platform.write_to_clipboard(item);
    }

    /// Read from the simulated clipboard.
    pub fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.platform.read_from_clipboard()
    }

    /// Get URLs that have been opened via `cx.open_url()`.
    pub fn opened_url(&self) -> Option<String> {
        self.platform.opened_url.borrow().clone()
    }

    /// Check if a file path prompt is pending.
    pub fn did_prompt_for_new_path(&self) -> bool {
        self.platform.did_prompt_for_new_path()
    }

    /// Simulate answering a path selection dialog.
    pub fn simulate_new_path_selection(
        &self,
        select: impl FnOnce(&std::path::Path) -> Option<std::path::PathBuf>,
    ) {
        self.platform.simulate_new_path_selection(select);
    }

    /// Check if a prompt dialog is pending.
    pub fn has_pending_prompt(&self) -> bool {
        self.platform.has_pending_prompt()
    }

    /// Simulate answering a prompt dialog.
    pub fn simulate_prompt_answer(&self, button: &str) {
        self.platform.simulate_prompt_answer(button);
    }

    /// Get all open windows.
    pub fn windows(&self) -> Vec<AnyWindowHandle> {
        self.read(|cx| cx.windows())
    }

    /// K04 Task 22: toggles the test-mode auto-redraw at
    /// `App::flush_effects_at` (the K04 Task 21 flag). Default is `true` under
    /// `cfg(test, feature = "test-support")` so pre-K04 tests keep working
    /// unchanged. Phase-order tests and other K04 / Framework-tier tests flip
    /// this to `false` and drive frames explicitly via [`Self::advance_frame`].
    ///
    /// K04+1 will flip the default to `false` once Tier-C tests migrate to
    /// `advance_frame`.
    pub fn set_auto_advance_frames(&mut self, enabled: bool) {
        let mut app = self.app.borrow_mut();
        app.auto_advance_frames_on_flush = enabled;
    }

    /// K04 Task 22: returns the most recently recorded always-on per-frame
    /// telemetry. Before the first [`Self::advance_frame`] call, every field
    /// is zero / empty (see [`FrameProfile::default()`]).
    pub fn frame_profile(&self) -> FrameProfile {
        *self.app.borrow().frame_profile()
    }

    /// K04 Task 22: drives one frame through [`App::run_frame`] on the
    /// most-recently-opened window. Returns the resulting [`FrameOutcome`].
    ///
    /// # Window selection
    ///
    /// Picks the single open window when there is exactly one; panics when
    /// there is none. With multiple windows, use [`TestAppWindow::advance_frame`]
    /// directly on the desired window for unambiguous targeting.
    pub fn advance_frame(&mut self) -> FrameOutcome {
        let handle = {
            let app = self.app.borrow();
            let mut handles = app.windows();
            assert!(
                !handles.is_empty(),
                "TestApp::advance_frame called with no open windows; open a window first"
            );
            assert_eq!(
                handles.len(),
                1,
                "TestApp::advance_frame is ambiguous with {} open windows; use TestAppWindow::advance_frame",
                handles.len()
            );
            handles.pop().unwrap()
        };
        let outcome = self
            .app
            .borrow_mut()
            .run_frame(handle)
            .expect("run_frame failed");
        self.run_until_parked();
        outcome
    }

    /// K04 Task 22: iterates [`Self::advance_frame`] `n` times. Returns a vec
    /// of per-frame outcomes in order. `n == 0` returns an empty vec without
    /// touching the App.
    pub fn advance_frames(&mut self, n: usize) -> Vec<FrameOutcome> {
        (0..n).map(|_| self.advance_frame()).collect()
    }
}

impl Default for TestApp {
    fn default() -> Self {
        Self::new()
    }
}

/// A test window with inspection and simulation capabilities.
pub struct TestAppWindow<V> {
    handle: WindowHandle<V>,
    app: Rc<AppCell>,
    platform: Rc<TestPlatform>,
    background_executor: BackgroundExecutor,
}

impl<V: 'static + Render> TestAppWindow<V> {
    /// Get the window handle.
    pub fn handle(&self) -> WindowHandle<V> {
        self.handle
    }

    /// Get the root view entity.
    pub fn root(&self) -> Entity<V> {
        let mut app = self.app.borrow_mut();
        let any_handle: AnyWindowHandle = self.handle.into();
        app.update_window(any_handle, |root_view, _, _| {
            root_view.downcast::<V>().expect("root view type mismatch")
        })
        .expect("window not found")
    }

    /// Update the root view.
    pub fn update<R>(&mut self, f: impl FnOnce(&mut V, &mut Window, &mut Context<V>) -> R) -> R {
        let result = {
            let mut app = self.app.borrow_mut();
            let any_handle: AnyWindowHandle = self.handle.into();
            app.update_window(any_handle, |root_view, window, cx| {
                let view = root_view.downcast::<V>().expect("root view type mismatch");
                view.update(cx, |view, cx| f(view, window, cx))
            })
            .expect("window not found")
        };
        self.background_executor.run_until_parked();
        result
    }

    /// Read the root view.
    pub fn read<R>(&self, f: impl FnOnce(&V, &App) -> R) -> R {
        let app = self.app.borrow();
        let view = self
            .app
            .borrow()
            .windows
            .get(self.handle.window_id())
            .and_then(|w| w.as_ref())
            .and_then(|w| w.root.clone())
            .and_then(|r| r.downcast::<V>().ok())
            .expect("window or root view not found");
        f(view.read(&app), &app)
    }

    /// Get the window title.
    pub fn title(&self) -> Option<String> {
        let app = self.app.borrow();
        app.read_window(&self.handle, |_, _cx| {
            // TODO: expose title through Window API
            None
        })
        .unwrap()
    }

    /// Simulate a keystroke.
    pub fn simulate_keystroke(&mut self, keystroke: &str) {
        let keystroke = Keystroke::parse(keystroke).unwrap();
        {
            let mut app = self.app.borrow_mut();
            let any_handle: AnyWindowHandle = self.handle.into();
            app.update_window(any_handle, |_, window, cx| {
                window.dispatch_keystroke(keystroke, cx);
            })
            .unwrap();
        }
        self.background_executor.run_until_parked();
    }

    /// Simulate multiple keystrokes (space-separated).
    pub fn simulate_keystrokes(&mut self, keystrokes: &str) {
        for keystroke in keystrokes.split(' ') {
            self.simulate_keystroke(keystroke);
        }
    }

    /// Simulate typing text.
    pub fn simulate_input(&mut self, input: &str) {
        for char in input.chars() {
            self.simulate_keystroke(&char.to_string());
        }
    }

    /// Simulate a mouse move.
    pub fn simulate_mouse_move(&mut self, position: Point<Pixels>) {
        self.simulate_event(MouseMoveEvent {
            position,
            modifiers: Default::default(),
            pressed_button: None,
        });
    }

    /// Simulate a mouse down event.
    pub fn simulate_mouse_down(&mut self, position: Point<Pixels>, button: MouseButton) {
        self.simulate_event(MouseDownEvent {
            position,
            button,
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        });
    }

    /// Simulate a mouse up event.
    pub fn simulate_mouse_up(&mut self, position: Point<Pixels>, button: MouseButton) {
        self.simulate_event(MouseUpEvent {
            position,
            button,
            modifiers: Default::default(),
            click_count: 1,
        });
    }

    /// Simulate a click at the given position.
    pub fn simulate_click(&mut self, position: Point<Pixels>, button: MouseButton) {
        self.simulate_mouse_down(position, button);
        self.simulate_mouse_up(position, button);
    }

    /// Simulate a scroll event.
    pub fn simulate_scroll(&mut self, position: Point<Pixels>, delta: Point<Pixels>) {
        self.simulate_event(crate::ScrollWheelEvent {
            position,
            delta: crate::ScrollDelta::Pixels(delta),
            modifiers: Default::default(),
            touch_phase: crate::TouchPhase::Moved,
        });
    }

    /// Simulate an input event.
    pub fn simulate_event<E: InputEvent>(&mut self, event: E) {
        let platform_input = event.to_platform_input();
        {
            let mut app = self.app.borrow_mut();
            let any_handle: AnyWindowHandle = self.handle.into();
            app.update_window(any_handle, |_, window, cx| {
                window.dispatch_event(platform_input, cx);
            })
            .unwrap();
        }
        self.background_executor.run_until_parked();
    }

    /// Simulate resizing the window.
    pub fn simulate_resize(&mut self, size: Size<Pixels>) {
        let window_id = self.handle.window_id();
        let mut app = self.app.borrow_mut();
        if let Some(Some(window)) = app.windows.get_mut(window_id) {
            if let Some(test_window) = window.platform_window.as_test() {
                test_window.simulate_resize(size);
            }
        }
        drop(app);
        self.background_executor.run_until_parked();
    }

    /// ADR-007: simulate this window being moved to (or reattached to) a
    /// different display. Drives the platform-side display swap that
    /// surfaces through `Window::bounds_changed` and fires
    /// `Window::observe_display_change` observers.
    ///
    /// Use a fresh `TestDisplay::with_id(...)` to ensure the new display
    /// reports a distinct id from the previous one.
    pub fn simulate_display_change(
        &mut self,
        new_display: std::rc::Rc<dyn crate::PlatformDisplay>,
    ) {
        let window_id = self.handle.window_id();
        let mut app = self.app.borrow_mut();
        if let Some(Some(window)) = app.windows.get_mut(window_id) {
            if let Some(test_window) = window.platform_window.as_test() {
                test_window.simulate_display_change(new_display);
            }
        }
        drop(app);
        self.background_executor.run_until_parked();
    }

    /// Force a redraw of the window.
    pub fn draw(&mut self) {
        let mut app = self.app.borrow_mut();
        let any_handle: AnyWindowHandle = self.handle.into();
        app.update_window(any_handle, |_, window, cx| {
            window.draw(cx).clear();
        })
        .unwrap();
    }

    /// K04 Task 22: drives one frame through [`App::run_frame`] on this
    /// specific window. Returns the resulting [`FrameOutcome`].
    ///
    /// Use this rather than [`TestApp::advance_frame`] when the test owns
    /// multiple windows and needs to advance a specific one.
    pub fn advance_frame(&mut self) -> FrameOutcome {
        let any_handle: AnyWindowHandle = self.handle.into();
        let outcome = self
            .app
            .borrow_mut()
            .run_frame(any_handle)
            .expect("run_frame failed");
        self.background_executor.run_until_parked();
        outcome
    }
}

impl<V> Clone for TestAppWindow<V> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle,
            app: self.app.clone(),
            platform: self.platform.clone(),
            background_executor: self.background_executor.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FocusHandle, Focusable, div, prelude::*};

    struct Counter {
        count: usize,
        focus_handle: FocusHandle,
    }

    impl Counter {
        fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
            let focus_handle = cx.focus_handle();
            Self {
                count: 0,
                focus_handle,
            }
        }

        fn increment(&mut self, _cx: &mut Context<Self>) {
            self.count += 1;
        }
    }

    impl Focusable for Counter {
        fn focus_handle(&self, _cx: &App) -> FocusHandle {
            self.focus_handle.clone()
        }
    }

    impl Render for Counter {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(format!("Count: {}", self.count))
        }
    }

    #[test]
    fn test_basic_usage() {
        let mut app = TestApp::new();

        let mut window = app.open_window(Counter::new);

        window.update(|counter, _window, cx| {
            counter.increment(cx);
        });

        window.read(|counter, _| {
            assert_eq!(counter.count, 1);
        });

        drop(window);
        app.update(|cx| cx.shutdown());
    }

    #[test]
    fn test_entity_creation() {
        let mut app = TestApp::new();

        let entity = app.new_entity(|cx| Counter {
            count: 42,
            focus_handle: cx.focus_handle(),
        });

        app.read_entity(&entity, |counter, _| {
            assert_eq!(counter.count, 42);
        });

        app.update_entity(&entity, |counter, _cx| {
            counter.count += 1;
        });

        app.read_entity(&entity, |counter, _| {
            assert_eq!(counter.count, 43);
        });
    }

    #[test]
    fn test_advance_frame_records_profile() {
        let mut app = TestApp::new();
        // Phase order needs the legacy auto-redraw OFF — otherwise effects
        // flushed during `open_window` already drew once and a subsequent
        // `advance_frame` is redundant.
        app.set_auto_advance_frames(false);

        let mut window = app.open_window(Counter::new);

        let initial = app.frame_profile().frame_index;

        let outcome = window.advance_frame();

        assert!(outcome.panicked_phase.is_none());
        assert_eq!(outcome.frame_index, initial + 1);

        let profile = app.frame_profile();
        assert_eq!(profile.frame_index, outcome.frame_index);

        drop(window);
        app.update(|cx| cx.shutdown());
    }

    #[test]
    fn test_globals() {
        let mut app = TestApp::new();

        struct MyGlobal(String);
        impl Global for MyGlobal {}

        assert!(!app.has_global::<MyGlobal>());

        app.set_global(MyGlobal("hello".into()));

        assert!(app.has_global::<MyGlobal>());

        app.read_global::<MyGlobal, _>(|global, _| {
            assert_eq!(global.0, "hello");
        });

        app.update_global::<MyGlobal, _>(|global, _| {
            global.0 = "world".into();
        });

        app.read_global::<MyGlobal, _>(|global, _| {
            assert_eq!(global.0, "world");
        });
    }

    /// ADR-007 regression test: `Window::observe_display_change` fires
    /// when the window's bound display id changes, and the window
    /// survives the swap (decision 5 — "Output disconnect does not
    /// implicitly kill a window").
    ///
    /// This locks the public API surface added by ADR-007 and the test
    /// hook (`TestAppWindow::simulate_display_change`,
    /// `TestDisplay::with_id`) that platform-glue follow-ups will reuse
    /// to verify Wayland `wl_output` add/remove and X11 XRandR paths.
    ///
    /// See `docs/research/adr/ADR-007-display-lifecycle.md`.
    #[test]
    fn adr_007_observe_display_change_fires_on_display_swap() {
        use crate::platform::TestDisplay;
        use std::cell::Cell;
        use std::rc::Rc as StdRc;

        let mut app = TestApp::new();
        let mut window = app.open_window(Counter::new);

        // Observer fires inside `Window::bounds_changed` when display_id
        // (or scale factor) changes. Capture call count via a shared
        // `Cell` — observer keeps a strong ref while it lives.
        let fire_count = StdRc::new(Cell::new(0u32));
        let fire_count_for_observer = StdRc::clone(&fire_count);
        let _subscription = window.update(move |_view, w, _cx| {
            w.observe_display_change(move |_w, _cx| {
                fire_count_for_observer.set(fire_count_for_observer.get() + 1);
            })
        });

        assert_eq!(
            fire_count.get(),
            0,
            "ADR-007: observe_display_change must not fire on registration"
        );

        // Stage the platform-side display swap, then drive
        // `Window::bounds_changed` ourselves — we want to lock the
        // observer-firing semantics inside `bounds_changed`, independent
        // of whether the test platform's resize-callback wiring has
        // already been exercised by the harness (it varies by harness
        // run order). The contract under test is "when platform reports
        // a new display id, observers fire" — both the platform-side
        // swap and the engine-side `bounds_changed` are required steps.
        let new_display = StdRc::new(TestDisplay::with_id(5));
        window.simulate_display_change(StdRc::clone(&new_display) as _);
        // Explicitly drive bounds_changed in case simulate_display_change's
        // platform callback path is a no-op (resize_callback unset on
        // freshly-opened test windows in some harness configurations).
        window.update(|_view, w, cx| {
            w.bounds_changed(cx);
        });

        assert_eq!(
            fire_count.get(),
            1,
            "ADR-007: observe_display_change must fire exactly once when \
             display_id changes from the initial TestDisplay (id=1) to the \
             swapped one (id=5). Got fire_count = {}.",
            fire_count.get()
        );

        // Window is still alive — decision 5 says output disconnect /
        // reattach must not implicitly kill the window. Read its state
        // to prove it survives.
        let count_after_swap = window.read(|view, _| view.count);
        assert_eq!(
            count_after_swap, 0,
            "ADR-007 decision 5: window state (Counter::count = 0) must \
             survive display swap unchanged"
        );

        drop(window);
        app.update(|cx| cx.shutdown());
    }
}
