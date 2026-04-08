use flui_core::ElementId;

/// Headless switch/toggle primitive.
///
/// **Status**: Stub — not yet implemented.
pub struct SwitchBase {
    #[allow(dead_code)]
    id: ElementId,
}

impl SwitchBase {
    /// Create a new switch.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { id: id.into() }
    }
}
