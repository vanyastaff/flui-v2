use flui_core::ElementId;

/// Headless slider primitive.
///
/// **Status**: Stub — not yet implemented.
pub struct SliderBase {
    #[allow(dead_code)]
    id: ElementId,
}

impl SliderBase {
    /// Create a new slider.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { id: id.into() }
    }
}
