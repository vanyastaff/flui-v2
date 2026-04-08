use flui_core::Hsla;

/// Complete color scheme tokens for a theme.
///
/// Based on Material Design 3 color roles, but universal enough
/// for any design system (Fluent, Cupertino, Nord, etc).
#[derive(Clone, Debug)]
pub struct ColorScheme {
    // Primary
    pub primary: Hsla,
    pub on_primary: Hsla,
    pub primary_container: Hsla,
    pub on_primary_container: Hsla,

    // Secondary
    pub secondary: Hsla,
    pub on_secondary: Hsla,
    pub secondary_container: Hsla,
    pub on_secondary_container: Hsla,

    // Tertiary
    pub tertiary: Hsla,
    pub on_tertiary: Hsla,

    // Error
    pub error: Hsla,
    pub on_error: Hsla,
    pub error_container: Hsla,
    pub on_error_container: Hsla,

    // Surface
    pub surface: Hsla,
    pub on_surface: Hsla,
    pub surface_variant: Hsla,
    pub on_surface_variant: Hsla,
    pub surface_container: Hsla,
    pub surface_container_high: Hsla,
    pub surface_container_low: Hsla,

    // Background
    pub background: Hsla,
    pub on_background: Hsla,

    // Outline
    pub outline: Hsla,
    pub outline_variant: Hsla,

    // Inverse
    pub inverse_surface: Hsla,
    pub inverse_on_surface: Hsla,
    pub inverse_primary: Hsla,

    // Scrim / Shadow
    pub scrim: Hsla,
    pub shadow: Hsla,

    // Semantic
    pub warning: Hsla,
    pub success: Hsla,
}
