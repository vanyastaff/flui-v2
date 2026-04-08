use flui_core::{App, IntoElement, RenderOnce, Styled, Window, div, px};
use flui_theme::ActiveTheme;

/// Material Design 3 Divider — a thin horizontal or vertical line.
#[derive(flui_core::IntoElement)]
pub struct Divider {
    vertical: bool,
}

impl Divider {
    pub fn horizontal() -> Self { Self { vertical: false } }
    pub fn vertical() -> Self { Self { vertical: true } }
}

impl RenderOnce for Divider {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let color = theme.color_scheme.outline_variant;
        let d = div().bg(color);
        if self.vertical {
            d.w(px(1.)).h_full()
        } else {
            d.h(px(1.)).w_full()
        }
    }
}
