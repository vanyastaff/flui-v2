/// A snapshot of an interactive element's current state.
///
/// Passed to the `build()` closure of headless widgets so design systems
/// can branch on hover/press/focus/disabled without coupling to behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InteractionState {
    /// Whether the pointer is currently over this element.
    pub hovered: bool,
    /// Whether the element is currently being pressed (mouse down or Space/Enter held).
    pub pressed: bool,
    /// Whether the element currently has keyboard focus.
    pub focused: bool,
    /// Whether the element is disabled and non-interactive.
    pub disabled: bool,
}

impl InteractionState {
    /// Returns `true` if the element can receive interaction events.
    pub fn is_interactive(&self) -> bool {
        !self.disabled
    }
}
