//! Renderer-owned font discovery, face selection, shaping, and rasterization.
//!
//! Native Bold companions and Unicode fallback share the retained database;
//! terminal cell metrics always come from the configured primary face.

use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::fmt;

#[cfg(any(target_os = "linux", test))]
use fontdb::Source as FontSource;
use fontdb::{Database, Family, ID, Query, Weight};
#[cfg(any(target_os = "linux", test))]
use std::path::Path;
#[cfg(target_os = "linux")]
use std::process::Command;
use swash::{
    FontRef,
    scale::{Render, ScaleContext, Source, StrikeWith, image::Content},
    shape::ShapeContext,
    zeno::{Format, Transform},
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

    /// Adds symmetric vertical spacing to UI rows without changing glyph size.
    pub(crate) fn with_vertical_padding(mut self, padding: u32) -> Self {
        self.height += padding * 2;
        self.baseline += padding;
        self
    }
}

/// Positioned glyph produced from the renderer's selected font face.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapedGlyph {
    face_id: ID,
    weight: Option<u32>,
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
        format!("{}:{:?}", self.face_id, self.weight)
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

/// CPU-side bitmap and placement for one rasterized glyph.
/// Masks have one coverage byte per pixel; color has straight-alpha sRGB RGBA.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphBitmap {
    glyph_id: u16,
    is_color: bool,
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    bearing_x: i32,
    bearing_y: i32,
    advance: f32,
}

impl GlyphBitmap {
    pub fn is_color(&self) -> bool {
        self.is_color
    }

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
                    "glyph {glyph_id} has no supported outline or bitmap"
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
    weight: Option<u32>,
    glyph_id: u16,
    pixels_per_em: u32,
    bold: bool,
    italic: bool,
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
    bold_companions: HashMap<ID, Option<ID>>,
    bold_intensity: f32,
    bold_instances: HashMap<ID, (ID, Option<u32>)>,
    shaped_cache: HashMap<(String, bool), ShapedText>,
    shaped_lru: VecDeque<(String, bool)>,
    shaped_pixels_per_em: Option<u32>,
}

impl FontSystem {
    /// Consuming initialization prevents changing weight under retained GPU frames.
    pub(super) fn with_bold_intensity(mut self, intensity: f32) -> Self {
        self.bold_intensity = intensity;
        self.bold_instances.clear();
        self.shaped_cache.clear();
        self.shaped_lru.clear();
        self.glyph_cache = GlyphCache::new(self.glyph_cache.capacity);
        self
    }
    pub(super) fn cell_metrics(
        &self,
        scale_factor: f64,
        logical_font_size: f32,
    ) -> Result<CellMetrics, FontProcessingError> {
        let pixels_per_em = logical_font_size * scale_factor as f32;
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
        let database_stage = crate::startup_stage("font-database-create");
        let mut database = Database::new();
        drop(database_stage);
        let discovery_stage = crate::startup_stage("system-font-discovery");
        database.load_system_fonts();
        drop(discovery_stage);
        #[cfg(target_os = "linux")]
        if matches!(&request, FontRequest::SystemMonospace) {
            if let Some(id) = fontconfig_monospace_face(&mut database) {
                return Self::with_primary_face(database, id, GLYPH_CACHE_CAPACITY);
            }
            let primary_face = select_discovered_monospace_face(&database)
                .ok_or(FontLoadError::NoSystemMonospaceFace)?;
            return Self::with_primary_face(database, primary_face, GLYPH_CACHE_CAPACITY);
        }
        Self::from_database(database, request)
    }

    fn from_database(database: Database, request: FontRequest) -> Result<Self, FontLoadError> {
        let resolution_stage = crate::startup_stage("font-family-resolution");
        let primary_face = select_primary_face(&database, &request)?;
        drop(resolution_stage);
        Self::with_primary_face(database, primary_face, GLYPH_CACHE_CAPACITY)
    }

    fn with_primary_face(
        database: Database,
        primary_face: ID,
        cache_capacity: usize,
    ) -> Result<Self, FontLoadError> {
        let fallback_stage = crate::startup_stage("fallback-candidate-ordering");
        let collection_stage = crate::startup_stage("fallback-candidate-collection");
        let mut fallback_candidates: Vec<_> = database
            .faces()
            .filter(|face| face.id != primary_face)
            .map(|face| face.id)
            .collect();
        drop(collection_stage);
        let sorting_stage = crate::startup_stage("fallback-candidate-sort");
        fallback_candidates.sort_by_key(|id| {
            let face = database.face(*id).unwrap();
            (face.post_script_name.clone(), face.id.to_string())
        });
        drop(sorting_stage);
        drop(fallback_stage);
        let _cache_stage = crate::startup_stage("font-cache-setup");
        Ok(Self {
            database,
            primary_face,
            shape_context: ShapeContext::new(),
            scale_context: ScaleContext::new(),
            fallback_candidates,
            fallback_by_character: HashMap::new(),
            glyph_cache: GlyphCache::new(cache_capacity),
            bold_companions: HashMap::new(),
            bold_intensity: 1.0,
            bold_instances: HashMap::new(),
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
        self.shape_text_cached_with_bold(text, pixels_per_em, false)
    }

    pub(super) fn shape_text_cached_with_bold(
        &mut self,
        text: &str,
        pixels_per_em: f32,
        bold: bool,
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
        let key = (text.to_owned(), bold);
        if let Some(shaped) = self.shaped_cache.get(&key).cloned() {
            if let Some(position) = self
                .shaped_lru
                .iter()
                .position(|candidate| candidate == &key)
            {
                let key = self.shaped_lru.remove(position).expect("known cache key");
                self.shaped_lru.push_back(key);
            }
            return Ok((shaped, true));
        }
        let shaped = self.shape_text_uncached(text, pixels_per_em, bold)?;
        if self.shaped_cache.len() == SHAPED_TEXT_CACHE_CAPACITY
            && let Some(oldest) = self.shaped_lru.pop_front()
        {
            self.shaped_cache.remove(&oldest);
        }
        self.shaped_cache.insert(key.clone(), shaped.clone());
        self.shaped_lru.push_back(key);
        Ok((shaped, false))
    }

    fn shape_text_uncached(
        &mut self,
        text: &str,
        pixels_per_em: f32,
        bold: bool,
    ) -> Result<ShapedText, FontProcessingError> {
        let mut glyphs = Vec::new();
        let mut run_start = 0;
        let mut run_face: Option<(ID, Option<u32>)> = None;
        for (offset, character) in text.char_indices() {
            let regular = self.face_for_character(character);
            let instance = if bold {
                let instance = self.bold_instance(regular);
                if self.face_supports(instance.0, character) {
                    instance
                } else {
                    (
                        self.bold_companion(regular)
                            .filter(|face| self.face_supports(*face, character))
                            .unwrap_or(regular),
                        None,
                    )
                }
            } else {
                (regular, None)
            };
            if let Some(previous_face) = run_face
                && instance != previous_face
            {
                self.shape_run(
                    &text[run_start..offset],
                    run_start as u32,
                    previous_face.0,
                    previous_face.1,
                    pixels_per_em,
                    &mut glyphs,
                )?;
                run_start = offset;
            }
            run_face = Some(instance);
        }
        self.shape_run(
            &text[run_start..],
            run_start as u32,
            run_face.unwrap_or((self.primary_face, None)).0,
            run_face.unwrap_or((self.primary_face, None)).1,
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
        self.rasterize_glyph_with_bold(glyph, pixels_per_em, false)
    }

    pub(super) fn rasterize_glyph_with_bold(
        &mut self,
        glyph: &ShapedGlyph,
        pixels_per_em: f32,
        bold: bool,
    ) -> Result<GlyphBitmap, FontProcessingError> {
        self.rasterize_glyph_with_style(glyph, pixels_per_em, bold, false)
    }

    pub(super) fn rasterize_glyph_with_style(
        &mut self,
        glyph: &ShapedGlyph,
        pixels_per_em: f32,
        bold: bool,
        italic: bool,
    ) -> Result<GlyphBitmap, FontProcessingError> {
        validate_pixels_per_em(pixels_per_em)?;
        let key = GlyphCacheKey {
            face_id: glyph.face_id,
            weight: glyph.weight,
            glyph_id: glyph.glyph_id,
            pixels_per_em: pixels_per_em.to_bits(),
            bold,
            italic,
        };
        if let Some(bitmap) = self.glyph_cache.get(key) {
            return Ok(bitmap);
        }
        let synthesize = bold && glyph.weight.is_none() && !self.face_is_bold(glyph.face_id);
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
                    // Swash retains builder coordinates in its context. An empty
                    // variations iterator does not reset a previous weight.
                    .normalized_coords(font.variations().normalized_coords(
                        glyph.weight.map(|weight| ("wght", f32::from_bits(weight))),
                    ))
                    .build();
                let mut renderer = Render::new(&[
                    Source::ColorOutline(0),
                    Source::ColorBitmap(StrikeWith::BestFit),
                    Source::Outline,
                    Source::Bitmap(StrikeWith::BestFit),
                ]);
                renderer.format(Format::Alpha);
                if italic {
                    // Shear outlines around the baseline, retaining fixed cell
                    // advances and the existing native Bold/variation selection.
                    renderer.transform(Some(Transform::new(1.0, 0.0, 0.2, 1.0, 0.0, 0.0)));
                }
                if synthesize {
                    // A restrained fallback keeps counters open at small sizes.
                    // Native Bold outlines must never receive additional weight.
                    renderer.embolden(pixels_per_em / 48.0);
                }
                let mut image = renderer
                    .render(&mut scaler, glyph.glyph_id)
                    .ok_or(FontProcessingError::GlyphNotRasterizable(glyph.glyph_id))?;
                // Swash's COLR layer compositor produces premultiplied bytes;
                // decoded color bitmaps (including Noto's PNGs) are straight.
                if matches!(image.source, Source::ColorOutline(_)) {
                    for pixel in image.data.as_chunks_mut::<4>().0 {
                        let alpha = u32::from(pixel[3]);
                        for channel in &mut pixel[..3] {
                            *channel = (u32::from(*channel) * 255 + alpha / 2)
                                .checked_div(alpha)
                                .unwrap_or(0)
                                .min(255) as u8;
                        }
                    }
                }
                Ok(GlyphBitmap {
                    glyph_id: glyph.glyph_id,
                    is_color: image.content == Content::Color,
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

    fn face_is_bold(&self, face: ID) -> bool {
        self.database
            .face(face)
            .is_some_and(|info| info.weight >= Weight::SEMIBOLD)
    }

    fn bold_instance(&mut self, regular: ID) -> (ID, Option<u32>) {
        let native = self.bold_companion(regular);
        let unchanged = (native.unwrap_or(regular), None);
        if self.bold_intensity == 1.0 {
            return unchanged;
        }
        if let Some(instance) = self.bold_instances.get(&regular) {
            return *instance;
        }
        let reference = native
            .and_then(|id| self.database.face(id))
            .map_or(700.0, |face| f32::from(face.weight.0));
        let target = reference * self.bold_intensity;
        let variable_weight = self
            .database
            .with_face_data(unchanged.0, |data, index| {
                let font = FontRef::from_index(data, index as usize)?;
                let axis = font
                    .variations()
                    .find(|axis| axis.tag() == swash::tag_from_bytes(b"wght"))?;
                (axis.min_value().is_finite()
                    && axis.max_value().is_finite()
                    && axis.min_value() < axis.max_value())
                .then(|| target.clamp(axis.min_value(), axis.max_value()).to_bits())
            })
            .flatten();
        let instance = if let Some(weight) = variable_weight {
            (unchanged.0, Some(weight))
        } else if native.is_some() {
            let info = self.database.face(regular).expect("resolved face");
            let selected = self
                .database
                .faces()
                .filter(|face| {
                    face.weight >= Weight::SEMIBOLD
                        && face.style == info.style
                        && face.stretch == info.stretch
                        && (!info.monospaced || face.monospaced)
                        && face.families.iter().any(|(name, _)| {
                            info.families.iter().any(|(original, _)| name == original)
                        })
                        && self
                            .database
                            .with_face_data(face.id, |data, index| {
                                FontRef::from_index(data, index as usize).is_some()
                            })
                            .unwrap_or(false)
                })
                .min_by_key(|face| {
                    (
                        (f32::from(face.weight.0) - target).abs().to_bits(),
                        (f32::from(face.weight.0) - reference).abs().to_bits(),
                        face.id.to_string(),
                    )
                })
                .map(|face| face.id)
                .unwrap_or(unchanged.0);
            if selected == unchanged.0 {
                eprintln!(
                    "font.bold_intensity: face {} has no closer supported native weight; keeping native Bold",
                    unchanged.0
                );
            }
            (selected, None)
        } else {
            eprintln!(
                "font.bold_intensity: face {} has no adjustable weight; keeping existing bold fallback",
                regular
            );
            unchanged
        };
        self.bold_instances.insert(regular, instance);
        instance
    }

    fn bold_companion(&mut self, regular: ID) -> Option<ID> {
        if self
            .database
            .face(regular)
            .is_some_and(|info| info.weight >= Weight::BOLD)
        {
            return Some(regular);
        }
        if let Some(face) = self.bold_companions.get(&regular) {
            return *face;
        }
        let selected = self.database.face(regular).and_then(|info| {
            let families: Vec<_> = info
                .families
                .iter()
                .map(|(name, _)| Family::Name(name))
                .collect();
            let candidate = self.database.query(&Query {
                families: &families,
                weight: Weight::BOLD,
                stretch: info.stretch,
                style: info.style,
            })?;
            let bold = self.database.face(candidate)?;
            // fontdb can return a nearest regular face: reject that substitute.
            if bold.weight < Weight::SEMIBOLD
                || bold.style != info.style
                || bold.stretch != info.stretch
                || (info.monospaced && !bold.monospaced)
                || !bold.families.iter().any(|(name, _)| {
                    info.families
                        .iter()
                        .any(|(regular_name, _)| regular_name == name)
                })
            {
                return None;
            }
            self.database
                .with_face_data(candidate, |data, index| {
                    FontRef::from_index(data, index as usize).is_some()
                })
                .filter(|valid| *valid)
                .map(|_| candidate)
        });
        self.bold_companions.insert(regular, selected);
        selected
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
        weight: Option<u32>,
        pixels_per_em: f32,
        glyphs: &mut Vec<ShapedGlyph>,
    ) -> Result<(), FontProcessingError> {
        let primary_face = self.primary_face;
        let primary_bold = self.bold_instance(primary_face).0;
        let primary_native_bold = self.bold_companion(primary_face);
        let shape_context = &mut self.shape_context;
        self.database
            .with_face_data(face_id, |data, index| {
                let font = FontRef::from_index(data, index as usize)
                    .ok_or(FontProcessingError::InvalidFaceData)?;
                let mut shaper =
                    shape_context
                        .builder(font)
                        .size(pixels_per_em)
                        .normalized_coords(font.variations().normalized_coords(
                            weight.map(|weight| ("wght", f32::from_bits(weight))),
                        ))
                        .build();
                shaper.add_str(text);
                shaper.shape_with(|cluster| {
                    glyphs.extend(cluster.glyphs.iter().map(|glyph| ShapedGlyph {
                        face_id,
                        weight,
                        is_fallback: face_id != primary_face
                            && face_id != primary_bold
                            && Some(face_id) != primary_native_bold,
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
    let query_stage = crate::startup_stage(match request {
        FontRequest::SystemMonospace => "font-generic-family-query",
        FontRequest::Family(_) => "font-configured-family-query",
    });
    let id = database.query(&Query {
        families: &[family],
        ..Default::default()
    });
    drop(query_stage);
    let _validation_stage = crate::startup_stage("font-primary-monospace-validation");
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

// fontdb's generic-family query only checks one configured alias. It does not
// perform fontconfig's substitutions, which can resolve "monospace" to a face
// with a completely different family name.
#[cfg(target_os = "linux")]
fn fontconfig_monospace_face(database: &mut Database) -> Option<ID> {
    let output = Command::new("fc-match")
        .args(["-f", "%{file}\n%{index}\n%{spacing}\n", "monospace"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let result = String::from_utf8(output.stdout).ok()?;
    resolve_fontconfig_face(database, &result)
}

#[cfg(any(target_os = "linux", test))]
fn resolve_fontconfig_face(database: &mut Database, result: &str) -> Option<ID> {
    let mut fields = result.lines();
    let path = Path::new(fields.next()?);
    let index = fields.next()?.parse::<u32>().ok()?;
    let spacing = fields.next()?.parse::<u32>().ok()?;
    if !path.is_absolute() || spacing < 100 {
        return None;
    }

    // fontdb's directory scan can miss a fontconfig-visible file. Loading the
    // resolved file also keeps its bytes in the same database used for shaping.
    if !database
        .faces()
        .any(|face| face.index == index && face_source_is_path(&face.source, path))
    {
        database.load_font_file(path).ok()?;
    }
    database
        .faces()
        .find(|face| face.index == index && face_source_is_path(&face.source, path))
        .and_then(|face| usable_face(database, face.id).then_some(face.id))
}

#[cfg(any(target_os = "linux", test))]
fn face_source_is_path(source: &FontSource, path: &Path) -> bool {
    match source {
        FontSource::File(file) | FontSource::SharedFile(file, _) => file == path,
        FontSource::Binary(_) => false,
    }
}

#[cfg(any(target_os = "linux", test))]
fn usable_face(database: &Database, id: ID) -> bool {
    database
        .with_face_data(id, |data, index| {
            FontRef::from_index(data, index as usize).is_some()
        })
        .unwrap_or(false)
}

#[cfg(any(target_os = "linux", test))]
fn select_discovered_monospace_face(database: &Database) -> Option<ID> {
    let preferred = database.query(&Query {
        families: &[Family::Monospace],
        ..Default::default()
    });
    preferred
        .filter(|id| {
            database.face(*id).is_some_and(|face| face.monospaced) && usable_face(database, *id)
        })
        .or_else(|| {
            let mut candidates: Vec<_> = database.faces().filter(|face| face.monospaced).collect();
            candidates.sort_by(|left, right| {
                left.families
                    .first()
                    .map(|family| &family.0)
                    .cmp(&right.families.first().map(|family| &family.0))
                    .then_with(|| left.post_script_name.cmp(&right.post_script_name))
            });
            candidates
                .into_iter()
                .find(|face| usable_face(database, face.id))
                .map(|face| face.id)
        })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fontdb::{FaceInfo, Language, Source, Stretch, Style, Weight};

    use super::{
        DEFAULT_LOGICAL_FONT_SIZE, FontLoadError, FontProcessingError, FontRequest, FontSystem,
        GLYPH_CACHE_CAPACITY, GlyphCacheKey, SHAPED_TEXT_CACHE_CAPACITY,
        cell_metrics_from_scaled_values, resolve_fontconfig_face, select_discovered_monospace_face,
        select_primary_face,
    };

    const TEST_FONT: &[u8] = include_bytes!("../tests/fixtures/Tuffy.ttf");

    // Add one COLRv0 layer to the existing licensed outline fixture. This
    // exercises color rasterization on CI without relying on installed fonts.
    fn color_fixture_system() -> FontSystem {
        let glyph = swash::FontRef::from_index(TEST_FONT, 0)
            .unwrap()
            .charmap()
            .map('A');
        let mut colr = vec![0, 0, 0, 1, 0, 0, 0, 14, 0, 0, 0, 20, 0, 1];
        colr.extend_from_slice(&glyph.to_be_bytes());
        colr.extend_from_slice(&[0, 0, 0, 1]);
        colr.extend_from_slice(&glyph.to_be_bytes());
        colr.extend_from_slice(&[0, 0]);
        let cpal = vec![0, 0, 0, 1, 0, 1, 0, 1, 0, 0, 0, 14, 0, 0, 240, 160, 80, 255];
        let count = u16::from_be_bytes(TEST_FONT[4..6].try_into().unwrap()) as usize;
        let directory_end = 12 + count * 16;
        let mut bytes = TEST_FONT[..directory_end].to_vec();
        bytes.resize(directory_end + 32, 0);
        bytes.extend_from_slice(&TEST_FONT[directory_end..]);
        bytes[4..6].copy_from_slice(&((count + 2) as u16).to_be_bytes());
        for offset in (12..directory_end).step_by(16) {
            let old = u32::from_be_bytes(bytes[offset + 8..offset + 12].try_into().unwrap());
            bytes[offset + 8..offset + 12].copy_from_slice(&(old + 32).to_be_bytes());
        }
        for (index, (tag, table)) in [(b"COLR", colr), (b"CPAL", cpal)].into_iter().enumerate() {
            while !bytes.len().is_multiple_of(4) {
                bytes.push(0);
            }
            let record = directory_end + index * 16;
            bytes[record..record + 4].copy_from_slice(tag);
            let start = bytes.len() as u32;
            bytes[record + 8..record + 12].copy_from_slice(&start.to_be_bytes());
            bytes[record + 12..record + 16].copy_from_slice(&(table.len() as u32).to_be_bytes());
            bytes.extend_from_slice(&table);
        }
        // OpenType table directories are sorted by tag for binary search.
        let mut records = bytes[12..directory_end + 32]
            .as_chunks::<16>()
            .0
            .iter()
            .map(|record| record.to_vec())
            .collect::<Vec<_>>();
        records.sort_by(|a, b| a[..4].cmp(&b[..4]));
        for (dest, record) in bytes[12..directory_end + 32]
            .as_chunks_mut::<16>()
            .0
            .iter_mut()
            .zip(records)
        {
            dest.copy_from_slice(&record);
        }
        let mut database = fontdb::Database::new();
        database.load_font_data(bytes);
        let face = database.faces().next().unwrap().id;
        FontSystem::with_primary_face(database, face, GLYPH_CACHE_CAPACITY).unwrap()
    }

    #[test]
    fn color_glyphs_preserve_rgba_and_cache_identity_without_changing_masks() {
        let mut system = color_fixture_system();
        let mut reference = fixture_system();
        for size in [16.0, 20.0, 32.0] {
            let shaped = system.shape_text("AB", size).unwrap();
            let color = system.rasterize_glyph(&shaped.glyphs()[0], size).unwrap();
            assert!(color.is_color());
            assert_eq!(
                color.pixels().len(),
                (color.width() * color.height() * 4) as usize
            );
            assert!(
                color
                    .pixels()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[3] > 200 && p[2] > p[1] && p[1] > p[0])
            );
            assert_eq!(
                color,
                system.rasterize_glyph(&shaped.glyphs()[0], size).unwrap()
            );
            let normal = reference.shape_text("B", size).unwrap();
            let mask = system.rasterize_glyph(&shaped.glyphs()[1], size).unwrap();
            assert!(!mask.is_color());
            assert_eq!(
                mask.pixels(),
                reference
                    .rasterize_glyph(&normal.glyphs()[0], size)
                    .unwrap()
                    .pixels()
            );
        }
        assert_eq!(system.glyph_cache.entries.len(), 6);
    }

    fn variable_system(intensity: f32) -> FontSystem {
        let mut database = fontdb::Database::new();
        database.load_font_data(
            include_bytes!("../tests/fixtures/JetBrainsMono-variable.ttf").to_vec(),
        );
        FontSystem::from_database(database, FontRequest::Family("JetBrains Mono".into()))
            .unwrap()
            .with_bold_intensity(intensity)
    }

    #[test]
    fn italic_rasterization_preserves_advances_and_separates_style_size_and_weight() {
        for mut system in [fixture_system(), variable_system(1.25)] {
            for size in [12.0, 20.0, 32.0] {
                for bold in [false, true] {
                    let shaped = system
                        .shape_text_cached_with_bold("Ae\u{301}", size, bold)
                        .unwrap()
                        .0;
                    let glyph = &shaped.glyphs()[0];
                    let upright = system
                        .rasterize_glyph_with_style(glyph, size, bold, false)
                        .unwrap();
                    let italic = system
                        .rasterize_glyph_with_style(glyph, size, bold, true)
                        .unwrap();
                    assert_ne!(italic.pixels(), upright.pixels());
                    assert_eq!(italic.advance(), upright.advance());
                    assert_eq!(
                        system
                            .rasterize_glyph_with_style(glyph, size, bold, false)
                            .unwrap(),
                        upright
                    );
                    assert_eq!(
                        system
                            .rasterize_glyph_with_style(glyph, size, bold, true)
                            .unwrap(),
                        italic
                    );
                    for mark in &shaped.glyphs()[1..] {
                        let bitmap = system
                            .rasterize_glyph_with_style(mark, size, bold, true)
                            .unwrap();
                        assert_eq!(bitmap.advance(), mark.advance());
                    }
                }
            }
        }
    }

    #[test]
    fn intensity_preserves_default_and_unsupported_font_bitmaps() {
        for intensity in [0.75, 1.0, 1.25] {
            let mut reference = fixture_system();
            let mut candidate = fixture_system().with_bold_intensity(intensity);
            for size in [12.0, 16.0, 20.0, 32.0] {
                for bold in [false, true] {
                    let reference_text = reference
                        .shape_text_cached_with_bold("Ae\u{301}", size, bold)
                        .unwrap()
                        .0;
                    let candidate_text = candidate
                        .shape_text_cached_with_bold("Ae\u{301}", size, bold)
                        .unwrap()
                        .0;
                    assert_eq!(candidate_text, reference_text);
                    for (a, b) in reference_text.glyphs().iter().zip(candidate_text.glyphs()) {
                        assert_eq!(
                            reference.rasterize_glyph_with_bold(a, size, bold).unwrap(),
                            candidate.rasterize_glyph_with_bold(b, size, bold).unwrap()
                        );
                    }
                }
            }
            assert_eq!(
                candidate.bold_instances.len(),
                usize::from(intensity != 1.0)
            );
        }
        let (mut reference, _, _) = paired_fixture_system();
        let (candidate, _, _) = paired_fixture_system();
        let mut candidate = candidate.with_bold_intensity(1.0);
        let a = reference
            .shape_text_cached_with_bold("A", 16.0, true)
            .unwrap()
            .0;
        let b = candidate
            .shape_text_cached_with_bold("A", 16.0, true)
            .unwrap()
            .0;
        assert_eq!(
            reference
                .rasterize_glyph_with_bold(&a.glyphs()[0], 16.0, true)
                .unwrap(),
            candidate
                .rasterize_glyph_with_bold(&b.glyphs()[0], 16.0, true)
                .unwrap()
        );
    }

    #[test]
    fn variable_intensity_clamps_separates_caches_and_preserves_grid() {
        let mut light = variable_system(0.75);
        let mut heavy = variable_system(1.25);
        for scale in [1.0, 1.25, 2.0] {
            for logical_size in [12.0, 16.0, 20.0] {
                let size = scale as f32 * logical_size;
                assert_eq!(
                    light.cell_metrics(scale, logical_size).unwrap(),
                    heavy.cell_metrics(scale, logical_size).unwrap()
                );
                let normal = light.shape_text("Ag0@é e\u{301}", size).unwrap();
                let (a, hit) = light
                    .shape_text_cached_with_bold("Ag0@é e\u{301}", size, true)
                    .unwrap();
                assert!(!hit);
                let b = heavy
                    .shape_text_cached_with_bold("Ag0@é e\u{301}", size, true)
                    .unwrap()
                    .0;
                assert_eq!(a.glyphs().len(), b.glyphs().len());
                for ((a, b), normal) in a.glyphs().iter().zip(b.glyphs()).zip(normal.glyphs()) {
                    assert_eq!(a.weight, Some(525.0f32.to_bits()));
                    assert_eq!(b.weight, Some(800.0f32.to_bits()));
                    assert_ne!(a.face_cache_identity(), b.face_cache_identity());
                    assert_eq!(a.advance, b.advance);
                    assert_eq!(a.advance, normal.advance);
                    assert_eq!(a.cluster_range(), normal.cluster_range());
                    assert!(!a.uses_fallback());
                    let bitmap = light.rasterize_glyph_with_bold(a, size, true).unwrap();
                    assert_eq!(
                        bitmap,
                        light.rasterize_glyph(a, size).unwrap(),
                        "native variable outline must never be synthesized"
                    );
                    assert_eq!(
                        bitmap,
                        light.rasterize_glyph_with_bold(a, size, true).unwrap()
                    );
                }
                let first = &a.glyphs()[0];
                assert_ne!(
                    light.rasterize_glyph_with_bold(first, size, true).unwrap(),
                    heavy
                        .rasterize_glyph_with_bold(&b.glyphs()[0], size, true)
                        .unwrap()
                );
                assert_eq!(
                    light
                        .shape_text_cached_with_bold("Ag0@é e\u{301}", size, true)
                        .unwrap(),
                    (a, true)
                );
                assert_eq!(light.shape_text("Ag0@é e\u{301}", size).unwrap(), normal);
            }
        }
        // Even the same font system must not reuse a shape/bitmap from an old instance.
        let old = light
            .shape_text_cached_with_bold("A", 16.0, true)
            .unwrap()
            .0;
        let old_bitmap = light
            .rasterize_glyph_with_bold(&old.glyphs()[0], 16.0, true)
            .unwrap();
        let mut changed = light.with_bold_intensity(1.25);
        let (new, hit) = changed
            .shape_text_cached_with_bold("A", 16.0, true)
            .unwrap();
        assert!(!hit);
        assert_ne!(
            old_bitmap,
            changed
                .rasterize_glyph_with_bold(&new.glyphs()[0], 16.0, true)
                .unwrap()
        );
        assert_eq!(
            old_bitmap,
            changed
                .rasterize_glyph_with_bold(&old.glyphs()[0], 16.0, true)
                .unwrap()
        );
        assert_eq!(changed.glyph_cache.entries.len(), 2);
    }

    #[test]
    fn variable_regular_bitmaps_after_bold_match_fresh_rasterization() {
        for intensity in [0.75, 1.0, 1.25] {
            let mut system = variable_system(intensity);
            let bold = system
                .shape_text_cached_with_bold("Ag0@é", 16.0, true)
                .unwrap()
                .0;
            for glyph in bold.glyphs() {
                system.rasterize_glyph_with_bold(glyph, 16.0, true).unwrap();
            }
            let normal = system.shape_text("RED GREEN BLUE", 16.0).unwrap();
            let mut fresh = variable_system(1.0);
            let reference = fresh.shape_text("RED GREEN BLUE", 16.0).unwrap();
            for (a, b) in normal.glyphs().iter().zip(reference.glyphs()) {
                assert_eq!(
                    system.rasterize_glyph(a, 16.0).unwrap(),
                    fresh.rasterize_glyph(b, 16.0).unwrap(),
                    "{intensity}"
                );
            }
        }
    }

    #[test]
    fn static_intensity_uses_compatible_faces_and_preserves_default_on_ties() {
        let (mut system, regular, bold) = paired_fixture_system();
        let mut face = system.database.face(bold).unwrap().clone();
        face.weight = Weight::SEMIBOLD;
        face.post_script_name = "Paired-Semibold".into();
        let semibold = system.database.push_face_info(face.clone());
        face.weight = Weight::EXTRA_BOLD;
        face.post_script_name = "Paired-ExtraBold".into();
        let extra = system.database.push_face_info(face.clone());
        face.weight = Weight(875);
        face.style = Style::Italic;
        let mismatched = system.database.push_face_info(face.clone());
        face.style = Style::Normal;
        face.source = Source::Binary(Arc::new(vec![0]));
        let invalid = system.database.push_face_info(face);
        for (intensity, expected) in [
            (0.75, semibold),
            (1.0, bold),
            (1.25, extra),
            (650.0 / 700.0, bold),
        ] {
            system = system.with_bold_intensity(intensity);
            assert_eq!(system.bold_instance(regular), (expected, None));
            assert_ne!(expected, mismatched);
            assert_ne!(expected, invalid);
            let shaped = system
                .shape_text_cached_with_bold("Ae\u{301}", 16.0, true)
                .unwrap()
                .0;
            assert!(
                shaped
                    .glyphs()
                    .iter()
                    .all(|g| g.face_id == expected && !g.uses_fallback())
            );
            for glyph in shaped.glyphs() {
                assert_eq!(
                    system.rasterize_glyph_with_bold(glyph, 16.0, true).unwrap(),
                    system.rasterize_glyph(glyph, 16.0).unwrap()
                );
            }
        }
    }

    #[test]
    fn intensity_face_without_character_keeps_original_native_bold_coverage() {
        let (mut system, regular, bold) = paired_fixture_system();
        let mut bytes = TEST_FONT.to_vec();
        let tables = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
        for offset in (12..12 + tables * 16).step_by(16) {
            if &bytes[offset..offset + 4] == b"cmap" {
                bytes[offset..offset + 4].copy_from_slice(b"xxxx");
            }
        }
        let mut face = system.database.face(bold).unwrap().clone();
        face.source = Source::Binary(Arc::new(bytes));
        face.weight = Weight::EXTRA_BOLD;
        let extra = system.database.push_face_info(face);
        system = system.with_bold_intensity(1.25);
        assert_eq!(system.bold_instance(regular).0, extra);
        assert!(!system.face_supports(extra, 'A'));
        let shaped = system
            .shape_text_cached_with_bold("Ae\u{301}", 16.0, true)
            .unwrap()
            .0;
        assert!(
            shaped.glyphs().iter().all(|glyph| glyph.face_id == bold
                && glyph.glyph_id != 0
                && !glyph.uses_fallback())
        );
    }

    #[test]
    #[ignore = "requires installed JetBrainsMono Nerd Font Mono weight faces"]
    fn native_static_intensity_selects_real_weight_faces() {
        for intensity in [0.75, 1.0, 1.25] {
            let mut system =
                FontSystem::load_system(FontRequest::Family("JetBrainsMono Nerd Font Mono".into()))
                    .unwrap()
                    .with_bold_intensity(intensity);
            let instance = system.bold_instance(system.primary_face);
            let expected = if intensity < 1.0 {
                600
            } else if intensity > 1.0 {
                800
            } else {
                700
            };
            assert_eq!(system.database.face(instance.0).unwrap().weight.0, expected);
            for scale in [1.0, 1.25, 2.0] {
                let metrics = system.cell_metrics(scale, 16.0).unwrap();
                let text = system
                    .shape_text_cached_with_bold(
                        "Ag0@é e\u{301} \u{e0b0} \u{f120}",
                        scale as f32 * 16.0,
                        true,
                    )
                    .unwrap()
                    .0;
                for glyph in text.glyphs() {
                    assert_eq!(glyph.face_id, instance.0);
                    assert!(!glyph.uses_fallback());
                    assert_eq!(
                        system
                            .rasterize_glyph_with_bold(glyph, scale as f32 * 16.0, true)
                            .unwrap(),
                        system.rasterize_glyph(glyph, scale as f32 * 16.0).unwrap()
                    );
                }
                assert_eq!(system.cell_metrics(scale, 16.0).unwrap(), metrics);
            }
        }
    }

    #[test]
    #[ignore = "requires native fonts and the variable fixture through FONTCONFIG_FILE"]
    fn native_variable_unicode_fallback_rasterization() {
        for intensity in [0.75, 1.0, 1.25] {
            let mut system = FontSystem::load_system(FontRequest::Family("JetBrains Mono".into()))
                .unwrap()
                .with_bold_intensity(intensity);
            for size in [16.0, 20.0, 32.0] {
                for ch in "Native weight: regular / Bold / reset Ag0@8 BDPQ counters é e\u{301} ┌─┬─┐ │ └─┴─┘".chars() {
                    let shaped=system.shape_text_cached_with_bold(&ch.to_string(),size,false).unwrap().0;
                    for glyph in shaped.glyphs() { system.rasterize_glyph(glyph,size).unwrap(); }
                }
                for bold in [false, true] {
                    let shaped = system
                        .shape_text_cached_with_bold("Ag0@ 日本語", size, bold)
                        .unwrap()
                        .0;
                    for glyph in shaped.glyphs() {
                        let italic = system
                            .rasterize_glyph_with_style(glyph, size, bold, true)
                            .unwrap();
                        assert_eq!(italic.advance(), glyph.advance());
                        let bitmap = system.rasterize_glyph_with_bold(glyph, size, bold);
                        if glyph.uses_fallback() {
                            eprintln!(
                                "size={size} intensity={intensity} bold={bold} face={:?} glyph={} bitmap={:?}",
                                system.database.face(glyph.face_id).unwrap(),
                                glyph.glyph_id,
                                bitmap.as_ref().map(|b| (
                                    b.width,
                                    b.height,
                                    b.bearing_x,
                                    b.bearing_y
                                ))
                            );
                            assert!(bitmap.is_ok());
                            let bitmap = bitmap.unwrap();
                            assert!(
                                bitmap.width > 0
                                    && bitmap.height > 0
                                    && bitmap.width <= 64
                                    && bitmap.height <= 64
                            );
                            assert!(
                                bitmap.pixels.iter().any(|pixel| *pixel != 0),
                                "blank fallback at size={size} intensity={intensity} bold={bold}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn variable_weight_api_changes_native_outlines_without_changing_advance() {
        use swash::{
            FontRef,
            scale::{Render, ScaleContext, Source},
            shape::ShapeContext,
            zeno::Format,
        };
        let font = FontRef::from_index(
            include_bytes!("../tests/fixtures/JetBrainsMono-variable.ttf"),
            0,
        )
        .unwrap();
        let axis = font
            .variations()
            .find(|axis| axis.tag() == swash::tag_from_bytes(b"wght"))
            .unwrap();
        assert_eq!(
            (axis.min_value(), axis.default_value(), axis.max_value()),
            (100.0, 400.0, 800.0)
        );
        for size in [12.0, 16.0, 20.0, 32.0] {
            let mut previous = None;
            for weight in [525.0, 600.0, 700.0] {
                let mut shape = ShapeContext::new();
                let mut shaper = shape
                    .builder(font)
                    .size(size)
                    .variations([("wght", weight)])
                    .build();
                shaper.add_str("Ag0@");
                let mut glyphs = Vec::new();
                shaper.shape_with(|cluster| glyphs.extend_from_slice(cluster.glyphs));
                let mut scale = ScaleContext::new();
                let mut scaler = scale
                    .builder(font)
                    .size(size)
                    .hint(true)
                    .variations([("wght", weight)])
                    .build();
                let mut render = Render::new(&[Source::Outline]);
                render.format(Format::Alpha);
                let bitmaps: Vec<_> = glyphs
                    .iter()
                    .map(|g| render.render(&mut scaler, g.id).unwrap().data)
                    .collect();
                let advances: Vec<_> = glyphs.iter().map(|g| g.advance).collect();
                if let Some((prior_bitmaps, prior_advances)) = previous {
                    assert_ne!(bitmaps, prior_bitmaps);
                    assert_eq!(advances, prior_advances);
                }
                previous = Some((bitmaps, advances));
            }
        }
    }

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
    fn discovered_monospace_fallback_skips_unusable_faces_and_ignores_generic_alias() {
        let mut database = database_with_face("Proportional Alias", false);
        database.set_monospace_family("Proportional Alias");
        database.push_face_info(FaceInfo {
            id: fontdb::ID::dummy(),
            source: Source::Binary(Arc::new(vec![1, 2, 3])),
            index: 0,
            families: vec![("A Broken Mono".to_owned(), Language::English_UnitedStates)],
            post_script_name: "A-Broken-Mono".to_owned(),
            style: Style::Normal,
            weight: Weight::NORMAL,
            stretch: Stretch::Normal,
            monospaced: true,
        });
        let usable = database.push_face_info(FaceInfo {
            id: fontdb::ID::dummy(),
            source: Source::Binary(Arc::new(TEST_FONT.to_vec())),
            index: 0,
            families: vec![("Z Valid Mono".to_owned(), Language::English_UnitedStates)],
            post_script_name: "Z-Valid-Mono".to_owned(),
            style: Style::Normal,
            weight: Weight::NORMAL,
            stretch: Stretch::Normal,
            monospaced: true,
        });

        assert!(matches!(
            select_primary_face(&database, &FontRequest::SystemMonospace),
            Err(FontLoadError::NoSystemMonospaceFace)
        ));
        assert_eq!(select_discovered_monospace_face(&database), Some(usable));
    }

    #[test]
    fn discovered_monospace_fallback_requires_a_loadable_monospace_face() {
        let database = database_with_face("Proportional", false);
        assert_eq!(select_discovered_monospace_face(&database), None);
    }

    #[test]
    fn fontconfig_match_loads_the_resolved_file_and_rejects_proportional_spacing() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/Tuffy.ttf");
        let mut database = fontdb::Database::new();
        let result = format!("{}\n0\n100\n", path.display());
        let selected = resolve_fontconfig_face(&mut database, &result).unwrap();
        assert!(database.face(selected).is_some());
        assert_eq!(
            resolve_fontconfig_face(&mut database, &result),
            Some(selected)
        );

        let proportional = format!("{}\n0\n0\n", path.display());
        assert_eq!(resolve_fontconfig_face(&mut database, &proportional), None);
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
        let one_x = system.cell_metrics(1.0, DEFAULT_LOGICAL_FONT_SIZE).unwrap();
        let two_x = system.cell_metrics(2.0, DEFAULT_LOGICAL_FONT_SIZE).unwrap();
        let custom_one_x = system.cell_metrics(1.0, 20.0).unwrap();
        let custom_two_x = system.cell_metrics(2.0, 20.0).unwrap();

        assert!(one_x.width() > 0 && one_x.height() > 0);
        assert!(one_x.baseline() > 0 && one_x.baseline() <= one_x.height());
        assert!(two_x.baseline() > 0 && two_x.baseline() <= two_x.height());
        assert!(two_x.width() >= one_x.width() && two_x.height() >= one_x.height());
        assert_eq!(one_x.pixels_per_em(), DEFAULT_LOGICAL_FONT_SIZE);
        assert_eq!(two_x.pixels_per_em(), DEFAULT_LOGICAL_FONT_SIZE * 2.0);
        assert_eq!(custom_one_x.pixels_per_em(), 20.0);
        assert_eq!(custom_two_x.pixels_per_em(), 40.0);
        assert!(custom_one_x.height() >= one_x.height());
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
                    .and_then(|font| font.cell_metrics(1.0, DEFAULT_LOGICAL_FONT_SIZE).ok())
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
    #[ignore = "requires installed Noto Color Emoji"]
    fn native_color_emoji_rasterization() {
        let mut system = FontSystem::load_system(FontRequest::default()).unwrap();
        let metrics = system.cell_metrics(1.0, 16.0).unwrap();
        for size in [16.0, 20.0, 32.0, 48.0] {
            let shaped = system.shape_text("🚀", size).unwrap();
            let glyph = &shaped.glyphs()[0];
            assert!(glyph.uses_fallback());
            assert_ne!(glyph.glyph_id(), 0);
            assert!(
                system
                    .database
                    .face(glyph.face_id)
                    .unwrap()
                    .families
                    .iter()
                    .any(|(name, _)| name == "Noto Color Emoji")
            );
            let bitmap = system.rasterize_glyph(glyph, size).unwrap();
            assert!(bitmap.is_color());
            assert_eq!(
                bitmap.pixels().len(),
                (bitmap.width() * bitmap.height() * 4) as usize
            );
            assert!(bitmap.width() > 0 && bitmap.height() > 0);
            assert!(bitmap.pixels().iter().any(|&byte| byte != 0));
            for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
                let styled = system
                    .rasterize_glyph_with_style(glyph, size, bold, italic)
                    .unwrap();
                // Bitmap strikes are not synthetically weighted or sheared.
                assert_eq!(bitmap, styled);
                assert_eq!(
                    styled,
                    system
                        .rasterize_glyph_with_style(glyph, size, bold, italic)
                        .unwrap()
                );
            }
        }
        assert_eq!(metrics, system.cell_metrics(1.0, 16.0).unwrap());
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

    fn paired_fixture_system() -> (FontSystem, fontdb::ID, fontdb::ID) {
        let mut database = fontdb::Database::new();
        let mut add = |weight, name: &str| {
            database.push_face_info(FaceInfo {
                id: fontdb::ID::dummy(),
                source: Source::Binary(Arc::new(TEST_FONT.to_vec())),
                index: 0,
                families: vec![("Paired Mono".to_owned(), Language::English_UnitedStates)],
                post_script_name: name.to_owned(),
                style: Style::Normal,
                weight,
                stretch: Stretch::Normal,
                monospaced: true,
            })
        };
        let regular = add(Weight::NORMAL, "Paired-Regular");
        let bold = add(Weight::BOLD, "Paired-Bold");
        (
            FontSystem::with_primary_face(database, regular, 16).unwrap(),
            regular,
            bold,
        )
    }

    #[test]
    fn native_bold_is_shaped_with_its_face_and_never_emboldened() {
        // Same bytes under separate fixture identities test selection and synthesis
        // independently from host fonts. Real native outlines are tested separately.
        let (mut system, regular, bold) = paired_fixture_system();
        let metrics = system.cell_metrics(1.25, 16.0).unwrap();
        for size in [16.0, 20.0, 32.0] {
            let (normal, hit) = system
                .shape_text_cached_with_bold("Ae\u{301}", size, false)
                .unwrap();
            assert!(!hit);
            let (weighted, hit) = system
                .shape_text_cached_with_bold("Ae\u{301}", size, true)
                .unwrap();
            assert!(!hit);
            assert!(normal.glyphs().iter().all(|g| g.face_id == regular));
            assert!(
                weighted
                    .glyphs()
                    .iter()
                    .all(|g| g.face_id == bold && !g.uses_fallback())
            );
            for glyph in weighted.glyphs() {
                let native = system.rasterize_glyph_with_bold(glyph, size, true).unwrap();
                assert_eq!(native, system.rasterize_glyph(glyph, size).unwrap());
            }
            assert_eq!(
                system
                    .shape_text_cached_with_bold("Ae\u{301}", size, false)
                    .unwrap(),
                (normal, true)
            );
            assert_eq!(
                system
                    .shape_text_cached_with_bold("Ae\u{301}", size, true)
                    .unwrap(),
                (weighted, true)
            );
        }
        assert_eq!(system.cell_metrics(1.25, 16.0).unwrap(), metrics);
        // A configured primary that is already Bold must not get more weight.
        system.primary_face = bold;
        assert_eq!(system.bold_companion(bold), Some(bold));
    }

    #[test]
    fn missing_native_bold_keeps_unicode_coverage_and_uses_restrained_synthesis() {
        for mut system in [fixture_system(), fallback_system(16)] {
            let normal = system.shape_text("Ae\u{301}", 16.0).unwrap();
            let (bold, _) = system
                .shape_text_cached_with_bold("Ae\u{301}", 16.0, true)
                .unwrap();
            assert_eq!(normal, bold);
            assert!(bold.glyphs().iter().all(|g| g.glyph_id != 0));
            for glyph in bold.glyphs() {
                let regular = system.rasterize_glyph(glyph, 16.0).unwrap();
                let weighted = system.rasterize_glyph_with_bold(glyph, 16.0, true).unwrap();
                assert_eq!(regular.advance(), weighted.advance());
                assert_ne!(regular.pixels(), weighted.pixels());
            }
        }
    }

    #[test]
    fn bold_resolution_rejects_invalid_data_and_mismatched_style() {
        let (mut system, regular, bold) = paired_fixture_system();
        let mut info = system.database.face(bold).unwrap().clone();
        system.database.remove_face(bold);
        info.style = Style::Italic;
        let italic = system.database.push_face_info(info.clone());
        assert_eq!(system.bold_companion(regular), None);
        system.bold_companions.clear();
        system.database.remove_face(italic);
        info.style = Style::Normal;
        info.source = Source::Binary(Arc::new(vec![1, 2, 3]));
        system.database.push_face_info(info);
        assert_eq!(system.bold_companion(regular), None);
    }

    #[test]
    #[ignore = "requires installed JetBrainsMono Nerd Font Mono regular and Bold faces"]
    fn native_jetbrains_bold_uses_distinct_outlines_and_preserves_grid() {
        let mut system =
            FontSystem::load_system(FontRequest::Family("JetBrainsMono Nerd Font Mono".into()))
                .unwrap();
        let primary = system.primary_face;
        let companion = system.bold_companion(primary).expect("native Bold face");
        assert_ne!(primary, companion);
        for scale in [1.0, 1.25, 2.0] {
            let size = 16.0 * scale as f32;
            let metrics = system.cell_metrics(scale, 16.0).unwrap();
            let regular = system.shape_text("Ag0@é", size).unwrap();
            let (bold, _) = system
                .shape_text_cached_with_bold("Ag0@é", size, true)
                .unwrap();
            assert_eq!(regular.glyphs().len(), bold.glyphs().len());
            for (regular, bold) in regular.glyphs().iter().zip(bold.glyphs()) {
                assert_eq!(bold.face_id, companion);
                assert_eq!(regular.advance(), bold.advance());
                let normal = system.rasterize_glyph(regular, size).unwrap();
                let weighted = system.rasterize_glyph_with_bold(bold, size, true).unwrap();
                assert_ne!(normal.pixels(), weighted.pixels());
                assert_eq!(weighted, system.rasterize_glyph(bold, size).unwrap());
            }
            assert_eq!(metrics, system.cell_metrics(scale, 16.0).unwrap());
        }
        let (unicode, _) = system
            .shape_text_cached_with_bold("日本語 \u{e0b0} \u{f120}", 16.0, true)
            .unwrap();
        assert!(unicode.glyphs().iter().all(|g| g.glyph_id != 0));
        assert!(unicode.glyphs().iter().any(|g| g.uses_fallback()));
        eprintln!(
            "regular={} bold={}",
            system.database.face(primary).unwrap().post_script_name,
            system.database.face(companion).unwrap().post_script_name
        );
    }

    #[test]
    #[ignore = "requires installed regular-only Noto Mono"]
    fn native_regular_only_family_uses_synthesis_without_family_substitution() {
        let mut system = FontSystem::load_system(FontRequest::Family("Noto Mono".into())).unwrap();
        let primary = system.primary_face;
        assert_eq!(system.bold_companion(primary), None);
        let (bold, _) = system
            .shape_text_cached_with_bold("Ag0@", 16.0, true)
            .unwrap();
        assert!(bold.glyphs().iter().all(|g| g.face_id == primary));
        for glyph in bold.glyphs() {
            let regular = system.rasterize_glyph(glyph, 16.0).unwrap();
            let synthesized = system.rasterize_glyph_with_bold(glyph, 16.0, true).unwrap();
            assert_ne!(regular.pixels(), synthesized.pixels());
            assert_eq!(regular.advance(), synthesized.advance());
        }
    }

    #[test]
    fn bold_rasterization_preserves_advance_and_separates_cached_weights() {
        for mut system in [fixture_system(), fallback_system(4)] {
            for size in [16.0, 32.0] {
                let shaped = system.shape_text("Ae\u{301}", size).unwrap();
                for glyph in shaped.glyphs() {
                    let normal = system.rasterize_glyph(glyph, size).unwrap();
                    let bold = system.rasterize_glyph_with_bold(glyph, size, true).unwrap();
                    assert_eq!(normal.advance(), bold.advance());
                    let coverage = |bitmap: &super::GlyphBitmap| {
                        bitmap.pixels().iter().map(|&v| u64::from(v)).sum::<u64>()
                    };
                    assert!(coverage(&bold) > coverage(&normal));
                    assert_eq!(system.rasterize_glyph(glyph, size).unwrap(), normal);
                    assert_eq!(
                        system.rasterize_glyph_with_bold(glyph, size, true).unwrap(),
                        bold
                    );
                }
            }
        }
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
            weight: None,
            glyph_id: shaped.glyphs()[0].glyph_id,
            pixels_per_em: 16.0f32.to_bits(),
            bold: false,
            italic: false,
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
