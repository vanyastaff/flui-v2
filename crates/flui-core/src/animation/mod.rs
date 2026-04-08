mod animated;
mod controller;
mod curve;
mod lerp;
mod simulation;
mod tween;

pub use animated::animated;
pub use controller::{AnimationController, AnimationStatus};
pub use curve::Curve;
pub use lerp::Lerp;
pub use simulation::{
    FrictionSimulation, GravitySimulation, Simulation, SpringDescription, SpringSimulation,
    Tolerance,
};
pub use tween::Tween;
