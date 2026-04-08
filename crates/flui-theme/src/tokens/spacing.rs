use flui_core::Pixels;

/// Spacing scale tokens.
#[derive(Clone, Debug)]
pub struct SpacingTheme {
    /// 4px
    pub xs: Pixels,
    /// 8px
    pub sm: Pixels,
    /// 12px
    pub md: Pixels,
    /// 16px
    pub lg: Pixels,
    /// 24px
    pub xl: Pixels,
    /// 32px
    pub xxl: Pixels,
    /// 48px
    pub xxxl: Pixels,
}

impl Default for SpacingTheme {
    fn default() -> Self {
        use flui_core::px;
        Self {
            xs: px(4.),
            sm: px(8.),
            md: px(12.),
            lg: px(16.),
            xl: px(24.),
            xxl: px(32.),
            xxxl: px(48.),
        }
    }
}
