//! The GPUI prelude is a collection of traits and types that are widely used
//! throughout the library. It is recommended to import this prelude into your
//! application to avoid having to import each trait individually.

pub use crate::{
    AppContext as _, BorrowAppContext, BuildElement, Context, Element, ElementBuildCx,
    ElementBuilder, GlobalKey, InteractiveElement, IntoElement, Key, LayoutCx, PaintCx,
    ParentElement, PrepaintCx, Refineable, Render, RenderOnce, StatefulInteractiveElement, Styled,
    StyledImage, ValueKey, VisualContext, build_element,
    local_util::FluentBuilder,
    reentrancy::{ReentryError, ReentryMode},
};
