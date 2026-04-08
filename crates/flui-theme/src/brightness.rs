/// Whether the theme is light or dark.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Brightness {
    /// Light theme variant.
    #[default]
    Light,
    /// Dark theme variant.
    Dark,
}
