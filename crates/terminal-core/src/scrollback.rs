use std::collections::VecDeque;

use crate::{Cell, CellOccupancy};

/// Maximum number of complete Primary-screen rows retained as scrollback.
///
/// At capacity, capturing a newer row evicts the oldest row. This is a fixed
/// project-owned bound; runtime configuration is intentionally deferred.
pub const MAX_SCROLLBACK_ROWS: usize = 10_000;

/// Bounded ordered storage for complete historical screen rows.
#[derive(Debug)]
pub(crate) struct Scrollback {
    rows: VecDeque<Vec<Cell>>,
}

impl Scrollback {
    pub(crate) fn new() -> Self {
        Self {
            rows: VecDeque::with_capacity(MAX_SCROLLBACK_ROWS),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.rows.len()
    }

    pub(crate) fn row(&self, index: usize) -> Option<&[Cell]> {
        self.rows.get(index).map(Vec::as_slice)
    }

    pub(crate) fn push(&mut self, row: &[Cell]) {
        debug_assert!(row_is_valid(row));
        if self.rows.len() == MAX_SCROLLBACK_ROWS {
            self.rows.pop_front();
        }
        self.rows.push_back(row.to_vec());
        debug_assert!(self.rows.iter().all(|row| row_is_valid(row)));
    }
}

fn row_is_valid(row: &[Cell]) -> bool {
    row.iter()
        .enumerate()
        .all(|(column, cell)| match cell.occupancy() {
            CellOccupancy::Single => true,
            CellOccupancy::WideLead => row
                .get(column + 1)
                .is_some_and(|next| next.occupancy() == CellOccupancy::WideContinuation),
            CellOccupancy::WideContinuation => {
                column > 0 && row[column - 1].occupancy() == CellOccupancy::WideLead
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_the_oldest_row_when_at_capacity() {
        let mut scrollback = Scrollback::new();
        for character in 0..=MAX_SCROLLBACK_ROWS {
            scrollback.push(&[Cell::new(
                char::from_u32(u32::from(b'A') + u32::try_from(character % 26).unwrap()).unwrap(),
                Default::default(),
            )]);
        }

        assert_eq!(scrollback.len(), MAX_SCROLLBACK_ROWS);
        assert_eq!(scrollback.row(0).unwrap()[0].character(), 'B');
    }
}
