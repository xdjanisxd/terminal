//! Renderer-owned font discovery and initial face selection.
//!
//! Shaping, fallback execution, rasterization, and glyph caching intentionally
//! build on this retained database later.

use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::fmt;

use fontdb::{Database, Family, ID, Query};
use swash::{
    FontRef,
    scale::{Render, ScaleContext, Source},
    shape::ShapeContext,
    zeno::Format,
};

pub const DEFAULT_LOGICAL_FONT_SIZE: f32 = 16.0;

/// Stable physical terminal cell metrics for one logical font size and DPI scale.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellMetrics {
    width: u32,
    height: u32,
    baseline: u32,
    pixels_per_em: u32,
    ascent: u32,
    descent: u32,
    leading: u32,
}

impl CellMetrics {
    /// Constructs already-validated physical metrics for deterministic consumers/tests.
    pub const fn from_physical(width: u32, height: u32, pixels_per_em: f32) -> Self {
        Self {
            width,
            height,
            baseline: height,
            pixels_per_em: pixels_per_em.to_bits(),
            ascent: (height as f32).to_bits(),
            descent: 0.0_f32.to_bits(),
            leading: 0.0_f32.to_bits(),
        }
    }
    pub fn width(self) -> u32 {
        self.width
    }
    pub fn height(self) -> u32 {
        self.height
    }
    /// Physical distance from the top of the cell to the font baseline.
    pub fn baseline(self) -> u32 {
        self.baseline
    }
    pub fn pixels_per_em(self) -> f32 {
        f32::from_bits(self.pixels_per_em)
    }
    /// Physical ascent from baseline to the top of the font alignment box.
    pub fn ascent(self) -> f32 {
        f32::from_bits(self.ascent)
    }
    /// Physical descent from baseline to the bottom of the font alignment box.
    pub fn descent(self) -> f32 {
        f32::from_bits(self.descent)
    }
    /// Physical inter-line leading recommended by the font.
    pub fn leading(self) -> f32 {
        f32::from_bits(self.leading)
    }
}

/// Positioned glyph produced from the renderer's selected font face.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapedGlyph {
    face_id: ID,
    is_fallback: bool,
    glyph_id: u16,
    advance: f32,
    offset_x: f32,
    offset_y: f32,
    cluster_start: u32,
    cluster_end: u32,
}

impl ShapedGlyph {
    pub(crate) fn face_cache_identity(&self) -> String {
        self.face_id.to_string()
    }

    pub fn glyph_id(&self) -> u16 {
        self.glyph_id
    }

    /// Returns whether this glyph uses a discovered fallback face.
    pub fn uses_fallback(&self) -> bool {
        self.is_fallback
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

const GLYPH_CACHE_CAPACITY: usize = 256;
const SHAPED_TEXT_CACHE_CAPACITY: usize = 512;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct GlyphCacheKey {
    face_id: ID,
    glyph_id: u16,
    pixels_per_em: u32,
}

struct GlyphCache {
    entries: HashMap<GlyphCacheKey, GlyphBitmap>,
    lru: VecDeque<GlyphCacheKey>,
    capacity: usize,
}

impl GlyphCache {
    fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            lru: VecDeque::new(),
            capacity,
        }
    }

    fn get(&mut self, key: GlyphCacheKey) -> Option<GlyphBitmap> {
        let bitmap = self.entries.get(&key)?.clone();
        self.touch(key);
        Some(bitmap)
    }

    fn insert(&mut self, key: GlyphCacheKey, bitmap: GlyphBitmap) {
        if let std::collections::hash_map::Entry::Occupied(mut entry) = self.entries.entry(key) {
            entry.insert(bitmap);
            self.touch(key);
            return;
        }
        if self.entries.len() == self.capacity
            && let Some(oldest) = self.lru.pop_front()
        {
            self.entries.remove(&oldest);
        }
        self.entries.insert(key, bitmap);
        self.lru.push_back(key);
    }

    fn touch(&mut self, key: GlyphCacheKey) {
        if let Some(position) = self.lru.iter().position(|candidate| *candidate == key) {
            self.lru.remove(position);
        }
        self.lru.push_back(key);
    }
}

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
    fallback_candidates: Vec<ID>,
    fallback_by_character: HashMap<char, Option<ID>>,
    glyph_cache: GlyphCache,
    shaped_cache: HashMap<String, ShapedText>,
    shaped_lru: VecDeque<String>,
    shaped_pixels_per_em: Option<u32>,
}

impl FontSystem {
    pub(super) fn cell_metrics(
        &self,
        scale_factor: f64,
    ) -> Result<CellMetrics, FontProcessingError> {
        let pixels_per_em = DEFAULT_LOGICAL_FONT_SIZE * scale_factor as f32;
        validate_pixels_per_em(pixels_per_em)?;
        self.database
            .with_face_data(self.primary_face, |data, index| {
                let font = FontRef::from_index(data, index as usize)
                    .ok_or(FontProcessingError::InvalidFaceData)?;
                let metrics = font.metrics(&[]).scale(pixels_per_em);
                Ok(cell_metrics_from_scaled_values(
                    metrics.max_width,
                    metrics.average_width,
                    metrics.ascent,
                    metrics.descent,
                    metrics.leading,
                    pixels_per_em,
                ))
            })
            .ok_or(FontProcessingError::FaceDataUnavailable)?
    }
    /// Discovers platform fonts and selects an initial terminal face.
    pub(super) fn load_system(request: FontRequest) -> Result<Self, FontLoadError> {
        let mut database = Database::new();
        database.load_system_fonts();
        Self::from_database(database, request)
    }

    fn from_database(database: Database, request: FontRequest) -> Result<Self, FontLoadError> {
        let primary_face = select_primary_face(&database, &request)?;
        Self::with_primary_face(database, primary_face, GLYPH_CACHE_CAPACITY)
    }

    fn with_primary_face(
        database: Database,
        primary_face: ID,
        cache_capacity: usize,
    ) -> Result<Self, FontLoadError> {
        let mut fallback_candidates: Vec<_> = database
            .faces()
            .filter(|face| face.id != primary_face)
            .map(|face| face.id)
            .collect();
        fallback_candidates.sort_by_key(|id| {
            let face = database.face(*id).unwrap();
            (face.post_script_name.clone(), face.id.to_string())
        });
        Ok(Self {
            database,
            primary_face,
            shape_context: ShapeContext::new(),
            scale_context: ScaleContext::new(),
            fallback_candidates,
            fallback_by_character: HashMap::new(),
            glyph_cache: GlyphCache::new(cache_capacity),
            shaped_cache: HashMap::new(),
            shaped_lru: VecDeque::new(),
            shaped_pixels_per_em: None,
        })
    }

    /// Gives a later shaping or rasterization stage temporary access to the selected face bytes.
    pub(super) fn shape_text(
        &mut self,
        text: &str,
        pixels_per_em: f32,
    ) -> Result<ShapedText, FontProcessingError> {
        self.shape_text_cached(text, pixels_per_em)
            .map(|(shaped, _)| shaped)
    }

    pub(super) fn shape_text_cached(
        &mut self,
        text: &str,
        pixels_per_em: f32,
    ) -> Result<(ShapedText, bool), FontProcessingError> {
        validate_pixels_per_em(pixels_per_em)?;
        if text.is_empty() {
            return Ok((ShapedText::default(), true));
        }
        let scale = pixels_per_em.to_bits();
        if self.shaped_pixels_per_em != Some(scale) {
            self.shaped_cache.clear();
            self.shaped_lru.clear();
            self.shaped_pixels_per_em = Some(scale);
        }
        if let Some(shaped) = self.shaped_cache.get(text).cloned() {
            if let Some(position) = self.shaped_lru.iter().position(|key| key == text) {
                let key = self.shaped_lru.remove(position).expect("known cache key");
                self.shaped_lru.push_back(key);
            }
            return Ok((shaped, true));
        }
        let shaped = self.shape_text_uncached(text, pixels_per_em)?;
        if self.shaped_cache.len() == SHAPED_TEXT_CACHE_CAPACITY
            && let Some(oldest) = self.shaped_lru.pop_front()
        {
            self.shaped_cache.remove(&oldest);
        }
        self.shaped_cache.insert(text.to_owned(), shaped.clone());
        self.shaped_lru.push_back(text.to_owned());
        Ok((shaped, false))
    }

    fn shape_text_uncached(
        &mut self,
        text: &str,
        pixels_per_em: f32,
    ) -> Result<ShapedText, FontProcessingError> {
        let mut glyphs = Vec::new();
        let mut run_start = 0;
        let mut run_face = None;
        for (offset, character) in text.char_indices() {
            let face = self.face_for_character(character);
            if let Some(previous_face) = run_face
                && face != previous_face
            {
                self.shape_run(
                    &text[run_start..offset],
                    run_start as u32,
                    previous_face,
                    pixels_per_em,
                    &mut glyphs,
                )?;
                run_start = offset;
            }
            run_face = Some(face);
        }
        self.shape_run(
            &text[run_start..],
            run_start as u32,
            run_face.unwrap_or(self.primary_face),
            pixels_per_em,
            &mut glyphs,
        )?;
        Ok(ShapedText { glyphs })
    }

    pub(super) fn rasterize_glyph(
        &mut self,
        glyph: &ShapedGlyph,
        pixels_per_em: f32,
    ) -> Result<GlyphBitmap, FontProcessingError> {
        validate_pixels_per_em(pixels_per_em)?;
        let key = GlyphCacheKey {
            face_id: glyph.face_id,
            glyph_id: glyph.glyph_id,
            pixels_per_em: pixels_per_em.to_bits(),
        };
        if let Some(bitmap) = self.glyph_cache.get(key) {
            return Ok(bitmap);
        }
        let scale_context = &mut self.scale_context;
        let bitmap = self
            .database
            .with_face_data(glyph.face_id, |data, index| {
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
            .ok_or(FontProcessingError::FaceDataUnavailable)??;
        self.glyph_cache.insert(key, bitmap.clone());
        Ok(bitmap)
    }

    fn face_for_character(&mut self, character: char) -> ID {
        if self.face_supports(self.primary_face, character) {
            return self.primary_face;
        }
        if let Some(face) = self.fallback_by_character.get(&character) {
            return face.unwrap_or(self.primary_face);
        }
        let fallback = self
            .fallback_candidates
            .iter()
            .copied()
            .find(|face| self.face_supports(*face, character));
        self.fallback_by_character.insert(character, fallback);
        fallback.unwrap_or(self.primary_face)
    }

    fn face_supports(&self, face_id: ID, character: char) -> bool {
        self.database
            .with_face_data(face_id, |data, index| {
                FontRef::from_index(data, index as usize)
                    .is_some_and(|font| font.charmap().map(character) != 0)
            })
            .unwrap_or(false)
    }

    fn shape_run(
        &mut self,
        text: &str,
        source_offset: u32,
        face_id: ID,
        pixels_per_em: f32,
        glyphs: &mut Vec<ShapedGlyph>,
    ) -> Result<(), FontProcessingError> {
        let primary_face = self.primary_face;
        let shape_context = &mut self.shape_context;
        self.database
            .with_face_data(face_id, |data, index| {
                let font = FontRef::from_index(data, index as usize)
                    .ok_or(FontProcessingError::InvalidFaceData)?;
                let mut shaper = shape_context.builder(font).size(pixels_per_em).build();
                shaper.add_str(text);
                shaper.shape_with(|cluster| {
                    glyphs.extend(cluster.glyphs.iter().map(|glyph| ShapedGlyph {
                        face_id,
                        is_fallback: face_id != primary_face,
                        glyph_id: glyph.id,
                        advance: glyph.advance,
                        offset_x: glyph.x,
                        offset_y: glyph.y,
                        cluster_start: source_offset + cluster.source.start,
                        cluster_end: source_offset + cluster.source.end,
                    }));
                });
                Ok(())
            })
            .ok_or(FontProcessingError::FaceDataUnavailable)?
    }
}

fn cell_metrics_from_scaled_values(
    max_width: f32,
    average_width: f32,
    ascent: f32,
    descent: f32,
    leading: f32,
    pixels_per_em: f32,
) -> CellMetrics {
    let width = max_width.max(average_width).ceil().max(1.0) as u32;
    // Swash defines both ascent and descent as positive distances from the
    // baseline, so the alignment box spans their sum, not their difference.
    let height = (ascent + descent + leading).ceil().max(1.0) as u32;
    let baseline = ascent.ceil().clamp(1.0, height as f32) as u32;
    CellMetrics {
        width,
        height,
        baseline,
        pixels_per_em: pixels_per_em.to_bits(),
        ascent: ascent.to_bits(),
        descent: descent.to_bits(),
        leading: leading.to_bits(),
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
        DEFAULT_LOGICAL_FONT_SIZE, FontLoadError, FontProcessingError, FontRequest, FontSystem,
        GLYPH_CACHE_CAPACITY, GlyphCacheKey, SHAPED_TEXT_CACHE_CAPACITY,
        cell_metrics_from_scaled_values,
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
        FontSystem::with_primary_face(database, primary_face, GLYPH_CACHE_CAPACITY).unwrap()
    }

    fn fallback_system(cache_capacity: usize) -> FontSystem {
        let mut database = fontdb::Database::new();
        database.load_font_data(TEST_FONT.to_vec());
        let primary_face = database.push_face_info(FaceInfo {
            id: fontdb::ID::dummy(),
            source: Source::Binary(Arc::new(vec![1, 2, 3])),
            index: 0,
            families: vec![("Missing Glyphs".to_owned(), Language::English_UnitedStates)],
            post_script_name: "AAA Missing Glyphs".to_owned(),
            style: Style::Normal,
            weight: Weight::NORMAL,
            stretch: Stretch::Normal,
            monospaced: true,
        });
        FontSystem::with_primary_face(database, primary_face, cache_capacity).unwrap()
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
    fn shape_cache_reuses_text_and_invalidates_on_font_scale_change() {
        let mut system = fixture_system();
        assert!(!system.shape_text_cached("ABC", 16.0).unwrap().1);
        assert!(system.shape_text_cached("ABC", 16.0).unwrap().1);
        assert!(!system.shape_text_cached("ABC", 17.0).unwrap().1);
        assert_eq!(system.shaped_cache.len(), 1);
    }

    #[test]
    fn shape_cache_evicts_least_recently_used_text() {
        let mut system = fixture_system();
        for index in 0..SHAPED_TEXT_CACHE_CAPACITY {
            system.shape_text_cached(&format!("{index}"), 16.0).unwrap();
        }
        assert!(system.shape_text_cached("0", 16.0).unwrap().1);
        system.shape_text_cached("new", 16.0).unwrap();
        assert_eq!(system.shaped_cache.len(), SHAPED_TEXT_CACHE_CAPACITY);
        assert!(system.shape_text_cached("0", 16.0).unwrap().1);
        assert!(!system.shape_text_cached("1", 16.0).unwrap().1);
    }

    #[test]
    fn cell_metrics_change_with_dpi_but_not_window_size() {
        let system = fixture_system();
        let one_x = system.cell_metrics(1.0).unwrap();
        let two_x = system.cell_metrics(2.0).unwrap();

        assert!(one_x.width() > 0 && one_x.height() > 0);
        assert!(one_x.baseline() > 0 && one_x.baseline() <= one_x.height());
        assert!(two_x.baseline() > 0 && two_x.baseline() <= two_x.height());
        assert!(two_x.width() >= one_x.width() && two_x.height() >= one_x.height());
        assert_eq!(one_x.pixels_per_em(), DEFAULT_LOGICAL_FONT_SIZE);
        assert_eq!(two_x.pixels_per_em(), DEFAULT_LOGICAL_FONT_SIZE * 2.0);
    }

    #[test]
    fn cell_height_includes_the_positive_descent_below_the_baseline() {
        let metrics = cell_metrics_from_scaled_values(8.0, 8.0, 12.0, 4.0, 0.0, 16.0);

        assert_eq!(metrics.height(), 16);
        assert_eq!(metrics.baseline(), 12);
        assert_eq!(metrics.ascent(), 12.0);
        assert_eq!(metrics.descent(), 4.0);
        assert_eq!(metrics.leading(), 0.0);
    }

    #[test]
    #[ignore = "requires selected native control fonts"]
    fn native_monospace_control_fonts_report_complete_line_metrics() {
        let families = ["Cascadia Mono", "Consolas", "JetBrains Mono Nerd Font"];
        let metrics = families
            .iter()
            .filter_map(|family| {
                FontSystem::load_system(FontRequest::Family((*family).to_owned()))
                    .ok()
                    .and_then(|font| font.cell_metrics(1.0).ok())
                    .map(|metrics| (*family, metrics))
            })
            .collect::<Vec<_>>();

        assert!(
            !metrics.is_empty(),
            "at least one installed monospace control font is required"
        );

        for (family, metrics) in metrics {
            eprintln!(
                "{family}: width={} height={} ascent={} descent={} baseline={}",
                metrics.width(),
                metrics.height(),
                metrics.ascent(),
                metrics.descent(),
                metrics.baseline()
            );
            assert!(metrics.ascent() > 0.0);
            assert!(metrics.descent() > 0.0);
            assert!(metrics.height() >= metrics.baseline());
        }
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

    #[test]
    fn chooses_the_primary_face_when_it_supports_text() {
        let mut system = fixture_system();

        let shaped = system.shape_text("A", 16.0).unwrap();

        assert!(!shaped.glyphs()[0].uses_fallback());
    }

    #[test]
    fn uses_a_stable_fallback_when_the_primary_face_cannot_map_a_character() {
        let mut system = fallback_system(GLYPH_CACHE_CAPACITY);

        let first = system.shape_text("A", 16.0).unwrap();
        let second = system.shape_text("A", 16.0).unwrap();

        assert!(first.glyphs()[0].uses_fallback());
        assert_eq!(first.glyphs(), second.glyphs());
        assert_eq!(system.fallback_by_character.len(), 1);
    }

    #[test]
    fn glyph_cache_reuses_entries_and_evicts_the_least_recently_used() {
        let mut system = fallback_system(2);
        let shaped = system.shape_text("ABC", 16.0).unwrap();

        system.rasterize_glyph(&shaped.glyphs()[0], 16.0).unwrap();
        system.rasterize_glyph(&shaped.glyphs()[1], 16.0).unwrap();
        let first_key = GlyphCacheKey {
            face_id: shaped.glyphs()[0].face_id,
            glyph_id: shaped.glyphs()[0].glyph_id,
            pixels_per_em: 16.0f32.to_bits(),
        };
        system.rasterize_glyph(&shaped.glyphs()[0], 16.0).unwrap();
        system.rasterize_glyph(&shaped.glyphs()[2], 16.0).unwrap();

        assert_eq!(system.glyph_cache.entries.len(), 2);
        assert!(system.glyph_cache.entries.contains_key(&first_key));
        assert_eq!(system.glyph_cache.lru.len(), 2);
    }

    #[test]
    fn cache_keys_distinguish_font_size() {
        let mut system = fixture_system();
        let glyph = system.shape_text("A", 16.0).unwrap().glyphs()[0].clone();

        system.rasterize_glyph(&glyph, 16.0).unwrap();
        system.rasterize_glyph(&glyph, 24.0).unwrap();

        assert_eq!(system.glyph_cache.entries.len(), 2);
    }

    #[test]
    #[ignore = "requires the native system font database"]
    fn native_system_font_shapes_and_rasterizes_the_visual_smoke_wide_character() {
        let mut system = FontSystem::load_system(FontRequest::default()).unwrap();

        let shaped = system.shape_text("界", 32.0).unwrap();

        assert!(!shaped.glyphs().is_empty());
        assert!(shaped.glyphs().iter().any(|glyph| glyph.uses_fallback()));
        assert!(shaped.glyphs().iter().any(|glyph| {
            system
                .rasterize_glyph(glyph, 32.0)
                .is_ok_and(|bitmap| bitmap.width() > 0 && bitmap.height() > 0)
        }));
    }
}
