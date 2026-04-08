use flui_core::{
    AnyElement, App, ElementId, FocusHandle, InteractiveElement, IntoElement, MouseButton,
    ParentElement, RenderOnce, Styled, Window, div,
};

/// Headless dialog/modal primitive.
///
/// When `visible` is true, renders a full-screen overlay with optional
/// backdrop click-to-dismiss.
///
/// Design systems wrap this to add visual styling (background dim, surface card, etc).
#[derive(flui_core::IntoElement)]
pub struct DialogBase {
    id: ElementId,
    visible: bool,
    on_dismiss: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    close_on_backdrop: bool,
    focus_handle: Option<FocusHandle>,
    content: Option<AnyElement>,
}

impl DialogBase {
    /// Create a new dialog.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            visible: false,
            on_dismiss: None,
            close_on_backdrop: true,
            focus_handle: None,
            content: None,
        }
    }

    /// Set whether the dialog is visible.
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Set the dismiss callback (called on backdrop click).
    pub fn on_dismiss(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Box::new(f));
        self
    }

    /// Whether clicking the backdrop dismisses the dialog (default: true).
    pub fn close_on_backdrop(mut self, v: bool) -> Self {
        self.close_on_backdrop = v;
        self
    }

    /// Attach a focus handle for keyboard event capture.
    pub fn focus_handle(mut self, handle: FocusHandle) -> Self {
        self.focus_handle = Some(handle);
        self
    }

    /// Set the dialog content.
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }
}

impl RenderOnce for DialogBase {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        if !self.visible {
            return div().into_any_element();
        }

        let mut overlay = div()
            .id(self.id)
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center();

        if let Some(ref fh) = self.focus_handle {
            overlay = overlay.track_focus(fh);
        }

        // Backdrop with click-to-dismiss
        if self.close_on_backdrop {
            if let Some(on_dismiss) = self.on_dismiss {
                let backdrop = div()
                    .absolute()
                    .inset_0()
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        on_dismiss(window, cx);
                    });
                overlay = overlay.child(backdrop);
            }
        }

        // Content (centered over backdrop)
        if let Some(content) = self.content {
            overlay = overlay.child(div().relative().child(content));
        }

        overlay.into_any_element()
    }
}
