use flui_core::ElementId;

/// Headless radio button primitive.
///
/// **Status**: Stub — not yet implemented.
pub struct RadioBase {
    #[allow(dead_code)]
    id: ElementId,
}

impl RadioBase {
    /// Create a new radio button.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { id: id.into() }
    }
}
