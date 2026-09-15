/// Inclusive zero-based vertical scrolling margins bounded by a screen height.
///
/// A one-row region is representable so the invariant remains valid for a
/// one-row terminal. Protocol adapters can impose stricter command syntax.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerticalScrollingMargins {
    top: usize,
    bottom: usize,
}

impl VerticalScrollingMargins {
    /// Creates margins when both rows are in bounds and ordered.
    pub const fn new(top: usize, bottom: usize, screen_rows: usize) -> Option<Self> {
        if screen_rows == 0 || top > bottom || bottom >= screen_rows {
            None
        } else {
            Some(Self { top, bottom })
        }
    }

    /// Creates the full-screen region for a non-empty screen.
    ///
    /// # Panics
    ///
    /// Panics when `screen_rows` is zero. Valid terminal dimensions always have
    /// at least one row.
    pub const fn full_screen(screen_rows: usize) -> Self {
        assert!(screen_rows > 0, "a scrolling region requires a screen row");
        Self {
            top: 0,
            bottom: screen_rows - 1,
        }
    }

    /// Returns the inclusive zero-based top margin.
    pub const fn top(self) -> usize {
        self.top
    }

    /// Returns the inclusive zero-based bottom margin.
    pub const fn bottom(self) -> usize {
        self.bottom
    }
}
