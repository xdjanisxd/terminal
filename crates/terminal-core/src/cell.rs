/// A terminal cell color independent of rendering APIs.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CellColor {
    #[default]
    Default,
    Indexed(u8),
    Rgb {
        red: u8,
        green: u8,
        blue: u8,
    },
}

/// Presentation attributes stored with one terminal cell.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CellAttributes {
    foreground: CellColor,
    background: CellColor,
}

impl CellAttributes {
    pub const fn new(foreground: CellColor, background: CellColor) -> Self {
        Self {
            foreground,
            background,
        }
    }

    pub const fn foreground(self) -> CellColor {
        self.foreground
    }

    pub const fn background(self) -> CellColor {
        self.background
    }
}

/// One fixed-grid terminal cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cell {
    character: char,
    attributes: CellAttributes,
}

impl Cell {
    pub const fn new(character: char, attributes: CellAttributes) -> Self {
        Self {
            character,
            attributes,
        }
    }

    pub const fn character(self) -> char {
        self.character
    }

    pub const fn attributes(&self) -> &CellAttributes {
        &self.attributes
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            character: ' ',
            attributes: CellAttributes::default(),
        }
    }
}
