use flui_core::Pixels;

/// Border radius tokens.
#[derive(Clone, Debug)]
pub struct ShapeTheme {
    /// No rounding (0px).
    pub none: Pixels,
    /// Extra small radius (4px).
    pub extra_small: Pixels,
    /// Small radius (8px).
    pub small: Pixels,
    /// Medium radius (12px).
    pub medium: Pixels,
    /// Large radius (16px).
    pub large: Pixels,
    /// Extra large radius (28px).
    pub extra_large: Pixels,
    /// Full rounding — pill shape (9999px).
    pub full: Pixels,
}

impl Default for ShapeTheme {
    fn default() -> Self {
        use flui_core::px;
        Self {
            none: px(0.),
            extra_small: px(4.),
            small: px(8.),
            medium: px(12.),
            large: px(16.),
            extra_large: px(28.),
            full: px(9999.),
        }
    }
}
