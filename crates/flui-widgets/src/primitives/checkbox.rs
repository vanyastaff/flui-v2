use flui_core::ElementId;

/// Headless checkbox primitive.
///
/// **Status**: Stub — not yet implemented.
pub struct CheckboxBase {
    #[allow(dead_code)]
    id: ElementId,
}

impl CheckboxBase {
    /// Create a new checkbox.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { id: id.into() }
    }
}
