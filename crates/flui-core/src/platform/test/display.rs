use crate::{Bounds, DisplayId, Pixels, PlatformDisplay, Point, px};
use anyhow::{Ok, Result};

#[derive(Debug)]
pub(crate) struct TestDisplay {
    id: DisplayId,
    uuid: uuid::Uuid,
    bounds: Bounds<Pixels>,
}

impl TestDisplay {
    pub fn new() -> Self {
        TestDisplay {
            id: DisplayId(1),
            uuid: uuid::Uuid::new_v4(),
            bounds: Bounds::from_corners(Point::default(), Point::new(px(1920.), px(1080.))),
        }
    }

    /// ADR-007 test hook: create a TestDisplay with a caller-chosen id.
    /// Use when a test needs to swap between distinct displays to drive
    /// `Window::observe_display_change` (decision 3 + 5).
    ///
    /// `#[cfg(test)]` because this is only consumed by the ADR-007
    /// observer regression test today; if a non-test caller appears,
    /// drop the gate.
    #[cfg(test)]
    pub(crate) fn with_id(raw_id: u32) -> Self {
        TestDisplay {
            id: DisplayId(raw_id),
            uuid: uuid::Uuid::new_v4(),
            bounds: Bounds::from_corners(Point::default(), Point::new(px(1920.), px(1080.))),
        }
    }
}

impl PlatformDisplay for TestDisplay {
    fn id(&self) -> crate::DisplayId {
        self.id
    }

    fn uuid(&self) -> Result<uuid::Uuid> {
        Ok(self.uuid)
    }

    fn bounds(&self) -> crate::Bounds<crate::Pixels> {
        self.bounds
    }
}
