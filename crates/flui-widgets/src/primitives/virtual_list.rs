use std::ops::Range;

use flui_core::{
    AnyElement, App, ElementId, IntoElement, RenderOnce, Styled, Window, uniform_list,
};

/// Headless virtual list — only renders visible items.
///
/// Wraps flui-core's `uniform_list` for lazy rendering of large collections.
///
/// # Example
///
/// ```ignore
/// VirtualListBase::new("items", 10_000, |range, _window, _cx| {
///     range.map(|i| div().child(format!("Item {i}"))).collect()
/// })
/// ```
#[derive(flui_core::IntoElement)]
pub struct VirtualListBase {
    id: ElementId,
    item_count: usize,
    render_items: Box<dyn Fn(Range<usize>, &mut Window, &mut App) -> Vec<AnyElement> + 'static>,
}

impl VirtualListBase {
    /// Create a virtual list.
    ///
    /// - `id`: Unique identifier for scroll state persistence.
    /// - `item_count`: Total number of items in the list.
    /// - `render_items`: Called with the visible range; must return elements for that range.
    pub fn new(
        id: impl Into<ElementId>,
        item_count: usize,
        render_items: impl Fn(Range<usize>, &mut Window, &mut App) -> Vec<AnyElement> + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            item_count,
            render_items: Box::new(render_items),
        }
    }
}

impl RenderOnce for VirtualListBase {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        uniform_list(self.id, self.item_count, move |range, window, cx| {
            (self.render_items)(range, window, cx)
        })
        .h_full()
    }
}
