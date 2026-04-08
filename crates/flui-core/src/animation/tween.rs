// crates/flui-core/src/animation/tween.rs

use super::lerp::Lerp;

/// Interpolates between two values of the same type.
///
/// # Example
/// ```ignore
/// let tween = Tween::new(0.0f32, 100.0);
/// assert_eq!(tween.transform(0.5), 50.0);
/// ```
#[derive(Clone, Debug)]
pub struct Tween<T: Lerp> {
    pub begin: T,
    pub end: T,
}

impl<T: Lerp> Tween<T> {
    /// Create a new tween from `begin` to `end`.
    pub fn new(begin: T, end: T) -> Self {
        Self { begin, end }
    }

    /// Get the interpolated value at `t` (0.0..=1.0).
    pub fn transform(&self, t: f32) -> T {
        self.begin.lerp(&self.end, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f32_tween() {
        let tween = Tween::new(0.0f32, 100.0);
        assert_eq!(tween.transform(0.0), 0.0);
        assert_eq!(tween.transform(0.5), 50.0);
        assert_eq!(tween.transform(1.0), 100.0);
    }

    #[test]
    fn test_reverse_tween() {
        let tween = Tween::new(100.0f32, 0.0);
        assert_eq!(tween.transform(0.0), 100.0);
        assert_eq!(tween.transform(1.0), 0.0);
    }
}
