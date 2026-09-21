//! Renderer-owned font discovery and initial face selection.
//!
//! Shaping, fallback execution, rasterization, and glyph caching intentionally
//! build on this retained database later.

use std::error::Error;
use std::fmt;

use fontdb::{Database, Family, ID, Query};
use swash::{
    FontRef,
    scale::{Render, ScaleContext, Source},
    shape::ShapeContext,
    zeno::Format,
};

/// Positioned glyph produced from the renderer's selected font face.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapedGlyph {
    glyph_id: u16,
    advance: f32,
    offset_x: f32,
    offset_y: f32,
    cluster_start: u32,
    cluster_end: u32,
}

impl ShapedGlyph {
    pub fn glyph_id(&self) -> u16 {
        self.glyph_id
    }

    pub fn advance(&self) -> f32 {
        self.advance
    }

    pub fn offset_x(&self) -> f32 {
        self.offset_x
    }

    pub fn offset_y(&self) -> f32 {
        self.offset_y
    }

    pub fn cluster_range(&self) -> std::ops::Range<u32> {
        self.cluster_start..self.cluster_end
    }
}

/// Project-owned positioned glyph output for one terminal text run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShapedText {
    glyphs: Vec<ShapedGlyph>,
}

impl ShapedText {
    pub fn glyphs(&self) -> &[ShapedGlyph] {
        &self.glyphs
    }
}

/// CPU-side alpha bitmap and placement for one rasterized glyph.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphBitmap {
    glyph_id: u16,
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    bearing_x: i32,
    bearing_y: i32,
    advance: f32,
}

impl GlyphBitmap {
    pub fn glyph_id(&self) -> u16 {
        self.glyph_id
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn bearing_x(&self) -> i32 {
        self.bearing_x
    }

    pub fn bearing_y(&self) -> i32 {
        self.bearing_y
    }

    pub fn advance(&self) -> f32 {
        self.advance
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FontProcessingError {
    InvalidPixelsPerEm,
    FaceDataUnavailable,
    InvalidFaceData,
    GlyphNotRasterizable(u16),
}

impl fmt::Display for FontProcessingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPixelsPerEm => {
                formatter.write_str("pixels per em must be finite and positive")
            }
            Self::FaceDataUnavailable => {
                formatter.write_str("selected font face data is unavailable")
            }
            Self::InvalidFaceData => formatter.write_str("selected font face data is invalid"),
            Self::GlyphNotRasterizable(glyph_id) => {
                write!(
                    formatter,
                    "glyph {glyph_id} cannot be rasterized as an alpha outline"
                )
            }
        }
    }
}

impl Error for FontProcessingError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum FontRequest {
    /// Use the platform's configured monospace family.
    #[default]
    SystemMonospace,
    /// Require this named monospace family.
    Family(String),
}

#[derive(Debug)]
pub(super) enum FontLoadError {
    NoSystemMonospaceFace,
    RequestedFamilyUnavailable(String),
    RequestedFamilyIsNotMonospace(String),
}

impl fmt::Display for FontLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSystemMonospaceFace => {
                formatter.write_str("no usable system monospace font face was found")
            }
            Self::RequestedFamilyUnavailable(family) => {
                write!(formatter, "requested font family `{family}` was not found")
            }
            Self::RequestedFamilyIsNotMonospace(family) => {
                write!(
                    formatter,
                    "requested font family `{family}` is not monospaced"
                )
            }
        }
    }
}

impl Error for FontLoadError {}

/// Loaded font data retained by the renderer for future shaping and rasterization.
pub(super) struct FontSystem {
    database: Database,
    primary_face: ID,
    shape_context: ShapeContext,
    scale_context: ScaleContext,
}

impl FontSystem {
    /// Discovers platform fonts and selects an initial terminal face.
    pub(super) fn load_system(request: FontRequest) -> Result<Self, FontLoadError> {
        let mut database = Database::new();
        database.load_system_fonts();
        Self::from_database(database, request)
    }

    fn from_database(database: Database, request: FontRequest) -> Result<Self, FontLoadError> {
        let primary_face = select_primary_face(&database, &request)?;
        Ok(Self {
            database,
            primary_face,
            shape_context: ShapeContext::new(),
            scale_context: ScaleContext::new(),
        })
    }

    /// Gives a later shaping or rasterization stage temporary access to the selected face bytes.
    pub(super) fn shape_text(
        &mut self,
        text: &str,
        pixels_per_em: f32,
    ) -> Result<ShapedText, FontProcessingError> {
        validate_pixels_per_em(pixels_per_em)?;
        if text.is_empty() {
            return Ok(ShapedText::default());
        }
        let primary_face = self.primary_face;
        let shape_context = &mut self.shape_context;
        self.database
            .with_face_data(primary_face, |data, index| {
                let font = FontRef::from_index(data, index as usize)
                    .ok_or(FontProcessingError::InvalidFaceData)?;
                let mut shaper = shape_context.builder(font).size(pixels_per_em).build();
                shaper.add_str(text);
                let mut glyphs = Vec::new();
                shaper.shape_with(|cluster| {
                    glyphs.extend(cluster.glyphs.iter().map(|glyph| ShapedGlyph {
                        glyph_id: glyph.id,
                        advance: glyph.advance,
                        offset_x: glyph.x,
                        offset_y: glyph.y,
                        cluster_start: cluster.source.start,
                        cluster_end: cluster.source.end,
                    }));
                });
                Ok(ShapedText { glyphs })
            })
            .ok_or(FontProcessingError::FaceDataUnavailable)?
    }

    pub(super) fn rasterize_glyph(
        &mut self,
        glyph: &ShapedGlyph,
        pixels_per_em: f32,
    ) -> Result<GlyphBitmap, FontProcessingError> {
        validate_pixels_per_em(pixels_per_em)?;
        let primary_face = self.primary_face;
        let scale_context = &mut self.scale_context;
        self.database
            .with_face_data(primary_face, |data, index| {
                let font = FontRef::from_index(data, index as usize)
                    .ok_or(FontProcessingError::InvalidFaceData)?;
                let mut scaler = scale_context
                    .builder(font)
                    .size(pixels_per_em)
                    .hint(true)
                    .build();
                let mut renderer = Render::new(&[Source::Outline]);
                renderer.format(Format::Alpha);
                let image = renderer
                    .render(&mut scaler, glyph.glyph_id)
                    .ok_or(FontProcessingError::GlyphNotRasterizable(glyph.glyph_id))?;
                Ok(GlyphBitmap {
                    glyph_id: glyph.glyph_id,
                    pixels: image.data,
                    width: image.placement.width,
                    height: image.placement.height,
                    bearing_x: image.placement.left,
                    bearing_y: image.placement.top,
                    advance: glyph.advance,
                })
            })
            .ok_or(FontProcessingError::FaceDataUnavailable)?
    }
}

fn validate_pixels_per_em(pixels_per_em: f32) -> Result<(), FontProcessingError> {
    if pixels_per_em.is_finite() && pixels_per_em > 0.0 {
        Ok(())
    } else {
        Err(FontProcessingError::InvalidPixelsPerEm)
    }
}

fn select_primary_face(database: &Database, request: &FontRequest) -> Result<ID, FontLoadError> {
    let (family, missing_error) = match request {
        FontRequest::SystemMonospace => (Family::Monospace, FontLoadError::NoSystemMonospaceFace),
        FontRequest::Family(name) => (
            Family::Name(name),
            FontLoadError::RequestedFamilyUnavailable(name.clone()),
        ),
    };
    let id = database.query(&Query {
        families: &[family],
        ..Default::default()
    });
    let Some(id) = id else {
        return Err(missing_error);
    };
    if database.face(id).is_some_and(|face| face.monospaced) {
        Ok(id)
    } else {
        match request {
            FontRequest::SystemMonospace => Err(FontLoadError::NoSystemMonospaceFace),
            FontRequest::Family(name) => {
                Err(FontLoadError::RequestedFamilyIsNotMonospace(name.clone()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fontdb::{FaceInfo, Language, Source, Stretch, Style, Weight};

    use super::{
        FontLoadError, FontProcessingError, FontRequest, FontSystem, ScaleContext, ShapeContext,
    };

    const TEST_FONT: &[u8] = include_bytes!("../tests/fixtures/Tuffy.ttf");

    fn database_with_face(family: &str, monospaced: bool) -> fontdb::Database {
        let mut database = fontdb::Database::new();
        database.push_face_info(FaceInfo {
            id: fontdb::ID::dummy(),
            source: Source::Binary(Arc::new(vec![1, 2, 3])),
            index: 0,
            families: vec![(family.to_owned(), Language::English_UnitedStates)],
            post_script_name: format!("{family}-Regular"),
            style: Style::Normal,
            weight: Weight::NORMAL,
            stretch: Stretch::Normal,
            monospaced,
        });
        database
    }

    fn fixture_system() -> FontSystem {
        let mut database = fontdb::Database::new();
        database.load_font_data(TEST_FONT.to_vec());
        let primary_face = database.faces().next().unwrap().id;
        FontSystem {
            database,
            primary_face,
            shape_context: ShapeContext::new(),
            scale_context: ScaleContext::new(),
        }
    }

    #[test]
    fn default_request_selects_the_configured_monospace_family() {
        let mut database = database_with_face("Test Mono", true);
        database.set_monospace_family("Test Mono");

        assert!(FontSystem::from_database(database, FontRequest::default()).is_ok());
    }

    #[test]
    fn default_request_reports_when_no_system_monospace_face_is_available() {
        assert!(matches!(
            FontSystem::from_database(fontdb::Database::new(), FontRequest::default()),
            Err(FontLoadError::NoSystemMonospaceFace)
        ));
    }

    #[test]
    fn named_request_requires_a_matching_monospace_face() {
        let database = database_with_face("Proportional", false);

        assert!(matches!(
            FontSystem::from_database(database, FontRequest::Family("Proportional".to_owned())),
            Err(FontLoadError::RequestedFamilyIsNotMonospace(ref family)) if family == "Proportional"
        ));
    }

    #[test]
    fn named_request_reports_a_missing_family() {
        let database = database_with_face("Available Mono", true);

        assert!(matches!(
            FontSystem::from_database(database, FontRequest::Family("Missing".to_owned())),
            Err(FontLoadError::RequestedFamilyUnavailable(ref family)) if family == "Missing"
        ));
    }

    #[test]
    fn shapes_basic_text_in_source_order_with_positioned_advances() {
        let mut system = fixture_system();

        let shaped = system.shape_text("ABC", 16.0).unwrap();

        assert_eq!(shaped.glyphs().len(), 3);
        assert!(shaped.glyphs().iter().all(|glyph| glyph.advance() > 0.0));
        assert_eq!(shaped.glyphs()[0].cluster_range(), 0..1);
        assert_eq!(shaped.glyphs()[1].cluster_range(), 1..2);
        assert_eq!(shaped.glyphs()[2].cluster_range(), 2..3);
    }

    #[test]
    fn rasterizes_a_shaped_glyph_to_an_alpha_bitmap() {
        let mut system = fixture_system();
        let shaped = system.shape_text("A", 16.0).unwrap();

        let bitmap = system.rasterize_glyph(&shaped.glyphs()[0], 16.0).unwrap();

        assert!(bitmap.width() > 0);
        assert!(bitmap.height() > 0);
        assert_eq!(
            bitmap.pixels().len(),
            (bitmap.width() * bitmap.height()) as usize
        );
        assert!(bitmap.pixels().iter().any(|pixel| *pixel != 0));
        assert!(bitmap.advance() > 0.0);
    }

    #[test]
    fn empty_text_and_invalid_pixel_sizes_are_deterministic() {
        let mut system = fixture_system();

        assert!(system.shape_text("", 16.0).unwrap().glyphs().is_empty());
        assert_eq!(
            system.shape_text("A", 0.0),
            Err(FontProcessingError::InvalidPixelsPerEm)
        );
    }
}
