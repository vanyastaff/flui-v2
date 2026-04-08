use flui_core::Pixels;

/// Defines padding/margin offsets for all four edges.
///
/// Equivalent to Flutter's `EdgeInsets`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EdgeInsets {
    pub left: Pixels,
    pub top: Pixels,
    pub right: Pixels,
    pub bottom: Pixels,
}

impl EdgeInsets {
    /// Uniform padding on all sides.
    pub fn all(value: Pixels) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    /// Symmetric horizontal and vertical padding.
    pub fn symmetric(horizontal: Pixels, vertical: Pixels) -> Self {
        Self {
            left: horizontal,
            top: vertical,
            right: horizontal,
            bottom: vertical,
        }
    }

    /// Individual padding for each edge.
    pub fn only(left: Pixels, top: Pixels, right: Pixels, bottom: Pixels) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// Zero padding on all sides.
    pub fn zero() -> Self {
        Self::default()
    }
}
