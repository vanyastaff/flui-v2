use flui_core::{
    InteractiveElement,
    AnyElement, App, ClickEvent, ElementId, IntoElement, ParentElement, RenderOnce, Styled,
    Window, div, px, prelude::FluentBuilder,
};
use flui_theme::ActiveTheme;
use flui_widgets::ButtonBase;

/// Material Design 3 Icon Button.
///
/// Square button for icon-only actions.
#[derive(flui_core::IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: AnyElement,
    disabled: bool,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, icon: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            icon: icon.into_any_element(),
            disabled: false,
            on_click: None,
        }
    }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f)); self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let disabled = self.disabled;

        let mut base = ButtonBase::new(self.id).disabled(self.disabled).child(self.icon);
        if let Some(on_click) = self.on_click { base = base.on_click(on_click); }

        base.style(move |_state, child| {
            div()
                .flex().items_center().justify_center()
                .size(px(40.))
                .rounded(theme.shape.full)
                .when(disabled, |d| d.text_color(theme.color_scheme.on_surface.opacity(0.38)))
                .when(!disabled, |d| d.text_color(theme.color_scheme.on_surface_variant).hover(|s| s.bg(theme.color_scheme.on_surface.opacity(0.08))))
                .child(child).into_any_element()
        })
    }
}
