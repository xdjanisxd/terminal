//! Streaming cell reflow. Screen ownership, cursor policy and history limits
//! remain in `screens`; this module only packs logical lines and maps anchors.
use std::collections::VecDeque;

use crate::Cell;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ReflowPoint {
    pub row: usize,
    pub column: usize,
    /// Mouse endpoints identify a cell; caret/cursor endpoints are boundaries.
    pub cell: bool,
    pub mapped: (usize, usize),
}

impl ReflowPoint {
    pub fn new(row: usize, column: usize, cell: bool) -> Self {
        Self {
            row,
            column,
            cell,
            mapped: (row, column),
        }
    }
}

pub(crate) type Row = (Vec<Cell>, usize);

pub(crate) struct Reflow {
    pub rows: VecDeque<Row>,
    pub generated: usize,
    columns: usize,
    limit: usize,
    current: Vec<Cell>,
}

impl Reflow {
    pub fn new(columns: usize, limit: usize) -> Self {
        Self {
            rows: VecDeque::new(),
            generated: 0,
            columns,
            limit,
            current: Vec::new(),
        }
    }

    fn emit(&mut self, wrapped: usize) {
        let mut next = if self.rows.len() == self.limit {
            self.rows.pop_front().unwrap().0
        } else {
            Vec::new()
        };
        next.clear();
        let cells = std::mem::replace(&mut self.current, next);
        self.rows.push_back((cells, wrapped));
        self.generated += 1;
    }

    pub fn push(&mut self, row: usize, cells: &[Cell], wrapped: usize, points: &mut [ReflowPoint]) {
        let has_points = points.iter().any(|point| point.row == row);
        let mut length = if wrapped != 0 {
            wrapped.min(cells.len())
        } else {
            cells
                .iter()
                .rposition(|cell| *cell != Cell::default())
                .map_or(0, |i| i + 1)
        };
        if length != 0
            && length < cells.len()
            && cells[length - 1].is_wide_lead()
            && cells[length].is_wide_continuation()
        {
            length += 1;
        }
        if wrapped == 0 {
            for point in points.iter().filter(|p| p.row == row && !p.cell) {
                length = length.max(point.column.min(cells.len()));
            }
        }
        let mut column = 0;
        self.current
            .reserve_exact(length.min(self.columns - self.current.len()));
        // Squeezed wide cells can only come from a one-column source row.
        let expand_wide = cells.len() == 1 && cells[0].is_narrow_wide();
        while column < length {
            // The usual path copies a run of complete cells in one operation.
            // Only a wide glyph at the packing boundary needs special care.
            if self.columns > 1 && !expand_wide {
                let mut count = (length - column).min(self.columns - self.current.len());
                if count != 0 && cells[column + count - 1].is_wide_lead() {
                    count -= 1;
                }
                if count == 0 {
                    self.emit(self.current.len());
                    continue;
                }
                if has_points {
                    for point in points
                        .iter_mut()
                        .filter(|p| p.row == row && p.column >= column && p.column < column + count)
                    {
                        point.mapped = (self.generated, self.current.len() + point.column - column);
                    }
                }
                self.current
                    .extend_from_slice(&cells[column..column + count]);
                column += count;
                continue;
            }
            let cell = cells[column];
            let source_width = if cell.is_wide_lead() { 2 } else { 1 };
            let width = cell.reflow_width().min(self.columns);
            if self.current.len() + width > self.columns {
                self.emit(self.current.len());
            }
            if has_points {
                for point in points.iter_mut().filter(|p| {
                    p.row == row && p.column >= column && p.column < column + source_width
                }) {
                    point.mapped = (
                        self.generated,
                        self.current.len() + (point.column - column).min(width - 1),
                    );
                }
            }
            self.current.push(cell.with_reflow_width(self.columns));
            if width == 2 {
                self.current.push(Cell::wide_continuation());
            }
            column += source_width;
        }
        for point in points
            .iter_mut()
            .filter(|p| p.row == row && p.column >= length)
        {
            point.mapped = (
                self.generated,
                if point.cell {
                    self.current.len().saturating_sub(1)
                } else {
                    self.current.len()
                },
            );
        }
        if wrapped == 0 {
            self.emit(0);
        }
    }

    pub fn finish(&mut self) {
        if !self.current.is_empty() {
            self.emit(0);
        }
    }
}
