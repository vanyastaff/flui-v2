/// Whether the system or theme uses light or dark mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Brightness {
    /// Light theme variant.
    #[default]
    Light,
    /// Dark theme variant.
    Dark,
}
