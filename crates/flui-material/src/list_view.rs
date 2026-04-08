use std::ops::Range;

use flui_core::{App, ElementId, IntoElement, Window};
use flui_widgets::VirtualListBase;

/// Material Design 3 ListView.
///
/// Provides a `builder` constructor for efficiently rendering large lists.
pub struct ListView;

impl ListView {
    /// Create a virtual list that only renders visible items.
    ///
    /// # Example
    ///
    /// ```ignore
    /// ListView::builder("items", 10_000, |i, _window, _cx| {
    ///     div().child(format!("Item {i}"))
    /// })
    /// ```
    pub fn builder<R: IntoElement + 'static>(
        id: impl Into<ElementId>,
        item_count: usize,
        render_item: impl Fn(usize, &mut Window, &mut App) -> R + 'static,
    ) -> VirtualListBase {
        VirtualListBase::new(id, item_count, move |range: Range<usize>, window, cx| {
            range
                .map(|i| render_item(i, window, cx).into_any_element())
                .collect()
        })
    }
}
