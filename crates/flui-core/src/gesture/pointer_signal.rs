//! `PointerSignalEvent` — non-competitive pointer signals (scroll,
//! scale, scroll-inertia-cancel) that bypass the gesture arena entirely.
//!
//! See the design doc at
//! `docs/superpowers/specs/2026-05-06-S07-gesture-arena-design.md`
//! § "Design — PointerSignalEvent".

use super::{PointerId, PointerKind};
use crate::scheduler::Instant;
use crate::{Modifiers, Pixels, Point, WindowId};

/// Origin information shared by pointer-signal variants.
///
/// The platform layer does not expose every native identifier yet, so
/// fields are optional instead of using sentinel values. This keeps the
/// Rust API explicit while still leaving room for Flutter-compatible
/// `viewId`, `device`, and native event data.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct PointerSignalSource {
    /// Window that received the signal.
    pub window_id: Option<WindowId>,
    /// Stable physical pointing-device id, reused across interactions
    /// when the platform exposes one.
    pub device_id: Option<u64>,
    /// Opaque native event id for diagnostics or future platform
    /// default-response bridges.
    pub platform_event_id: Option<u64>,
}

/// Common fields shared by every pointer signal.
///
/// Construction goes through the platform-input translator. Downstream
/// code should treat this as the signal equivalent of the shared fields
/// on [`super::PointerEvent`].
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct PointerSignalData {
    /// Pointer that produced this signal.
    pub pointer_id: PointerId,
    /// Device kind that produced this signal.
    pub kind: PointerKind,
    /// Position in window-local logical pixels.
    pub position: Point<Pixels>,
    /// Currently-held keyboard modifiers.
    pub modifiers: Modifiers,
    /// Time at which this signal was delivered into the dispatcher.
    pub timestamp: Instant,
    /// Platform/window origin ids associated with this signal.
    pub source: PointerSignalSource,
}

/// A non-competitive signal from a pointer device.
///
/// Recognizers do not compete on signals — there is no winner of a
/// scroll-wheel tick or scale tick. A signal resolves to at most one
/// interested listener outside the gesture arena.
///
/// `#[non_exhaustive]` to admit future signals (e.g. smart-zoom,
/// force-press) without breaking changes.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum PointerSignalEvent {
    /// A scroll-wheel / two-finger-pan tick.
    Scroll {
        /// Shared pointer-signal fields.
        data: PointerSignalData,
        /// Scroll delta in window-local logical pixels.
        scroll_delta: Point<Pixels>,
    },
    /// A discrete scale / zoom tick. `scale` is multiplicative:
    /// `1.0` means no zoom, `> 1.0` zooms in, `< 1.0` zooms out.
    Scale {
        /// Shared pointer-signal fields.
        data: PointerSignalData,
        /// Multiplicative scale factor for this signal.
        scale: f32,
    },
    /// A platform signal that cancels in-flight scroll inertia.
    ScrollInertiaCancel {
        /// Shared pointer-signal fields.
        data: PointerSignalData,
    },
}

impl PointerSignalEvent {
    /// Shared fields for this signal.
    pub(crate) fn data(&self) -> &PointerSignalData {
        match self {
            Self::Scroll { data, .. }
            | Self::Scale { data, .. }
            | Self::ScrollInertiaCancel { data } => data,
        }
    }

    fn data_mut(&mut self) -> &mut PointerSignalData {
        match self {
            Self::Scroll { data, .. }
            | Self::Scale { data, .. }
            | Self::ScrollInertiaCancel { data } => data,
        }
    }

    /// Pointer that produced this signal.
    pub(crate) fn pointer_id(&self) -> PointerId {
        self.data().pointer_id
    }

    /// Coarse signal family, used by the resolver and debug logs.
    pub(crate) fn signal_kind(&self) -> PointerSignalKind {
        match self {
            Self::Scroll { .. } => PointerSignalKind::Scroll,
            Self::Scale { .. } => PointerSignalKind::Scale,
            Self::ScrollInertiaCancel { .. } => PointerSignalKind::ScrollInertiaCancel,
        }
    }

    /// Origin ids associated with this signal.
    pub(crate) fn source(&self) -> PointerSignalSource {
        self.data().source
    }

    /// Assign the receiving window once `Window::dispatch_event` knows
    /// which window accepted the platform input.
    pub(crate) fn set_window_id(&mut self, window_id: WindowId) {
        self.data_mut().source.window_id = Some(window_id);
    }
}

/// Coarse pointer-signal family for routing and diagnostics.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum PointerSignalKind {
    Scroll,
    Scale,
    ScrollInertiaCancel,
}

/// Opaque identity for a listener that registered interest in a
/// pointer signal during dispatch.
///
/// The current live path routes signals through legacy mouse listeners,
/// but keeping the resolver in terms of listener ids gives the future
/// typed `on_pointer_signal` API a stable, testable shape without
/// storing `Window`/`App` callback closures in the gesture module.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct PointerSignalListenerId(pub(crate) usize);

/// Result of resolving one pointer signal.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum PointerSignalRoute {
    /// Temporary compatibility marker for the already-existing
    /// `on_scroll_wheel` / `on_pinch` listener chain.
    MouseListeners,
    /// A typed pointer-signal listener registered first and won.
    Listener(PointerSignalListenerId),
    /// No typed listener registered for this signal.
    Unhandled,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct PointerSignalResolution {
    pub(crate) pointer_id: PointerId,
    pub(crate) signal_kind: PointerSignalKind,
    pub(crate) source: PointerSignalSource,
    pub(crate) route: PointerSignalRoute,
}

/// Per-window mediator for non-competitive pointer signals.
///
/// Scroll, scale, and scroll-inertia-cancel signals are immediate:
/// they never compete in the gesture arena, but at most one interested
/// listener should handle each signal. During a dispatch pass listeners
/// register in hit-test order; resolving the event picks the first
/// registration.
#[derive(Default)]
pub(crate) struct PointerSignalResolver {
    first_listener: Option<PointerSignalListenerId>,
    last_resolution: Option<PointerSignalResolution>,
}

impl PointerSignalResolver {
    /// Start collecting registrations for one signal event.
    #[allow(
        dead_code,
        reason = "typed pointer-signal listeners will call this before hit-test registration"
    )]
    pub(crate) fn begin_event(&mut self) {
        self.first_listener = None;
    }

    /// Register interest in handling `event`.
    ///
    /// Returns `true` for the first registered listener and `false` for
    /// later listeners. Dispatch code can use the return value for
    /// diagnostics, but only [`Self::resolve`] decides the final winner.
    #[allow(
        dead_code,
        reason = "typed pointer-signal listeners are not wired to Interactivity yet"
    )]
    pub(crate) fn register_listener(
        &mut self,
        event: &PointerSignalEvent,
        listener_id: PointerSignalListenerId,
    ) -> bool {
        let accepted = self.first_listener.is_none();
        if accepted {
            self.first_listener = Some(listener_id);
        }
        log::trace!(
            target: "flui::gesture::pointer_signal",
            phase = "register",
            pointer_id = event.pointer_id().raw(),
            signal_kind = format!("{:?}", event.signal_kind()),
            window_id = format!("{:?}", event.source().window_id.map(|id| id.as_u64())),
            device_id = format!("{:?}", event.source().device_id),
            platform_event_id = format!("{:?}", event.source().platform_event_id),
            listener_id = listener_id.0,
            accepted = accepted;
            "pointer signal listener registration"
        );
        accepted
    }

    /// Resolve the current signal to the first registered typed
    /// listener, if any.
    #[allow(
        dead_code,
        reason = "typed pointer-signal listeners are not wired to Interactivity yet"
    )]
    pub(crate) fn resolve(&mut self, event: &PointerSignalEvent) -> PointerSignalRoute {
        let route = match self.first_listener.take() {
            Some(listener_id) => PointerSignalRoute::Listener(listener_id),
            None => PointerSignalRoute::Unhandled,
        };
        self.record_resolution(event, route);
        route
    }

    /// Record the compatibility route used by the legacy mouse-event
    /// listener chain.
    ///
    /// The original platform event is still delivered later by
    /// `Window::dispatch_mouse_event`; this method only records resolver
    /// state. The future typed signal listener dispatch can replace this
    /// call site with `begin_event` + `register` + `resolve`.
    pub(crate) fn resolve_to_mouse_listeners(
        &mut self,
        event: &PointerSignalEvent,
    ) -> PointerSignalRoute {
        self.first_listener = None;
        let route = PointerSignalRoute::MouseListeners;
        self.record_resolution(event, route);
        route
    }

    fn record_resolution(&mut self, event: &PointerSignalEvent, route: PointerSignalRoute) {
        let resolution = PointerSignalResolution {
            pointer_id: event.pointer_id(),
            signal_kind: event.signal_kind(),
            source: event.source(),
            route,
        };
        log::trace!(
            target: "flui::gesture::pointer_signal",
            phase = "resolve",
            pointer_id = resolution.pointer_id.raw(),
            signal_kind = format!("{:?}", resolution.signal_kind),
            window_id = format!("{:?}", resolution.source.window_id.map(|id| id.as_u64())),
            device_id = format!("{:?}", resolution.source.device_id),
            platform_event_id = format!("{:?}", resolution.source.platform_event_id),
            route = format!("{:?}", resolution.route);
            "pointer signal resolved outside gesture arena"
        );
        self.last_resolution = Some(resolution);
    }

    #[cfg(test)]
    pub(crate) fn last_resolution(&self) -> Option<PointerSignalResolution> {
        self.last_resolution
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scroll_signal(pointer_id: PointerId) -> PointerSignalEvent {
        PointerSignalEvent::Scroll {
            data: signal_data(pointer_id, PointerSignalSource::default()),
            scroll_delta: Point::new(Pixels(0.0), Pixels(12.0)),
        }
    }

    fn scale_signal(pointer_id: PointerId) -> PointerSignalEvent {
        PointerSignalEvent::Scale {
            data: signal_data(
                pointer_id,
                PointerSignalSource {
                    window_id: Some(WindowId::from(9)),
                    device_id: Some(3),
                    platform_event_id: Some(99),
                },
            ),
            scale: 1.1,
        }
    }

    fn scroll_inertia_cancel_signal(pointer_id: PointerId) -> PointerSignalEvent {
        PointerSignalEvent::ScrollInertiaCancel {
            data: signal_data(pointer_id, PointerSignalSource::default()),
        }
    }

    fn signal_data(pointer_id: PointerId, source: PointerSignalSource) -> PointerSignalData {
        PointerSignalData {
            pointer_id,
            kind: PointerKind::Mouse,
            position: Point::new(Pixels(30.0), Pixels(40.0)),
            modifiers: Modifiers::default(),
            timestamp: Instant::now(),
            source,
        }
    }

    #[test]
    fn resolver_picks_first_listener() {
        let event = scroll_signal(PointerId(7));
        let mut resolver = PointerSignalResolver::default();

        resolver.begin_event();
        assert!(resolver.register_listener(&event, PointerSignalListenerId(11)));
        assert!(!resolver.register_listener(&event, PointerSignalListenerId(22)));

        let route = resolver.resolve(&event);
        assert_eq!(
            route,
            PointerSignalRoute::Listener(PointerSignalListenerId(11)),
            "pointer signals must resolve to the deepest/first registered listener"
        );
        assert_eq!(
            resolver.last_resolution(),
            Some(PointerSignalResolution {
                pointer_id: PointerId(7),
                signal_kind: PointerSignalKind::Scroll,
                source: PointerSignalSource::default(),
                route,
            })
        );
    }

    #[test]
    fn resolver_clears_registrations_between_events() {
        let first = scroll_signal(PointerId(1));
        let second = scale_signal(PointerId(2));
        let mut resolver = PointerSignalResolver::default();

        resolver.begin_event();
        assert!(resolver.register_listener(&first, PointerSignalListenerId(1)));
        assert_eq!(
            resolver.resolve(&first),
            PointerSignalRoute::Listener(PointerSignalListenerId(1))
        );

        resolver.begin_event();
        assert_eq!(
            resolver.resolve(&second),
            PointerSignalRoute::Unhandled,
            "listener registration must not leak into the next signal event"
        );
        assert_eq!(
            resolver.last_resolution(),
            Some(PointerSignalResolution {
                pointer_id: PointerId(2),
                signal_kind: PointerSignalKind::Scale,
                source: PointerSignalSource {
                    window_id: Some(WindowId::from(9)),
                    device_id: Some(3),
                    platform_event_id: Some(99),
                },
                route: PointerSignalRoute::Unhandled,
            })
        );
    }

    #[test]
    fn legacy_resolution_records_signal_without_arena_participation() {
        let event = scroll_signal(PointerId(0));
        let mut resolver = PointerSignalResolver::default();

        let route = resolver.resolve_to_mouse_listeners(&event);

        assert_eq!(route, PointerSignalRoute::MouseListeners);
        assert_eq!(
            resolver.last_resolution(),
            Some(PointerSignalResolution {
                pointer_id: PointerId(0),
                signal_kind: PointerSignalKind::Scroll,
                source: PointerSignalSource::default(),
                route: PointerSignalRoute::MouseListeners,
            })
        );
    }

    #[test]
    fn set_window_id_preserves_device_and_platform_event_ids() {
        let mut event = scale_signal(PointerId(4));

        event.set_window_id(WindowId::from(42));

        assert_eq!(
            event.source(),
            PointerSignalSource {
                window_id: Some(WindowId::from(42)),
                device_id: Some(3),
                platform_event_id: Some(99),
            },
            "Window dispatch may fill window_id without erasing backend-provided ids"
        );
    }

    #[test]
    fn signal_kind_covers_all_pointer_signal_variants() {
        assert_eq!(
            scroll_signal(PointerId(1)).signal_kind(),
            PointerSignalKind::Scroll
        );
        assert_eq!(
            scale_signal(PointerId(2)).signal_kind(),
            PointerSignalKind::Scale
        );
        assert_eq!(
            scroll_inertia_cancel_signal(PointerId(3)).signal_kind(),
            PointerSignalKind::ScrollInertiaCancel
        );
    }
}
