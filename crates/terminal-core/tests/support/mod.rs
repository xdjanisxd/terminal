use terminal_core::{Cell, CellAttributes, CellColor, CellOccupancy};

pub fn assert_blank(cell: &Cell, background: CellColor) {
    assert_eq!(cell.character(), ' ');
    assert_eq!(
        *cell.attributes(),
        CellAttributes::new(CellColor::Default, background)
    );
    assert_eq!(cell.occupancy(), CellOccupancy::Single);
    assert!(cell.combining_marks().is_empty());
    assert!(cell.target_id().is_none());
}
