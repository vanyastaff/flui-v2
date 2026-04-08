use flui_core::ElementId;

/// Headless select/dropdown primitive.
///
/// **Status**: Stub — not yet implemented.
pub struct SelectBase {
    #[allow(dead_code)]
    id: ElementId,
}

impl SelectBase {
    /// Create a new select.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { id: id.into() }
    }
}
