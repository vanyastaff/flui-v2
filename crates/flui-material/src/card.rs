use flui_core::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window, div,
};
use flui_theme::ActiveTheme;
use smallvec::SmallVec;

/// Material Design 3 Card variant.
#[derive(Clone, Copy, Debug, Default)]
pub enum CardVariant {
    /// Surface with shadow.
    #[default]
    Elevated,
    /// Filled with surface container color.
    Filled,
    /// Outlined with border.
    Outlined,
}

/// Material Design 3 Card.
#[derive(flui_core::IntoElement)]
pub struct Card {
    variant: CardVariant,
    children: SmallVec<[AnyElement; 2]>,
}

impl Card {
    pub fn new() -> Self {
        Self { variant: CardVariant::Elevated, children: SmallVec::new() }
    }

    pub fn variant(mut self, v: CardVariant) -> Self { self.variant = v; self }
    pub fn elevated() -> Self { Self::new().variant(CardVariant::Elevated) }
    pub fn filled() -> Self { Self::new().variant(CardVariant::Filled) }
    pub fn outlined() -> Self { Self::new().variant(CardVariant::Outlined) }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element()); self
    }
}

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let shape = theme.shape.medium;
        let spacing = theme.spacing.lg;

        let mut d = div().rounded(shape).p(spacing);

        d = match self.variant {
            CardVariant::Elevated => {
                d.bg(theme.color_scheme.surface_container_high).shadow_sm()
            }
            CardVariant::Filled => {
                d.bg(theme.color_scheme.surface_container_high)
            }
            CardVariant::Outlined => {
                d.bg(theme.color_scheme.surface)
                    .border_1()
                    .border_color(theme.color_scheme.outline_variant)
            }
        };

        d.children(self.children)
    }
}
