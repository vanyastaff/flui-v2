use crate::{Brightness, Pixels, Size};

/// Aggregated window + platform data — convenience struct.
/// Equivalent to Flutter's `MediaQueryData`.
#[derive(Clone, Debug)]
pub struct MediaQueryData {
    /// Window content area size in logical pixels.
    pub size: Size<Pixels>,
    /// Device pixel ratio (1.0 on standard displays, 2.0 on Retina).
    pub scale_factor: f32,
    /// OS-level brightness preference.
    pub brightness: Brightness,
    /// OS text scaling factor.
    // TODO: detect from OS (macOS accessibility, GNOME text-scaling-factor, Windows SystemParametersInfo)
    pub text_scale_factor: f32,
}
