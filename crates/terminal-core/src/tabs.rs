const DEFAULT_TAB_INTERVAL: usize = 8;

/// Bounded horizontal tab-stop state for the active terminal.
#[derive(Debug)]
pub(crate) struct HorizontalTabStops {
    stops: Vec<bool>,
}

impl HorizontalTabStops {
    pub(crate) fn new(columns: usize) -> Self {
        Self {
            stops: (0..columns).map(Self::is_default_stop).collect(),
        }
    }

    pub(crate) fn has(&self, column: usize) -> bool {
        self.stops.get(column).copied().unwrap_or(false)
    }

    pub(crate) fn next_after(&self, column: usize) -> Option<usize> {
        self.stops
            .iter()
            .enumerate()
            .skip(column.saturating_add(1))
            .find_map(|(column, &is_stop)| is_stop.then_some(column))
    }

    pub(crate) fn set(&mut self, column: usize) {
        if let Some(stop) = self.stops.get_mut(column) {
            *stop = true;
        }
    }

    pub(crate) fn clear(&mut self, column: usize) {
        if let Some(stop) = self.stops.get_mut(column) {
            *stop = false;
        }
    }

    pub(crate) fn clear_all(&mut self) {
        self.stops.fill(false);
    }

    /// Preserves surviving stops, drops truncated stops, and initializes only
    /// newly exposed columns with conventional defaults.
    pub(crate) fn resize(&mut self, columns: usize) {
        let mut column = self.stops.len();
        self.stops.resize_with(columns, || {
            let is_stop = Self::is_default_stop(column);
            column += 1;
            is_stop
        });
    }

    fn is_default_stop(column: usize) -> bool {
        column != 0 && column.is_multiple_of(DEFAULT_TAB_INTERVAL)
    }
}
