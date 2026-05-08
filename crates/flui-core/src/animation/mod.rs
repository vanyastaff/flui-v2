// crates/flui-core/src/animation/mod.rs
//
// `flui_core::animation` is the Flutter-parity animation surface.
// S21 grows it incrementally per `.ai-factory/plans/animation-flutter-parity.md`.
// Layout is intentionally flat — every concern is a sibling file, no
// subdirectories.

mod animated;
mod animation;
mod behavior;
mod combinator;
mod controller;
mod curve;
mod curved_animation;
mod lerp;
mod listeners;
mod simulation;
mod status;
mod ticker;
mod tween;

pub use animated::animated;
pub use animation::{Animation, ListenerCallback, ListenerId, StatusListenerCallback};
pub use behavior::{AnimationBehavior, AnimationStyle};
pub use combinator::{
    AlwaysStoppedAnimation, CompoundAnimation, ProxyAnimation, ReverseAnimation,
    TrainHoppingAnimation, animation_max, animation_mean, animation_min,
};
pub use controller::AnimationController;
pub use curve::{
    BounceIn, BounceInOut, BounceOut, Cubic, Curve, Curves, CustomCurve, Decelerate, EaseIn,
    EaseInCubic, EaseInOut, EaseInOutCubic, EaseOut, EaseOutCubic, ElasticIn, ElasticInOut,
    ElasticOut, FlippedCurve, Interval, Linear, Reversed, SawTooth, Split, Spring, Threshold,
};
pub use curved_animation::CurvedAnimation;
pub use lerp::Lerp;
// S21 review-fix Tier 3: listener storage primitives are crate-internal.
// `LocalListeners` / `LocalStatusListeners` carry the `add()` / `remove()` /
// `notify()` surface that should NOT be reachable to third parties — they
// would let external code manipulate another animation's listener storage,
// bypassing the `Animation<T>` trait contract. The `LazyListenable` /
// `EagerListenable` hook traits are sealed via `crate::seal::Sealed` (see
// listeners.rs) so any external impl is rejected at compile time; they
// remain `pub(crate)` here for ergonomic use within `flui-core` itself.
#[allow(unused_imports)]
// re-exports for crate-internal ergonomic use; some impls land in later phases
pub(crate) use listeners::{EagerListenable, LazyListenable, LocalListeners, LocalStatusListeners};
pub use simulation::{
    BoundedFrictionSimulation, FrictionSimulation, GravitySimulation, Simulation,
    SpringDescription, SpringSimulation, Tolerance,
};
pub use status::AnimationStatus;
// `TickerProvider` is `pub(crate)` until the first concrete impl ships
// (S21 review-fix Tier 3). The other ticker types remain `pub` — `Ticker`
// is consumed externally via `AnimationController::attach()`.
#[allow(unused_imports)] // first impl lands with widget-layer integration
pub(crate) use ticker::TickerProvider;
pub use ticker::{Ticker, TickerCanceled, TickerFuture, TickerFutureState};
pub use tween::{
    Animatable, AnimatableExt, ChainedAnimatable, ColorTween, ConstantTween, CurveTween,
    FlippedTweenSequence, IntTween, RectTween, ReverseTween, SizeTween, StepTween, Tween,
    TweenSequence, TweenSequenceItem,
};
