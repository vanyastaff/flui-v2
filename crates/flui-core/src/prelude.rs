//! The GPUI prelude is a collection of traits and types that are widely used
//! throughout the library. It is recommended to import this prelude into your
//! application to avoid having to import each trait individually.

pub use crate::{
    AppContext as _,
    BorrowAppContext,
    BuildElement,
    Context,
    Element,
    ElementBuildCx,
    ElementBuilder,
    GlobalKey,
    InteractiveElement,
    IntoElement,
    Key,
    LayoutCx,
    PaintCx,
    ParentElement,
    PrepaintCx,
    Refineable,
    Render,
    RenderOnce,
    StatefulInteractiveElement,
    Styled,
    StyledImage,
    ValueKey,
    VisualContext,
    build_element,
    // K04 Task 28: contract surface in the prelude. The narrow shape mirrors
    // Flutter `SchedulerPhase` / `FrameTiming` — observable phase + cheap
    // telemetry — without exposing implementation types like `FrameClock`,
    // `DeferPlacement`, `TickTarget`, or `FrameProfileDetailed`, which stay
    // `pub` but explicit-import.
    frame::{FramePhase, profile::FrameProfile},
    local_util::FluentBuilder,
    reentrancy::{ReentryError, ReentryMode},
};
