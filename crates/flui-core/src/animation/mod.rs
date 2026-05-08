// crates/flui-core/src/animation/mod.rs
//
// `flui_core::animation` is the Flutter-parity animation surface.
// S21 grows it incrementally per `.ai-factory/plans/animation-flutter-parity.md`.
// Layout is intentionally flat — every concern is a sibling file, no
// subdirectories.

mod animated;
mod animation;
mod controller;
mod curve;
mod lerp;
mod listeners;
mod simulation;
mod status;
mod ticker;
mod tween;

pub use animated::animated;
pub use animation::{Animation, ListenerCallback, ListenerId, StatusListenerCallback};
pub use controller::AnimationController;
pub use curve::Curve;
pub use lerp::Lerp;
pub use listeners::{EagerListenable, LazyListenable, LocalListeners, LocalStatusListeners};
pub use simulation::{
    FrictionSimulation, GravitySimulation, Simulation, SpringDescription, SpringSimulation,
    Tolerance,
};
pub use status::AnimationStatus;
pub use ticker::{Ticker, TickerCanceled, TickerFuture, TickerFutureState, TickerProvider};
pub use tween::Tween;
