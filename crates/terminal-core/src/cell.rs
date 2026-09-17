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

/// Selects normal, bold, or faint text rendering.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextIntensity {
    #[default]
    Normal,
    Bold,
    Faint,
}

/// Selects upright or italic text rendering.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ItalicStyle {
    #[default]
    Upright,
    Italic,
}

/// Selects whether a cell is underlined.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UnderlineStyle {
    #[default]
    Disabled,
    Enabled,
}

/// Selects whether foreground and background are presented inversely.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InverseVideo {
    #[default]
    Disabled,
    Enabled,
}

/// Presentation attributes stored with a cell or held as current rendition.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CellAttributes {
    foreground: CellColor,
    background: CellColor,
    intensity: TextIntensity,
    italic: ItalicStyle,
    underline: UnderlineStyle,
    inverse: InverseVideo,
}

impl CellAttributes {
    /// Creates attributes with explicit colors and default text styles.
    pub const fn new(foreground: CellColor, background: CellColor) -> Self {
        Self {
            foreground,
            background,
            intensity: TextIntensity::Normal,
            italic: ItalicStyle::Upright,
            underline: UnderlineStyle::Disabled,
            inverse: InverseVideo::Disabled,
        }
    }

    pub const fn foreground(self) -> CellColor {
        self.foreground
    }

    pub const fn background(self) -> CellColor {
        self.background
    }

    pub const fn intensity(self) -> TextIntensity {
        self.intensity
    }

    pub const fn italic(self) -> ItalicStyle {
        self.italic
    }

    pub const fn underline(self) -> UnderlineStyle {
        self.underline
    }

    pub const fn inverse(self) -> InverseVideo {
        self.inverse
    }

    pub(crate) fn set_foreground(&mut self, foreground: CellColor) {
        self.foreground = foreground;
    }

    pub(crate) fn set_background(&mut self, background: CellColor) {
        self.background = background;
    }

    pub(crate) fn set_intensity(&mut self, intensity: TextIntensity) {
        self.intensity = intensity;
    }

    pub(crate) fn set_italic(&mut self, italic: ItalicStyle) {
        self.italic = italic;
    }

    pub(crate) fn set_underline(&mut self, underline: UnderlineStyle) {
        self.underline = underline;
    }

    pub(crate) fn set_inverse(&mut self, inverse: InverseVideo) {
        self.inverse = inverse;
    }
}

/// Describes how a cell occupies columns in a fixed terminal row.
///
/// A `WideLead` is valid only when immediately followed by one
/// `WideContinuation`; continuation cells carry no duplicated character or
/// rendition snapshot.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CellOccupancy {
    #[default]
    Single,
    WideLead,
    WideContinuation,
}

/// Maximum width-zero scalars retained on one printable cell.
///
/// Input order is preserved. A further attachment returns a bounded semantic
/// error without changing the cell, cursor, or grid.
pub const MAX_COMBINING_MARKS: usize = 8;

/// One fixed-grid terminal cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cell {
    character: char,
    attributes: CellAttributes,
    occupancy: CellOccupancy,
    has_printable_base: bool,
    combining_marks: [char; MAX_COMBINING_MARKS],
    combining_mark_count: u8,
}

impl Cell {
    /// Creates an ordinary single-column printable cell.
    pub const fn new(character: char, attributes: CellAttributes) -> Self {
        Self {
            character,
            attributes,
            occupancy: CellOccupancy::Single,
            has_printable_base: true,
            combining_marks: ['\0'; MAX_COMBINING_MARKS],
            combining_mark_count: 0,
        }
    }

    pub const fn character(self) -> char {
        self.character
    }

    pub const fn attributes(&self) -> &CellAttributes {
        &self.attributes
    }

    pub const fn occupancy(self) -> CellOccupancy {
        self.occupancy
    }

    /// Returns width-zero scalars attached to this printable base in input order.
    pub fn combining_marks(&self) -> &[char] {
        &self.combining_marks[..usize::from(self.combining_mark_count)]
    }

    pub fn is_wide_lead(self) -> bool {
        self.occupancy == CellOccupancy::WideLead
    }

    pub fn is_wide_continuation(self) -> bool {
        self.occupancy == CellOccupancy::WideContinuation
    }

    pub(crate) const fn wide_lead(character: char, attributes: CellAttributes) -> Self {
        Self {
            character,
            attributes,
            occupancy: CellOccupancy::WideLead,
            has_printable_base: true,
            combining_marks: ['\0'; MAX_COMBINING_MARKS],
            combining_mark_count: 0,
        }
    }

    pub(crate) fn wide_continuation() -> Self {
        Self {
            character: ' ',
            attributes: CellAttributes::default(),
            occupancy: CellOccupancy::WideContinuation,
            has_printable_base: false,
            combining_marks: ['\0'; MAX_COMBINING_MARKS],
            combining_mark_count: 0,
        }
    }

    pub(crate) fn append_combining_mark(&mut self, character: char) -> Result<bool, ()> {
        if !self.has_printable_base || self.is_wide_continuation() {
            return Ok(false);
        }
        let index = usize::from(self.combining_mark_count);
        let Some(slot) = self.combining_marks.get_mut(index) else {
            return Err(());
        };
        *slot = character;
        self.combining_mark_count += 1;
        Ok(true)
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            character: ' ',
            attributes: CellAttributes::default(),
            occupancy: CellOccupancy::Single,
            has_printable_base: false,
            combining_marks: ['\0'; MAX_COMBINING_MARKS],
            combining_mark_count: 0,
        }
    }
}
