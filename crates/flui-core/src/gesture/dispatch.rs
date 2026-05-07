//! `PointerSanitizer` and `PlatformInput` → `PointerEvent` /
//! `PointerSignalEvent` translation.
//!
//! `pub(crate)` module — internal implementation detail of the gesture
//! dispatch path. `PointerSanitizer` and `WindowPointerState` are not
//! part of the public API.
//!
//! See the design doc at
//! `docs/superpowers/specs/2026-05-06-S07-gesture-arena-design.md`
//! § "PointerSanitizer" and § "Window::dispatch_event integration".

use crate::scheduler::Instant;
use crate::{
    Bounds, HitboxId, Modifiers, MouseButton, MouseDownEvent, MouseExitEvent, MouseMoveEvent,
    MousePressureEvent, MouseUpEvent, Pixels, PlatformInput, Point, ScrollDelta, ScrollWheelEvent,
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::PinchEvent;

use super::{
    HitTestResult, PointerButtons, PointerEvent, PointerId, PointerKind, PointerPhase,
    PointerSignalEvent,
};
use smallvec::SmallVec;

/// The default `PointerId` used for the desktop mouse cursor.
///
/// On mouse-only platforms there is exactly one logical pointer (the
/// cursor), so a fixed `PointerId(0)` is reused across `Down`/`Up`
/// sequences. Touch and stylus contacts (when surfaced by the platform
/// layer in the future) will allocate their own ids via
/// [`WindowPointerState::next_pointer_id`].
pub(crate) const DESKTOP_MOUSE_POINTER: PointerId = PointerId(0);

/// Per-`Window` pointer-state cache consumed by
/// [`PointerSanitizer::convert`]. Internal — `pub(crate)` only.
#[derive(Default)]
pub(crate) struct WindowPointerState {
    /// Last known mouse position, used to compute `delta`.
    pub(crate) last_mouse_position: Point<Pixels>,
    /// Currently-held mouse button(s).
    pub(crate) buttons: PointerButtons,
    /// Current keyboard modifiers (mirrors `Window::modifiers`).
    pub(crate) modifiers: Modifiers,
    /// Last reported pressure for the desktop mouse pointer.
    pub(crate) last_pressure: f32,
    /// Allocator counter for non-mouse pointer IDs (touch / stylus).
    /// Mouse uses [`DESKTOP_MOUSE_POINTER`]. Currently unread —
    /// `Window::dispatch_event` only handles desktop-mouse paths
    /// today, so [`Self::allocate_pointer_id`] is API-ready but not
    /// yet called. T15 (paint-time recognizer registration) and the
    /// future touch / stylus integrations will populate this.
    #[allow(dead_code, reason = "T15 + touch/stylus integration consumer")]
    pub(crate) next_pointer_id: u64,
    /// Tracks whether the desktop mouse is currently in the `Down`
    /// state. Used by `PointerSanitizer` for orphan-cancel synthesis
    /// and duplicate-down rejection.
    pub(crate) mouse_down: bool,
    /// Most recent window content bounds. Updated by
    /// `Window::dispatch_event` so the sanitizer can clamp
    /// out-of-bounds positions (e.g. from Wayland decoration drag).
    pub(crate) window_bounds: Bounds<Pixels>,
    /// Last `Down` position (for duplicate-`Down` slop check).
    pub(crate) last_down_position: Point<Pixels>,
    /// Hitboxes that contained the pointer during the previous frame.
    /// Used by [`PointerSanitizer::diff_hover`] to synthesize
    /// `Enter`/`Exit` events on hover boundary transitions.
    pub(crate) prior_hover_hitboxes: SmallVec<[HitboxId; 8]>,
}

impl WindowPointerState {
    /// Allocate the next `PointerId` for a non-mouse contact.
    ///
    /// Skips `DESKTOP_MOUSE_POINTER` (`PointerId(0)`) so the
    /// invariant "non-zero IDs are always non-mouse" holds.
    /// Unused in the active dispatch path; consumed by T15 (paint-time
    /// recognizer registration) and by the future touch / stylus
    /// integrations. Sealed behind `pub(crate)` so external callers
    /// cannot mint invalid pointer IDs.
    #[allow(dead_code, reason = "T15 + touch/stylus integration consumer")]
    pub(crate) fn allocate_pointer_id(&mut self) -> PointerId {
        self.next_pointer_id = self.next_pointer_id.saturating_add(1).max(1);
        PointerId(self.next_pointer_id)
    }
}

/// `PointerSanitizer` runs between `PlatformInput` translation and
/// the existing `dispatch_mouse_event` chain.
///
/// Implements:
///
/// 1. **Orphan-`Cancel` synthesis.** A new `Down` while the previous
///    `Down` had no matching `Up` (focus loss, modal switch) emits
///    a synthetic `Cancel` first, then forwards the new `Down`.
/// 2. **Duplicate-`Down` rejection.** A `Down` for an already-down
///    pointer at the same position (within the duplicate-slop) is
///    silently dropped.
/// 3. **Position clamping.** Positions outside `state.window_bounds`
///    (which can arrive on Wayland during decoration drag) are
///    clamped to the window rectangle before downstream consumption.
/// 4. **Hover diff** (via [`Self::diff_hover`]). Compares the current
///    hit-test result against `state.prior_hover_hitboxes` and emits
///    `Enter`/`Exit` events for boundary transitions during pure
///    hover (no buttons pressed). The bare `Hover` event passes
///    through unchanged.
///
/// All synthesized events are logged at `log::trace!` with the kv
/// schema documented in the design doc § "Logging" (`pointer_id`,
/// `phase`, `synthesized`, `reason`); orphan-`Cancel` synthesis
/// additionally logs at `log::warn!`.
#[derive(Default)]
pub(crate) struct PointerSanitizer;

/// The duplicate-`Down` slop in pixels. A `Down` arriving while the
/// pointer is already in the `Down` state, with `|new - last| <
/// DUPLICATE_DOWN_SLOP_PX`, is silently dropped (matches Flutter's
/// `_PointerEventConverter` behavior on jittery hardware).
const DUPLICATE_DOWN_SLOP_PX: f32 = 1.0;

impl PointerSanitizer {
    /// Translate one `PlatformInput` into zero, one, or more
    /// [`PointerEvent`]s. May synthesize a leading `Cancel` for
    /// orphan-`Down` cases.
    ///
    /// Returns an empty `SmallVec` for inputs that are not pointer
    /// events (keyboard, modifiers-changed, file-drop) and for
    /// rejected duplicate-`Down`s. Returns a single-element `SmallVec`
    /// for normal mouse / touch / stylus inputs and a two-element
    /// `SmallVec` for the orphan-`Cancel` synthesis case.
    pub(crate) fn convert(
        &mut self,
        input: &PlatformInput,
        state: &mut WindowPointerState,
    ) -> SmallVec<[PointerEvent; 2]> {
        let mut out: SmallVec<[PointerEvent; 2]> = SmallVec::new();

        // Duplicate-Down rejection.
        if let PlatformInput::MouseDown(e) = input
            && state.mouse_down
            && distance(e.position, state.last_down_position) < DUPLICATE_DOWN_SLOP_PX
        {
            log::trace!(
                target: "flui::gesture",
                "duplicate_down rejected: pointer_id={} reason=duplicate-within-slop",
                DESKTOP_MOUSE_POINTER.0,
            );
            return out;
        }

        // Orphan-Down → synthesize Cancel for prior pointer.
        if let PlatformInput::MouseDown(e) = input
            && state.mouse_down
        {
            log::warn!(
                target: "flui::gesture",
                "orphan_down: synthesizing Cancel for prior pointer pointer_id={} new_pos=({},{})",
                DESKTOP_MOUSE_POINTER.0,
                e.position.x.0,
                e.position.y.0,
            );
            out.push(PointerEvent {
                pointer_id: DESKTOP_MOUSE_POINTER,
                kind: PointerKind::Mouse,
                phase: PointerPhase::Cancel,
                position: state.last_mouse_position,
                delta: Point::default(),
                buttons: state.buttons,
                modifiers: state.modifiers,
                timestamp: Instant::now(),
                pressure: state.last_pressure,
                tilt: 0.0,
                orientation: 0.0,
            });
            // Reset the pointer-down state so the subsequent translate
            // call does not treat the new Down as a continuation.
            state.mouse_down = false;
            state.buttons = PointerButtons::default();
        }

        if let Some(mut event) = translate_one(input, state) {
            // Position clamping. Wayland decoration drag can deliver
            // positions outside the content bounds; clamp before
            // recognizers see them.
            event.position = clamp_to_bounds(event.position, &state.window_bounds);

            // Track last_down_position for the duplicate-Down check.
            if matches!(event.phase, PointerPhase::Down) {
                state.last_down_position = event.position;
            }
            out.push(event);
        }
        out
    }

    /// Translate one `PlatformInput` into a [`PointerSignalEvent`] if
    /// it is a non-competitive signal (scroll, magnify); returns
    /// `None` otherwise.
    pub(crate) fn convert_signal(
        &mut self,
        input: &PlatformInput,
        state: &mut WindowPointerState,
    ) -> Option<PointerSignalEvent> {
        translate_signal(input, state)
    }

    /// Synthesize `Enter`/`Exit` events for the pure-hover case.
    /// Compares `current_hit_test` against `state.prior_hover_hitboxes`
    /// and emits one `Exit` per hitbox that left the set, one `Enter`
    /// per hitbox that joined.
    ///
    /// Called by the dispatcher AFTER hit-test computation, BEFORE
    /// arena dispatch (T15). The supplied `template` is used to copy
    /// `pointer_id` / `kind` / `modifiers` / `timestamp` onto each
    /// synthesized event.
    ///
    /// Updates `state.prior_hover_hitboxes` on its way out so the
    /// next call diffs against the new set.
    pub(crate) fn diff_hover(
        &mut self,
        template: &PointerEvent,
        current_hit_test: &HitTestResult,
        state: &mut WindowPointerState,
    ) -> SmallVec<[PointerEvent; 4]> {
        let mut out: SmallVec<[PointerEvent; 4]> = SmallVec::new();
        if !matches!(template.phase, PointerPhase::Hover) {
            return out;
        }

        let current: SmallVec<[HitboxId; 8]> =
            current_hit_test.iter().map(|e| e.hitbox_id).collect();

        // Exits: in prior, not in current.
        for &id in state.prior_hover_hitboxes.iter() {
            if !current.contains(&id) {
                out.push(PointerEvent {
                    pointer_id: template.pointer_id,
                    kind: template.kind,
                    phase: PointerPhase::Exit,
                    position: template.position,
                    delta: template.delta,
                    buttons: template.buttons,
                    modifiers: template.modifiers,
                    timestamp: template.timestamp,
                    pressure: template.pressure,
                    tilt: template.tilt,
                    orientation: template.orientation,
                });
            }
        }

        // Enters: in current, not in prior.
        for &id in current.iter() {
            if !state.prior_hover_hitboxes.contains(&id) {
                out.push(PointerEvent {
                    pointer_id: template.pointer_id,
                    kind: template.kind,
                    phase: PointerPhase::Enter,
                    position: template.position,
                    delta: template.delta,
                    buttons: template.buttons,
                    modifiers: template.modifiers,
                    timestamp: template.timestamp,
                    pressure: template.pressure,
                    tilt: template.tilt,
                    orientation: template.orientation,
                });
            }
        }

        state.prior_hover_hitboxes = current;
        out
    }
}

#[inline]
fn distance(a: Point<Pixels>, b: Point<Pixels>) -> f32 {
    let dx = a.x.0 - b.x.0;
    let dy = a.y.0 - b.y.0;
    (dx * dx + dy * dy).sqrt()
}

#[inline]
fn clamp_to_bounds(p: Point<Pixels>, bounds: &Bounds<Pixels>) -> Point<Pixels> {
    // If bounds are zero-sized (uninitialized state), pass through.
    if bounds.size.width.0 <= 0.0 || bounds.size.height.0 <= 0.0 {
        return p;
    }
    let min_x = bounds.origin.x;
    let max_x = Pixels(bounds.origin.x.0 + bounds.size.width.0);
    let min_y = bounds.origin.y;
    let max_y = Pixels(bounds.origin.y.0 + bounds.size.height.0);
    Point::new(
        Pixels(p.x.0.clamp(min_x.0, max_x.0)),
        Pixels(p.y.0.clamp(min_y.0, max_y.0)),
    )
}

/// Stateful translation from a single `PlatformInput` to a single
/// `PointerEvent`. Returns `None` for non-pointer inputs and for
/// signal-class inputs (scroll, pinch — see [`translate_signal`]).
fn translate_one(input: &PlatformInput, state: &mut WindowPointerState) -> Option<PointerEvent> {
    match input {
        PlatformInput::MouseDown(e) => Some(translate_mouse_down(e, state)),
        PlatformInput::MouseUp(e) => Some(translate_mouse_up(e, state)),
        PlatformInput::MouseMove(e) => Some(translate_mouse_move(e, state)),
        PlatformInput::MousePressure(e) => Some(translate_mouse_pressure(e, state)),
        PlatformInput::MouseExited(e) => Some(translate_mouse_exited(e, state)),
        // Signals are translated separately via translate_signal.
        PlatformInput::ScrollWheel(_) => None,
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        PlatformInput::Pinch(_) => None,
        // Non-pointer inputs.
        PlatformInput::KeyDown(_)
        | PlatformInput::KeyUp(_)
        | PlatformInput::ModifiersChanged(_)
        | PlatformInput::FileDrop(_) => None,
    }
}

/// Stateful translation to a `PointerSignalEvent`.
fn translate_signal(
    input: &PlatformInput,
    state: &mut WindowPointerState,
) -> Option<PointerSignalEvent> {
    match input {
        PlatformInput::ScrollWheel(e) => Some(translate_scroll(e, state)),
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        PlatformInput::Pinch(e) => Some(translate_pinch(e, state)),
        _ => None,
    }
}

fn translate_mouse_down(e: &MouseDownEvent, state: &mut WindowPointerState) -> PointerEvent {
    let buttons = mouse_button_to_pointer_buttons(e.button);
    let delta = subtract(e.position, state.last_mouse_position);
    state.last_mouse_position = e.position;
    state.modifiers = e.modifiers;
    state.buttons = state.buttons.union(buttons);
    state.mouse_down = true;
    state.last_pressure = 1.0;
    PointerEvent {
        pointer_id: DESKTOP_MOUSE_POINTER,
        kind: PointerKind::Mouse,
        phase: PointerPhase::Down,
        position: e.position,
        delta,
        buttons: state.buttons,
        modifiers: e.modifiers,
        timestamp: Instant::now(),
        pressure: state.last_pressure,
        tilt: 0.0,
        orientation: 0.0,
    }
}

fn translate_mouse_up(e: &MouseUpEvent, state: &mut WindowPointerState) -> PointerEvent {
    let released = mouse_button_to_pointer_buttons(e.button);
    let delta = subtract(e.position, state.last_mouse_position);
    state.last_mouse_position = e.position;
    state.modifiers = e.modifiers;
    // Clear the released bit but leave any other held buttons in
    // place (for chorded mouse interactions).
    state.buttons = PointerButtons(state.buttons.0 & !released.0);
    if state.buttons.is_empty() {
        state.mouse_down = false;
    }
    state.last_pressure = 0.0;
    PointerEvent {
        pointer_id: DESKTOP_MOUSE_POINTER,
        kind: PointerKind::Mouse,
        phase: PointerPhase::Up,
        position: e.position,
        delta,
        buttons: state.buttons,
        modifiers: e.modifiers,
        timestamp: Instant::now(),
        pressure: state.last_pressure,
        tilt: 0.0,
        orientation: 0.0,
    }
}

fn translate_mouse_move(e: &MouseMoveEvent, state: &mut WindowPointerState) -> PointerEvent {
    let delta = subtract(e.position, state.last_mouse_position);
    state.last_mouse_position = e.position;
    state.modifiers = e.modifiers;
    let phase = if state.mouse_down || e.pressed_button.is_some() {
        PointerPhase::Move
    } else {
        PointerPhase::Hover
    };
    PointerEvent {
        pointer_id: DESKTOP_MOUSE_POINTER,
        kind: PointerKind::Mouse,
        phase,
        position: e.position,
        delta,
        buttons: state.buttons,
        modifiers: e.modifiers,
        timestamp: Instant::now(),
        pressure: if matches!(phase, PointerPhase::Move) {
            1.0
        } else {
            0.0
        },
        tilt: 0.0,
        orientation: 0.0,
    }
}

fn translate_mouse_pressure(
    e: &MousePressureEvent,
    state: &mut WindowPointerState,
) -> PointerEvent {
    let delta = subtract(e.position, state.last_mouse_position);
    state.last_mouse_position = e.position;
    state.modifiers = e.modifiers;
    state.last_pressure = e.pressure;
    PointerEvent {
        pointer_id: DESKTOP_MOUSE_POINTER,
        kind: PointerKind::Mouse,
        phase: PointerPhase::Move,
        position: e.position,
        delta,
        buttons: state.buttons,
        modifiers: e.modifiers,
        timestamp: Instant::now(),
        pressure: e.pressure,
        tilt: 0.0,
        orientation: 0.0,
    }
}

fn translate_mouse_exited(e: &MouseExitEvent, state: &mut WindowPointerState) -> PointerEvent {
    let delta = subtract(e.position, state.last_mouse_position);
    state.last_mouse_position = e.position;
    state.modifiers = e.modifiers;
    PointerEvent {
        pointer_id: DESKTOP_MOUSE_POINTER,
        kind: PointerKind::Mouse,
        phase: PointerPhase::Exit,
        position: e.position,
        delta,
        buttons: state.buttons,
        modifiers: e.modifiers,
        timestamp: Instant::now(),
        pressure: 0.0,
        tilt: 0.0,
        orientation: 0.0,
    }
}

fn translate_scroll(e: &ScrollWheelEvent, state: &mut WindowPointerState) -> PointerSignalEvent {
    state.last_mouse_position = e.position;
    state.modifiers = e.modifiers;
    let delta = match e.delta {
        ScrollDelta::Pixels(p) => p,
        // Convert line-based deltas to a 1.0px-per-line approximation;
        // recognizers that need real pixel-per-line conversion can
        // call `ScrollDelta::pixel_delta(line_height)` themselves on
        // the original platform event. Listeners on
        // `ScrollWheelEvent` continue to fire unchanged in T6.
        ScrollDelta::Lines(p) => Point::new(Pixels(p.x), Pixels(p.y)),
    };
    PointerSignalEvent::Scroll {
        pointer_id: DESKTOP_MOUSE_POINTER,
        kind: PointerKind::Mouse,
        position: e.position,
        delta,
        modifiers: e.modifiers,
        timestamp: Instant::now(),
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn translate_pinch(e: &PinchEvent, state: &mut WindowPointerState) -> PointerSignalEvent {
    state.last_mouse_position = e.position;
    state.modifiers = e.modifiers;
    // PinchEvent.delta is additive (e.g. 0.1 == +10%); convert to a
    // multiplicative scale by `1.0 + delta`. Rotation is always 0.0
    // on current desktop platforms — see the design doc § "Explicit
    // gaps".
    PointerSignalEvent::Magnify {
        pointer_id: DESKTOP_MOUSE_POINTER,
        kind: PointerKind::Mouse,
        position: e.position,
        scale_delta: 1.0 + e.delta,
        rotation_rad: 0.0,
        modifiers: e.modifiers,
        timestamp: Instant::now(),
    }
}

fn mouse_button_to_pointer_buttons(button: MouseButton) -> PointerButtons {
    match button {
        MouseButton::Left => PointerButtons::PRIMARY,
        MouseButton::Right => PointerButtons::SECONDARY,
        MouseButton::Middle => PointerButtons::TERTIARY,
        // Navigation buttons map to no PointerButtons bit; they are
        // surfaced via the existing raw `MouseDownEvent.button` for
        // listeners that care.
        MouseButton::Navigate(_) => PointerButtons(0),
    }
}

#[inline]
fn subtract(a: Point<Pixels>, b: Point<Pixels>) -> Point<Pixels> {
    Point::new(a.x - b.x, a.y - b.y)
}
