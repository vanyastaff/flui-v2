//! ADR-018: named priority constants for [`Window::defer_draw`].
//!
//! `defer_draw(priority)` is the only overlay layering mechanism per
//! ADR-018 decision 1; widget libraries should compose with these
//! constants instead of inventing magic numbers. The ranges are
//! conventions — the engine does not enforce them — but using the
//! named values keeps adjacent widget libraries from picking
//! conflicting numbers.
//!
//! Per-range conventions from ADR-018 decision 2:
//!
//! | Range | Use |
//! |-------|-----|
//! | `0..1000` | In-tree visual layering (drop shadows above siblings, etc.) |
//! | `1000..10000` | Tooltips, hover popovers |
//! | `10000..100000` | Drop-down menus, autocomplete |
//! | `100000..1000000` | Modals / dialogs |
//! | `1000000..` | Drag preview, top-most system overlays |
//!
//! Two overlays at the same priority follow insertion order (last
//! wins) per ADR-018 decision 6. Hit-test traversal uses the same
//! priority order as paint per decision 3 — the element drawn on
//! top is the element that wins the click.
//!
//! See `docs/research/adr/ADR-018-modal-overlay-layering.md`.

/// In-tree visual layering — drop shadows above siblings, decorative
/// overlays that should sit just above their owning element. Below
/// every floating UI.
pub const Z_IN_TREE: usize = 100;

/// Tooltips and hover popovers. Above every in-tree decoration; below
/// drop-down menus so a menu opened from a tooltip-bearing trigger
/// covers the tooltip.
pub const Z_TOOLTIP: usize = 1_000;

/// Drop-down menus, autocomplete popups, combo boxes. Above tooltips
/// (a menu opened while a tooltip is shown takes precedence) and
/// below modals (a modal opened from a menu covers it).
pub const Z_DROPDOWN: usize = 10_000;

/// Modal dialogs. The canonical [`modal_backdrop`] helper paints at
/// `Z_MODAL - 1` so the backdrop catches clicks below the modal
/// content but above every dropdown/tooltip.
///
/// [`modal_backdrop`]: crate::elements::modal_backdrop
pub const Z_MODAL: usize = 100_000;

/// Drag preview and top-most system overlays. Above modals so an
/// in-flight drag preview tracks the cursor over a modal dialog.
pub const Z_DRAG_PREVIEW: usize = 1_000_000;

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR-018 — z-stack ordering is the load-bearing invariant: the
    /// tier of constants must be strictly increasing so a higher-tier
    /// overlay always paints above a lower-tier one. A regression that
    /// flips two constants would silently put modals below drop-downs,
    /// for instance.
    #[test]
    fn adr_018_z_constants_are_strictly_increasing() {
        assert!(Z_IN_TREE < Z_TOOLTIP);
        assert!(Z_TOOLTIP < Z_DROPDOWN);
        assert!(Z_DROPDOWN < Z_MODAL);
        assert!(Z_MODAL < Z_DRAG_PREVIEW);
    }

    /// ADR-018 decision 2 — each constant sits in its documented range.
    /// Locks the range conventions so a future bump cannot accidentally
    /// move a constant out of its tier (e.g. setting Z_MODAL to 5_000
    /// would put it in the dropdown range).
    #[test]
    fn adr_018_z_constants_fit_in_documented_ranges() {
        assert!((0..1_000).contains(&Z_IN_TREE), "Z_IN_TREE in 0..1000");
        assert!(
            (1_000..10_000).contains(&Z_TOOLTIP),
            "Z_TOOLTIP in 1000..10000"
        );
        assert!(
            (10_000..100_000).contains(&Z_DROPDOWN),
            "Z_DROPDOWN in 10000..100000"
        );
        assert!(
            (100_000..1_000_000).contains(&Z_MODAL),
            "Z_MODAL in 100000..1000000"
        );
        assert!(
            Z_DRAG_PREVIEW >= 1_000_000,
            "Z_DRAG_PREVIEW in 1000000..*"
        );
    }
}
