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

/// Selects normal or bold/intense text rendering.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextIntensity {
    #[default]
    Normal,
    Bold,
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
