use std::{
  borrow::Cow,
  cell::OnceCell,
  collections::hash_map::Entry,
  fmt::{self, Debug, Formatter},
  ops::Deref,
  str::FromStr,
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
};

use harfrust::font::Variation;
use skrifa::{
  FontRef, MetadataProvider,
  attribute::Style,
  instance::{LocationRef, Size},
  raw::{
    TableProvider,
    types::{F2Dot14, Tag},
  },
};
use thiserror::Error;
use xxhash_rust::xxh3::{Xxh3, xxh3_64};

#[cfg(feature = "svg")]
use std::iter::once;

#[cfg(feature = "svg")]
use crate::{fonts::font_baseline::FontBaseline, resources::glyph::ResolvedGlyph};
use crate::{
  fonts::{
    css_font_selector::CssFontSelector,
    css_segmented_font_face::FontPlatformDataForRangeSet,
    font_custom_platform_data::FontCustomPlatformData,
    font_description::FontSelectionDescription,
    font_face::{FaceData, FontFaceDescriptors},
    font_height::FontHeight,
  },
  layout_unit::LayoutUnit,
  resources::{
    glyph::{GlyphResolveContext, ResolvedGlyphs},
    glyph_cache::resolved_glyph,
    unicode_range::UnicodeRangeSet,
  },
  style::{FontFamily, FontFamilyToken, FontStyle as CssFontStyle, GenericFamilyType},
};

/// Errors that can occur during font loading and conversion.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FontError {
  /// Error occurred during WOFF conversion
  #[cfg(any(feature = "woff", feature = "woff2"))]
  #[error("WOFF conversion failed: {0}")]
  Woff(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
  /// Unsupported Font Format
  #[error("Unsupported font format")]
  UnsupportedFormat,
  /// Font index is invalid
  #[error("Font index is invalid")]
  InvalidFontIndex,
  /// The string is not a CSS generic font family keyword
  #[error("Unknown generic font family keyword")]
  UnknownGenericFamily,
}

/// Supported font formats for loading and processing
#[derive(Copy, Clone)]
#[non_exhaustive]
pub(crate) enum FontFormat {
  #[cfg(feature = "woff")]
  /// Web Open Font Format (WOFF)
  Woff,
  #[cfg(feature = "woff2")]
  /// Web Open Font Format 2 (WOFF2)
  Woff2,
  /// TrueType Font format
  Ttf,
  /// OpenType Font format
  Otf,
  /// TrueType Collection
  Ttc,
}

fn guess_font_format(source: &[u8]) -> Result<FontFormat, FontError> {
  if source.len() < 4 {
    return Err(FontError::UnsupportedFormat);
  }

  match &source[0..4] {
    #[cfg(feature = "woff2")]
    b"wOF2" => Ok(FontFormat::Woff2),
    #[cfg(feature = "woff")]
    b"wOFF" => Ok(FontFormat::Woff),
    [0x00, 0x01, 0x00, 0x00] => Ok(FontFormat::Ttf),
    b"OTTO" => Ok(FontFormat::Otf),
    b"ttcf" => Ok(FontFormat::Ttc),
    _ => Err(FontError::UnsupportedFormat),
  }
}

/// Digest of everything but the glyph id of a run's cache key.
fn glyph_cache_key_prefix(
  font_id: u64,
  font_index: u32,
  font_size: f32,
  coords: &[F2Dot14],
  embolden: Option<f32>,
  skew: Option<f32>,
) -> u64 {
  let mut h = Xxh3::new();
  h.update(&font_id.to_le_bytes());
  h.update(&font_index.to_le_bytes());
  h.update(&font_size.to_le_bytes());
  for c in coords {
    h.update(&c.to_bits().to_le_bytes());
  }
  for synthesis in [embolden, skew] {
    match synthesis {
      Some(value) => {
        h.update(&[1u8]);
        h.update(&value.to_le_bytes());
      }
      None => h.update(&[0u8]),
    }
  }
  h.digest()
}

fn resolved_glyph_cache_key(prefix: u64, glyph_id: u32) -> u64 {
  let mut key = [0; 12];

  key[..8].copy_from_slice(&prefix.to_le_bytes());
  key[8..].copy_from_slice(&glyph_id.to_le_bytes());
  xxh3_64(&key)
}

/// A font family produced by [`Fonts::register`], with the faces it contains.
#[derive(Clone, Debug, serde::Serialize)]
pub struct RegisteredFamily {
  /// Family name as stored by the font system (normalized; reflects any override).
  pub name: String,
  /// Faces registered under this family.
  pub faces: Vec<RegisteredFace>,
}

/// A single face within a [`RegisteredFamily`].
#[derive(Clone, Debug, serde::Serialize)]
pub struct RegisteredFace {
  /// Weight class, typically `1.0..=1000.0`.
  pub weight: f32,
  /// CSS `font-style` value (`normal`, `italic`, or `oblique [<angle>deg]`).
  pub style: String,
  /// Width as a percentage of normal (e.g. `100.0`).
  pub width: f32,
  /// Index of the face within its source collection.
  pub index: u32,
}

fn font_style_css(style: Style) -> String {
  match style {
    Style::Normal => "normal".to_string(),
    Style::Italic => "italic".to_string(),
    Style::Oblique(None) => "oblique".to_string(),
    Style::Oblique(Some(angle)) => format!("oblique {angle}deg"),
  }
}

const SBIX: Tag = Tag::new(b"sbix");
const COLR: Tag = Tag::new(b"COLR");
const CPAL: Tag = Tag::new(b"CPAL");
const CBDT: Tag = Tag::new(b"CBDT");
const CBLC: Tag = Tag::new(b"CBLC");

/// Color tables in the pairs Blink's `ColorTableLookup` accepts.
pub(crate) fn has_color_table(font: &FontRef) -> bool {
  font.table_data(SBIX).is_some()
    || (font.table_data(COLR).is_some() && font.table_data(CPAL).is_some())
    || (font.table_data(CBDT).is_some() && font.table_data(CBLC).is_some())
}

/// A font file's sfnt bytes, shared, with the id glyph caches key them by.
#[derive(Clone)]
pub struct FontBlob {
  data: SharedBytes,
  id: u64,
}

impl FontBlob {
  pub(crate) fn new(data: SharedBytes, id: u64) -> Self {
    Self { data, id }
  }

  /// The sfnt bytes.
  pub fn data(&self) -> &[u8] {
    (*self.data).as_ref()
  }

  /// The sfnt bytes, shared with the blob instead of copied.
  pub fn shared_data(&self) -> Arc<dyn AsRef<[u8]> + Send + Sync> {
    self.data.clone()
  }

  /// The id glyph caches key the bytes by.
  pub fn id(&self) -> u64 {
    self.id
  }
}

impl AsRef<[u8]> for FontBlob {
  fn as_ref(&self) -> &[u8] {
    self.data()
  }
}

impl Debug for FontBlob {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("FontBlob").field("id", &self.id).finish()
  }
}

/// A face font matching picked, before it takes a size: what Blink makes `SimpleFontData` from.
#[derive(Clone, Debug)]
pub(crate) struct MatchedFace {
  /// The registered face, shared with the registry.
  pub(crate) typeface: Arc<FontCustomPlatformData>,
  /// Blink's `FontPlatformData::SyntheticBold`.
  pub(crate) synthetic_bold: bool,
  /// Blink's `FontPlatformData::SyntheticItalic`.
  pub(crate) synthetic_italic: bool,
  /// The design coordinates the face draws at, a later tag winning.
  pub(crate) variations: Vec<Variation>,
  /// The face's `unicode-range`, or `None` for every codepoint.
  pub(crate) unicode_range: Option<UnicodeRangeSet>,
}

impl MatchedFace {
  fn of(face: FontPlatformDataForRangeSet) -> Self {
    let platform = face.font_platform_data;

    Self {
      typeface: platform.typeface,
      synthetic_bold: platform.synthetic_bold,
      synthetic_italic: platform.synthetic_italic,
      variations: platform.variations,
      unicode_range: (!face.range_set.is_entire_range()).then_some(face.range_set),
    }
  }
}

/// The fonts a renderer draws with, Blink's `CSSFontSelector`, whose snapshots share one
/// copy-on-write registry.
#[derive(Clone, Default)]
pub struct Fonts {
  registry: Arc<FontRegistry>,
  /// The families a snapshot's fallback walks before the last-resort ones, in place of every
  /// registered family.
  fallback_families: Option<Vec<Arc<str>>>,
}

/// What registration builds, shared by a [`Fonts`] and every snapshot taken from it since.
#[derive(Clone, Default)]
struct FontRegistry {
  selector: CssFontSelector,
  /// A process-wide stamp of every change, which font-dependent caches key on.
  revision: u64,
}

impl FontRegistry {
  /// Marks the registry changed, so font-dependent caches can tell its states apart.
  fn revise(&mut self) {
    static REVISION: AtomicU64 = AtomicU64::new(1);

    self.revision = REVISION.fetch_add(1, Ordering::Relaxed);
  }
}

/// A render-local font handle: the registry as it stood, and the render's fallback families.
#[derive(Clone)]
pub struct FontsSnapshot(Fonts);

impl Deref for FontsSnapshot {
  type Target = Fonts;

  fn deref(&self) -> &Fonts {
    &self.0
  }
}

/// Synthetic styling still needed after variable axes are applied.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RunSynthesis {
  /// Stroke width in px for synthetic bold.
  pub embolden: Option<f32>,
  /// Synthetic oblique angle in degrees.
  pub skew: Option<f32>,
}

/// What resolving a run's glyphs reads from it: its face, size, instance, and synthesis.
pub(crate) struct RunFace<'a> {
  pub(crate) font_id: u64,
  pub(crate) font_index: u32,
  pub(crate) font_size: f32,
  pub(crate) normalized_coords: &'a [F2Dot14],
  pub(crate) synthesis: RunSynthesis,
}

impl Fonts {
  /// The faces each of `families` resolves to, as Blink's `FontFallbackList` takes them: every
  /// face of the family matching `description` best, several when `unicode-range` splits it.
  pub(crate) fn font_group(
    &self,
    families: &[FontFamilyToken],
    description: &FontSelectionDescription,
  ) -> Vec<Vec<FaceData<MatchedFace>>> {
    families
      .iter()
      .filter_map(|family| match family {
        FontFamilyToken::Owned(name) => {
          self
            .registry
            .selector
            .get_font_data(description, name, false)
        }
        FontFamilyToken::Generic(generic) => {
          self
            .registry
            .selector
            .get_font_data(description, generic.keyword(), true)
        }
      })
      .map(|faces| {
        faces
          .into_iter()
          .map(|face| face.map(MatchedFace::of))
          .collect()
      })
      .collect()
  }

  /// The faces system font fallback picks from: the fallback families, the last-resort families
  /// last, each by its best match for `description`.
  pub(crate) fn fallback_bucket(
    &self,
    description: &FontSelectionDescription,
  ) -> Vec<FaceData<MatchedFace>> {
    let families: Vec<&Arc<str>> = match &self.fallback_families {
      Some(families) => families
        .iter()
        .chain(self.registry.selector.last_resort_families())
        .collect(),
      None => self.registry.selector.fallback_families().collect(),
    };

    families
      .into_iter()
      .filter_map(|family| {
        self
          .registry
          .selector
          .get_font_data(description, family, false)
      })
      .flatten()
      .map(|face| face.map(MatchedFace::of))
      .collect()
  }

  /// Registration revision of the font registry.
  #[cfg(feature = "svg-raster")]
  pub(crate) fn revision(&self) -> u64 {
    self.registry.revision
  }

  /// Render-local snapshot with no extra fallbacks.
  pub fn snapshot(&self) -> FontsSnapshot {
    self.snapshot_with_fallbacks(None)
  }

  /// The registered family name of a face, by the font blob id and collection index a
  /// shaped run reports.
  #[cfg(feature = "paint-tree")]
  pub(crate) fn face_family(&self, font_id: u64, index: u32) -> Option<&str> {
    self
      .registry
      .selector
      .font_faces()
      .find(|face| {
        face
          .custom_platform_data()
          .is_some_and(|typeface| typeface.id == font_id && typeface.index == index)
      })
      .map(|face| &*face.family)
  }

  /// Render-local snapshot whose fallback walks the families of `fallbacks`, a generic one as the
  /// families registered for it, then the last-resort ones.
  pub fn snapshot_with_fallbacks(&self, fallbacks: Option<&FontFamily>) -> FontsSnapshot {
    let fallback_families = fallbacks.map(|fallbacks| {
      fallbacks
        .tokens()
        .iter()
        .flat_map(|token| match token {
          FontFamilyToken::Owned(name) => vec![Arc::from(name.as_str())],
          FontFamilyToken::Generic(generic) => self
            .registry
            .selector
            .family_names_from_settings(generic.keyword())
            .to_vec(),
        })
        .collect()
    });

    FontsSnapshot(Self {
      registry: Arc::clone(&self.registry),
      fallback_families,
    })
  }

  #[cfg(feature = "svg")]
  pub(crate) fn resolve_glyph(
    &self,
    face: RunFace<'_>,
    font_ref: FontRef,
    glyph_id: u32,
  ) -> Option<Arc<ResolvedGlyph>> {
    let mut resolved = ResolvedGlyphs::default();

    self.resolve_glyphs_into(&mut resolved, face, font_ref, once(glyph_id));
    resolved.remove(&glyph_id)
  }

  /// Adds the glyphs among `glyph_ids` that `resolved` lacks, drawn in `face`.
  pub(crate) fn resolve_glyphs_into(
    &self,
    resolved: &mut ResolvedGlyphs,
    face: RunFace<'_>,
    font_ref: FontRef,
    glyph_ids: impl Iterator<Item = u32>,
  ) {
    let RunFace {
      font_id,
      font_index,
      font_size,
      normalized_coords,
      synthesis: RunSynthesis { embolden, skew },
    } = face;
    // Built on the first cache miss; a run whose glyphs are all cached never needs it.
    let resolver = OnceCell::new();
    let resolver = || {
      resolver.get_or_init(|| GlyphResolveContext {
        outline_glyphs: font_ref.outline_glyphs(),
        color_glyphs: font_ref.color_glyphs(),
        bitmap_strikes: font_ref.bitmap_strikes(),
        font_size,
        size: Size::new(font_size),
        location: LocationRef::new(normalized_coords),
        embolden,
        skew,
      })
    };

    let key_prefix = glyph_cache_key_prefix(
      font_id,
      font_index,
      font_size,
      normalized_coords,
      embolden,
      skew,
    );
    for glyph_id in glyph_ids {
      if let Entry::Vacant(slot) = resolved.entry(glyph_id) {
        let key = resolved_glyph_cache_key(key_prefix, glyph_id);
        if let Some(glyph) = resolved_glyph(key, || resolver().resolve_glyph(glyph_id)) {
          slot.insert(glyph);
        }
      }
    }
  }

  /// Registers a font, decoding it and skipping faces already registered under the same family
  /// in this context.
  pub fn register(&mut self, font: FontResource) -> Result<Vec<RegisteredFamily>, FontError> {
    let registry = Arc::make_mut(&mut self.registry);

    registry.revise();

    let FontResource {
      source,
      info_override,
      generic_family,
      unicode_range,
      last_resort,
      is_restricted,
    } = font;
    let (data, id) = source.into_sfnt_with_id()?;
    let (data, id) = if is_restricted {
      (data, id)
    } else {
      unicode_range.restrict_sfnt(data, id)
    };
    let descriptors = FontFaceDescriptors {
      unicode_range,
      ..info_override
        .as_ref()
        .map(FontOverride::descriptors)
        .unwrap_or_default()
    };
    let faces = registry.selector.register(
      data,
      id,
      &descriptors,
      generic_family.map(|generic| generic.0.keyword()),
      last_resort,
    );
    let mut families: Vec<RegisteredFamily> = Vec::new();

    for font_face in faces {
      let Some(typeface) = font_face.custom_platform_data() else {
        continue;
      };

      let face = RegisteredFace {
        weight: typeface.attributes.weight.value(),
        style: font_style_css(typeface.attributes.style),
        width: typeface.attributes.stretch.ratio() * 100.0,
        index: typeface.index,
      };

      match families
        .iter_mut()
        .find(|family| *family.name == *font_face.family)
      {
        Some(family) => family.faces.push(face),
        None => families.push(RegisteredFamily {
          name: font_face.family.to_string(),
          faces: vec![face],
        }),
      }
    }

    Ok(families)
  }

  /// Registers a face still loading, which [`RenderNeeds::faces`](crate::layout::needs::RenderNeeds::faces) reports when reached.
  pub fn register_pending(&mut self, face: PendingFontFace) {
    let registry = Arc::make_mut(&mut self.registry);

    registry.revise();
    registry
      .selector
      .register_pending(face.family, face.descriptors, face.id);
  }

  /// Whether a pending face is registered.
  pub(crate) fn has_pending_faces(&self) -> bool {
    self.registry.selector.has_pending_faces()
  }

  /// Removes every face of `family`, pending ones included; false when it has none.
  pub fn unregister_family(&mut self, family: &str) -> bool {
    let registry = Arc::make_mut(&mut self.registry);

    if !registry.selector.unregister_family(family) {
      return false;
    }

    registry.revise();

    true
  }
}

/// A primary font's vertical metrics at a size, each rounded as Blink's `FontMetrics` rounds them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PrimaryFontMetrics {
  pub(crate) ascent: f32,
  pub(crate) descent: f32,
  pub(crate) line_gap: f32,
  /// Unrounded, as Blink's `FontMetrics::XHeight`.
  pub(crate) x_height: Option<f32>,
  /// The ascent, descent and line gap before rounding, which a scaled font rounds anew.
  pub(crate) exact: ExactFontMetrics,
  /// The `post` table's underline, when the font has one.
  pub(crate) underline: Option<FontUnderline>,
  /// How far the em box reaches below the baseline.
  pub(crate) em_descent: f32,
  /// Blink's `FontMetrics::ZeroWidth`: the advance of the `0` glyph, the `ch` unit's basis.
  pub(crate) zero_width: f32,
  /// Blink's `FontMetrics::HasZeroWidth`: whether the font maps `0` to a glyph.
  pub(crate) has_zero_width: bool,
  /// Blink's `SimpleFontData::AvgCharWidth` as `PlatformInit` reads it off FreeType outside Apple
  /// platforms: the `OS/2` average width, else the `x` glyph's advance, else the x-height.
  pub(crate) avg_char_width: f32,
  /// Blink's `SimpleFontData::MaxCharWidth` outside Windows and Apple platforms: the width of the
  /// `head` table's bounding box, in whole pixels.
  pub(crate) max_char_width: f32,
}

/// A font's own underline.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FontUnderline {
  /// How far below the baseline its top sits.
  pub(crate) position: f32,
  pub(crate) thickness: f32,
}

/// Blink's `SimpleFontData::ZeroInlineSize`, the [`ch`](https://drafts.csswg.org/css-values-4/#ch) unit.
pub(crate) fn zero_inline_size(font: &FontRef, font_size: f32, location: LocationRef<'_>) -> f32 {
  font
    .charmap()
    .map('0')
    .and_then(|glyph| {
      font
        .glyph_metrics(Size::new(font_size), location)
        .advance_width(glyph)
    })
    .unwrap_or(font_size / 2.0)
}

/// How far the [em box](https://drafts.csswg.org/css-inline-3/#ascent-descent) reaches below the baseline.
pub(crate) fn em_box_descent(
  font: Option<&FontRef>,
  font_size: f32,
  ascent: f32,
  descent: f32,
) -> f32 {
  let (em_ascent, em_descent) = font
    .and_then(|font| font.os2().ok())
    .map(|os2| {
      (
        f32::from(os2.s_typo_ascender()),
        -f32::from(os2.s_typo_descender()),
      )
    })
    .filter(|(ascent, descent)| ascent + descent > 0.0)
    .unwrap_or((ascent, descent));

  if em_ascent + em_descent <= 0.0 || em_ascent < 0.0 {
    return descent;
  }

  font_size * em_descent / (em_ascent + em_descent)
}

/// A font's vertical metrics at a size, unrounded.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ExactFontMetrics {
  pub(crate) ascent: f32,
  pub(crate) descent: f32,
}

impl PrimaryFontMetrics {
  /// The metrics of `font_ref` at `font_size` and `location`, rounded as Blink's
  /// `SimpleFontData::PlatformInit` rounds Skia's.
  pub(crate) fn of(font_ref: &FontRef, font_size: f32, location: LocationRef<'_>) -> Self {
    let metrics = font_ref.metrics(Size::new(font_size), location);

    Self {
      ascent: metrics.ascent.round(),
      descent: metrics.descent.abs().round(),
      line_gap: metrics.leading.round(),
      x_height: metrics.x_height,
      exact: ExactFontMetrics {
        ascent: metrics.ascent,
        descent: metrics.descent.abs(),
      },
      underline: metrics.underline.map(|underline| FontUnderline {
        position: -underline.offset,
        thickness: underline.thickness,
      }),
      em_descent: em_box_descent(
        Some(font_ref),
        font_size,
        metrics.ascent,
        metrics.descent.abs(),
      ),
      zero_width: zero_inline_size(font_ref, font_size, location),
      has_zero_width: font_ref.charmap().map('0').is_some(),
      avg_char_width: metrics
        .average_width
        .filter(|&width| width != 0.0)
        .or_else(|| {
          let glyph = font_ref.charmap().map('x')?;

          font_ref
            .glyph_metrics(Size::new(font_size), location)
            .advance_width(glyph)
        })
        .or(metrics.x_height)
        .unwrap_or(0.0),
      max_char_width: metrics
        .bounds
        .map_or(0.0, |bounds| (bounds.x_max - bounds.x_min).round()),
    }
  }

  /// Blink's `FontMetrics::GetFontHeight` at the alphabetic baseline: the whole-pixel ascent and
  /// descent.
  pub(crate) fn font_height(self) -> FontHeight {
    FontHeight::new(
      LayoutUnit::from_float(self.ascent),
      LayoutUnit::from_float(self.descent),
    )
  }

  /// Blink's `FontMetrics::GetFloatFontHeight`: the ascent and descent in the nearest layout units.
  pub(crate) fn float_font_height(self) -> FontHeight {
    FontHeight::new(
      LayoutUnit::from_float_round(self.exact.ascent),
      LayoutUnit::from_float_round(self.exact.descent),
    )
  }

  /// Blink's `FontMetrics::FixedLineSpacing`.
  pub(crate) fn fixed_line_spacing(self) -> LayoutUnit {
    LayoutUnit::from_float_round(self.line_spacing())
  }

  /// The `line-height: normal` these metrics give, as Blink's `FontMetrics::SetLineSpacing`.
  pub(crate) fn line_spacing(self) -> f32 {
    self.ascent + self.descent + self.line_gap
  }

  /// Blink's `FontMetrics::FloatHeight`.
  #[cfg(feature = "svg")]
  pub(crate) fn float_height(self) -> f32 {
    self.ascent + self.descent
  }

  /// Blink's `FontMetrics::XHeight`: the font's x-height, or 56% of the ascent, as Blink's
  /// `SimpleFontData::PlatformInit` guesses for a font without one.
  #[cfg(feature = "svg")]
  pub(crate) fn float_x_height(self) -> f32 {
    self
      .x_height
      .filter(|x_height| *x_height > 0.0)
      .unwrap_or(self.ascent * 0.56)
  }

  /// Blink's `FontMetrics::FloatAscent`: how far `baseline` sits below the top of the text.
  #[cfg(feature = "svg")]
  pub(crate) fn float_ascent(self, baseline: FontBaseline) -> f32 {
    match baseline {
      FontBaseline::Alphabetic => self.ascent,
      FontBaseline::Central => self.float_height() / 2.0,
      FontBaseline::TextUnder | FontBaseline::IdeographicUnder => self.float_height(),
      FontBaseline::XMiddle => self.ascent - self.float_x_height() / 2.0,
      FontBaseline::Math => self.ascent * 0.5,
      FontBaseline::Hanging => self.ascent * 0.2,
      FontBaseline::TextOver => 0.0,
    }
  }
}

/// Bytes the caller already holds behind an `Arc`, taken as they are.
pub(crate) type SharedBytes = Arc<dyn AsRef<[u8]> + Send + Sync>;

enum FontBytes<'a> {
  Inline(Cow<'a, [u8]>),
  Shared(SharedBytes),
}

impl FontBytes<'_> {
  fn into_owned(self) -> Vec<u8> {
    match self {
      Self::Inline(bytes) => bytes.into_owned(),
      Self::Shared(bytes) => (*bytes).as_ref().to_vec(),
    }
  }

  /// The raw sfnt bytes, decompressing WOFF and WOFF2.
  fn into_sfnt(self) -> Result<Vec<u8>, FontError> {
    match guess_font_format(self.as_ref())? {
      FontFormat::Ttf | FontFormat::Otf | FontFormat::Ttc => Ok(self.into_owned()),
      #[cfg(feature = "woff2")]
      FontFormat::Woff2 => {
        wuff::decompress_woff2(self.as_ref()).map_err(|e| FontError::Woff(Box::new(e)))
      }
      #[cfg(feature = "woff")]
      FontFormat::Woff => {
        wuff::decompress_woff1(self.as_ref()).map_err(|e| FontError::Woff(Box::new(e)))
      }
    }
  }
}

impl Debug for FontBytes<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_tuple("FontBytes")
      .field(&self.as_ref().len())
      .finish()
  }
}

impl AsRef<[u8]> for FontBytes<'_> {
  fn as_ref(&self) -> &[u8] {
    match self {
      Self::Inline(bytes) => bytes,
      Self::Shared(bytes) => (**bytes).as_ref(),
    }
  }
}

/// A font source buffer.
#[derive(Debug)]
pub struct FontSource<'a> {
  bytes: FontBytes<'a>,
  /// Whether `bytes` is already decompressed (woff/woff2 expanded to raw sfnt).
  is_decoded: bool,
  /// Blob id to use in place of hashing the decoded bytes, set by [`FontSource::from_static`].
  cache_id: Option<u64>,
}

impl<'a, T> From<T> for FontSource<'a>
where
  T: Into<Cow<'a, [u8]>>,
{
  fn from(value: T) -> Self {
    Self {
      bytes: FontBytes::Inline(value.into()),
      is_decoded: false,
      cache_id: None,
    }
  }
}

impl<'a> FontSource<'a> {
  /// Registers shared font bytes without copying them.
  pub fn from_shared(bytes: Arc<dyn AsRef<[u8]> + Send + Sync>) -> Self {
    Self {
      bytes: FontBytes::Shared(bytes),
      is_decoded: false,
      cache_id: None,
    }
  }

  /// Takes bytes that live as long as the process, `include_bytes!` above all.
  pub fn from_static(bytes: &'static [u8]) -> Self {
    let mut id = Xxh3::new();

    id.update(&bytes.as_ptr().addr().to_le_bytes());
    id.update(&bytes.len().to_le_bytes());

    Self {
      bytes: FontBytes::Shared(Arc::new(bytes)),
      is_decoded: false,
      cache_id: Some(id.digest()),
    }
  }

  /// Whether the bytes can go to the font system untouched.
  fn is_sfnt(&self) -> bool {
    matches!(
      guess_font_format(self.bytes.as_ref()),
      Ok(FontFormat::Ttf | FontFormat::Otf | FontFormat::Ttc)
    )
  }

  /// The decoded sfnt bytes and the id the glyph caches key them by.
  pub(crate) fn into_sfnt_with_id(self) -> Result<(SharedBytes, u64), FontError> {
    let passthrough = self.is_decoded || self.is_sfnt();
    let cache_id = self.cache_id;
    let decoded: SharedBytes = match self.bytes {
      FontBytes::Shared(bytes) if passthrough => bytes,
      bytes => Arc::new(bytes.into_sfnt()?),
    };

    // The id keys the shared glyph caches, so registering the same face again — a second
    // renderer, a rebuilt one — keeps every glyph it had already resolved.
    let id = cache_id.unwrap_or_else(|| xxh3_64((*decoded).as_ref()));

    Ok((decoded, id))
  }
}

impl<'a> AsRef<[u8]> for FontSource<'a> {
  fn as_ref(&self) -> &[u8] {
    self.bytes.as_ref()
  }
}

/// A CSS generic font family a registered font fulfills.
/// <https://drafts.csswg.org/css-fonts/#generic-font-families>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenericFamily(GenericFamilyType);

impl GenericFamily {
  /// `serif`
  pub const SERIF: Self = Self(GenericFamilyType::Serif);
  /// `sans-serif`
  pub const SANS_SERIF: Self = Self(GenericFamilyType::SansSerif);
  /// `monospace`
  pub const MONOSPACE: Self = Self(GenericFamilyType::Monospace);
  /// `cursive`
  pub const CURSIVE: Self = Self(GenericFamilyType::Cursive);
  /// `fantasy`
  pub const FANTASY: Self = Self(GenericFamilyType::Fantasy);
  /// `system-ui`
  pub const SYSTEM_UI: Self = Self(GenericFamilyType::SystemUi);
  /// `ui-serif`
  pub const UI_SERIF: Self = Self(GenericFamilyType::UiSerif);
  /// `ui-sans-serif`
  pub const UI_SANS_SERIF: Self = Self(GenericFamilyType::UiSansSerif);
  /// `ui-monospace`
  pub const UI_MONOSPACE: Self = Self(GenericFamilyType::UiMonospace);
  /// `ui-rounded`
  pub const UI_ROUNDED: Self = Self(GenericFamilyType::UiRounded);
  /// `emoji`
  pub const EMOJI: Self = Self(GenericFamilyType::Emoji);
  /// `math`
  pub const MATH: Self = Self(GenericFamilyType::Math);
  /// `fangsong`
  pub const FANG_SONG: Self = Self(GenericFamilyType::FangSong);
}

impl FromStr for GenericFamily {
  type Err = FontError;

  /// Parses a CSS generic family keyword (e.g. `monospace`, `sans-serif`).
  fn from_str(keyword: &str) -> Result<Self, Self::Err> {
    GenericFamilyType::parse(keyword)
      .map(Self)
      .ok_or(FontError::UnknownGenericFamily)
  }
}

/// Metadata overrides for a registered font.
#[derive(Debug, Default, Clone)]
pub struct FontOverride {
  /// Family name to register the font under, instead of its embedded name.
  pub family_name: Option<Arc<str>>,
  /// Font weight (CSS numeric, e.g. `400.0`) to use instead of the embedded one.
  pub weight: Option<f32>,
  /// Font style (slant) to use instead of the embedded one.
  pub style: Option<CssFontStyle>,
  /// Font width as a percentage (e.g. `100.0` for normal) to use instead of the embedded one.
  pub width: Option<f32>,
  /// Default values for named variation axes (four-byte OpenType tags).
  pub axes: Vec<(String, f32)>,
}

impl FontOverride {
  /// The `@font-face` descriptors these overrides stand for.
  fn descriptors(&self) -> FontFaceDescriptors {
    FontFaceDescriptors {
      family: self.family_name.clone(),
      weight: self.weight.map(|weight| (weight, weight)),
      width: self.width.map(|width| (width, width)),
      slope: self.style.map(|style| (style.slope(), style.slope())),
      variation_settings: self
        .axes
        .iter()
        .filter_map(|(tag, value)| {
          Tag::new_checked(tag.as_bytes()).ok().map(|tag| Variation {
            tag: harfrust::Tag::new(&tag.to_be_bytes()),
            value: *value,
          })
        })
        .collect(),
      ..FontFaceDescriptors::default()
    }
  }
}

/// A face known by its `@font-face` descriptors before its data loads.
#[derive(Debug, Clone)]
pub struct PendingFontFace {
  id: usize,
  family: Arc<str>,
  descriptors: FontFaceDescriptors,
}

impl PendingFontFace {
  /// A face of `family` reported as `id`, every descriptor `normal`.
  pub fn new(id: usize, family: impl Into<Arc<str>>) -> Self {
    Self {
      id,
      family: family.into(),
      descriptors: FontFaceDescriptors::default(),
    }
  }

  /// Sets `font-weight` to the range `start` to `end`.
  pub fn weight(mut self, start: f32, end: f32) -> Self {
    self.descriptors.weight = Some((start, end));
    self
  }

  /// Sets `font-style`.
  pub fn style(mut self, style: CssFontStyle) -> Self {
    self.descriptors.slope = Some((style.slope(), style.slope()));
    self
  }

  /// Limits the codepoints the face serves, as `@font-face` `unicode-range` does.
  pub fn unicode_range(mut self, unicode_range: UnicodeRangeSet) -> Self {
    self.descriptors.unicode_range = unicode_range;
    self
  }
}

#[derive(Debug)]
/// Information of a font resource
pub struct FontResource<'a> {
  source: FontSource<'a>,
  info_override: Option<FontOverride>,
  generic_family: Option<GenericFamily>,
  unicode_range: UnicodeRangeSet,
  /// Sorts after every normal family in default fallback selection (see
  /// [`FontResource::last_resort`]).
  last_resort: bool,
  /// Whether [`Self::into_resolved`] already limited the `cmap` to `unicode_range`.
  is_restricted: bool,
}

impl<'a> FontResource<'a> {
  /// Create a new font to load
  pub fn new(source: impl Into<FontSource<'a>>) -> Self {
    Self {
      source: source.into(),
      info_override: None,
      generic_family: None,
      unicode_range: UnicodeRangeSet::default(),
      last_resort: false,
      is_restricted: false,
    }
  }

  /// Set font metadata overrides
  pub fn override_info(self, info_override: FontOverride) -> Self {
    Self {
      info_override: Some(info_override),
      ..self
    }
  }

  /// Set generic family for the font
  pub fn generic_family(self, generic_family: GenericFamily) -> Self {
    Self {
      generic_family: Some(generic_family),
      ..self
    }
  }

  /// Limits the codepoints this font serves, as `@font-face` `unicode-range` does.
  pub fn unicode_range(self, unicode_range: UnicodeRangeSet) -> Self {
    Self {
      unicode_range,
      ..self
    }
  }

  /// Registers the font under the family `logical`.
  #[deprecated(note = "register every subset under its family name with `unicode_range` instead")]
  pub fn subset_of(self, logical: impl Into<Arc<str>>) -> Self {
    let mut info_override = self.info_override.unwrap_or_default();

    info_override.family_name = Some(logical.into());

    Self {
      info_override: Some(info_override),
      ..self
    }
  }

  /// Does nothing; `unicode_range` decides which subset serves a codepoint.
  #[deprecated(note = "use `unicode_range` instead")]
  pub fn subset_rank(self, _rank: u32) -> Self {
    self
  }

  /// Sorts this font's families after every normal family in default fallback selection, so they
  /// only serve text no registered font covers.
  pub fn last_resort(self) -> Self {
    Self {
      last_resort: true,
      ..self
    }
  }

  /// Bytes of its font data, as decoded once [`Self::into_resolved`] has run.
  pub fn byte_len(&self) -> usize {
    self.source.as_ref().len()
  }

  /// Decompresses, restricts, and hashes the font, so registering it after costs the same at any size.
  pub fn into_resolved(self) -> Result<Self, FontError> {
    if self.is_restricted {
      return Ok(self);
    }

    let (data, id) = self.source.into_sfnt_with_id()?;
    let (data, id) = self.unicode_range.restrict_sfnt(data, id);

    Ok(Self {
      source: FontSource {
        bytes: FontBytes::Shared(data),
        is_decoded: true,
        cache_id: Some(id),
      },
      is_restricted: true,
      ..self
    })
  }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)]
mod tests {
  use std::{fs::File, io::Read, path::Path};

  use super::*;
  use crate::{resources::unicode_range::UnicodeRange, style::FromCssStr};

  fn read_font_asset(relative: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    let mut bytes = Vec::new();
    let mut file = File::open(&path)
      .unwrap_or_else(|error| panic!("failed to open test font {}: {error}", path.display()));
    file
      .read_to_end(&mut bytes)
      .unwrap_or_else(|error| panic!("failed to read test font {}: {error}", path.display()));
    bytes
  }

  /// The bytes `source` registers, and their id.
  fn blob(source: FontSource<'_>) -> FontBlob {
    let (data, id) = source.into_sfnt_with_id().unwrap();

    FontBlob::new(data, id)
  }

  /// `source` as [`FontResource::into_resolved`] leaves it.
  fn resolved(source: FontSource<'_>) -> FontSource<'_> {
    FontResource::new(source).into_resolved().unwrap().source
  }

  /// The families fallback walks before the last-resort ones.
  fn order(fonts: &Fonts) -> Vec<String> {
    let last_resort: Vec<&Arc<str>> = fonts.registry.selector.last_resort_families().collect();

    fonts
      .registry
      .selector
      .fallback_families()
      .filter(|family| !last_resort.contains(family))
      .map(ToString::to_string)
      .collect()
  }

  fn geist_bytes() -> Vec<u8> {
    read_font_asset("../assets/fonts/geist/Geist[wght].woff2")
  }

  fn geist_mono_bytes() -> Vec<u8> {
    read_font_asset("../assets/fonts/geist/GeistMono[wght].woff2")
  }

  fn register_named(fonts: &mut Fonts, bytes: Vec<u8>, family_name: &str) -> Vec<RegisteredFamily> {
    fonts
      .register(FontResource::new(bytes).override_info(FontOverride {
        family_name: Some(family_name.into()),
        ..Default::default()
      }))
      .unwrap()
  }

  #[test]
  fn register_returns_family_with_at_least_one_face() {
    let mut fonts = Fonts::default();
    let families = register_named(&mut fonts, geist_bytes(), "Geist Test");

    assert_eq!(families.len(), 1);
    assert_eq!(families[0].name, "Geist Test");
    assert!(!families[0].faces.is_empty());
  }

  /// The same bytes under another family share one decoded face, so one set of shaping tables.
  #[test]
  fn a_font_registered_under_two_families_shares_its_face() {
    let mut fonts = Fonts::default();

    register_named(&mut fonts, geist_bytes(), "Geist A");
    register_named(&mut fonts, geist_bytes(), "Geist B");

    let faces: Vec<_> = fonts.registry.selector.font_faces().collect();

    assert_eq!(faces.len(), 2);
    assert!(Arc::ptr_eq(
      faces[0].custom_platform_data().unwrap(),
      faces[1].custom_platform_data().unwrap()
    ));
  }

  fn register_weighted(fonts: &mut Fonts, weight: f32) {
    fonts
      .register(
        FontResource::new(geist_bytes()).override_info(FontOverride {
          family_name: Some("Geist".into()),
          weight: Some(weight),
          ..Default::default()
        }),
      )
      .unwrap();
  }

  /// One file declared at two weights keeps a face for each, as `@font-face` rules sharing a `src` do.
  #[test]
  fn a_font_registered_at_two_weights_keeps_both_faces() {
    let mut fonts = Fonts::default();

    register_weighted(&mut fonts, 400.0);
    register_weighted(&mut fonts, 700.0);
    register_weighted(&mut fonts, 700.0);

    let weights: Vec<_> = fonts
      .registry
      .selector
      .font_faces()
      .map(|face| face.descriptors.weight)
      .collect();

    assert_eq!(weights, vec![Some((400.0, 400.0)), Some((700.0, 700.0))]);
  }

  /// Copies that differ only past their tables restrict to the same sfnt, so one `unicode-range` subset
  /// declared at two weights keeps a face for each too.
  #[test]
  fn a_ranged_subset_registered_at_two_weights_keeps_both_faces() {
    let mut fonts = Fonts::default();
    let latin: UnicodeRangeSet = [UnicodeRange::new(0x00, 0xff)].into_iter().collect();
    let sfnt = read_font_asset("../assets/fonts/geist/geist-latin-wght-300-800.ttf");
    let mut padded = sfnt.clone();

    padded.extend([1; 8]);

    for (bytes, weight) in [(sfnt, 400.0), (padded, 700.0)] {
      fonts
        .register(
          FontResource::new(bytes)
            .override_info(FontOverride {
              family_name: Some("Geist".into()),
              weight: Some(weight),
              ..Default::default()
            })
            .unicode_range(latin.clone()),
        )
        .unwrap();
    }

    let weights: Vec<_> = fonts
      .registry
      .selector
      .font_faces()
      .map(|face| face.descriptors.weight)
      .collect();

    assert_eq!(weights, vec![Some((400.0, 400.0)), Some((700.0, 700.0))]);
  }

  #[test]
  fn shared_sfnt_bytes_reach_the_font_system_uncopied() {
    let sfnt = FontBytes::Inline(Cow::Owned(geist_bytes()))
      .into_sfnt()
      .unwrap();
    let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(sfnt);
    let address = (*bytes).as_ref().as_ptr();
    let blob = blob(FontSource::from_shared(Arc::clone(&bytes)));

    assert_eq!(blob.data().as_ptr(), address);
  }

  #[test]
  fn static_bytes_keep_one_blob_id_without_reading_them() {
    static TTF: &[u8] = &[0x00, 0x01, 0x00, 0x00];
    static OTF: &[u8] = b"OTTO";

    let id = |bytes| blob(FontSource::from_static(bytes)).id();

    // Same slice, same id, so a re-registered face keeps the glyphs it resolved.
    assert_eq!(id(TTF), id(TTF));
    assert_ne!(id(TTF), id(OTF));
  }

  #[test]
  fn static_sfnt_bytes_reach_the_font_system_uncopied() {
    let sfnt: &'static [u8] = FontBytes::Inline(Cow::Owned(geist_bytes()))
      .into_sfnt()
      .unwrap()
      .leak();
    let blob = blob(FontSource::from_static(sfnt));

    assert_eq!(blob.data().as_ptr(), sfnt.as_ptr());
  }

  #[test]
  fn a_static_woff2_face_keeps_its_id_through_decompression() {
    let woff2: &'static [u8] = geist_bytes().leak();
    let registered = blob(FontSource::from_static(woff2));
    let resolved_first = blob(resolved(FontSource::from_static(woff2)));

    // The bytes the font system holds are the decompressed sfnt either way, so only
    // the id carried past decompression keeps the two from being separate faces.
    assert_ne!(registered.data().as_ptr(), woff2.as_ptr());
    assert_eq!(registered.id(), resolved_first.id());
  }

  #[test]
  fn shared_woff2_bytes_still_decompress() {
    let mut fonts = Fonts::default();
    let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(geist_bytes());
    let families = fonts
      .register(
        FontResource::new(FontSource::from_shared(bytes)).override_info(FontOverride {
          family_name: Some("Shared Geist".into()),
          ..Default::default()
        }),
      )
      .unwrap();

    assert_eq!(families[0].name, "Shared Geist");
    assert!(!families[0].faces.is_empty());
  }

  /// Concatenates sfnt fonts into a ttc, rebasing each table directory's offsets.
  fn build_ttc(fonts: &[Vec<u8>]) -> Vec<u8> {
    let header_len = 12 + 4 * fonts.len();
    let mut ttc = Vec::new();
    ttc.extend_from_slice(b"ttcf");
    ttc.extend_from_slice(&1u16.to_be_bytes());
    ttc.extend_from_slice(&0u16.to_be_bytes());
    ttc.extend_from_slice(&(fonts.len() as u32).to_be_bytes());

    let mut base = header_len as u32;
    for font in fonts {
      ttc.extend_from_slice(&base.to_be_bytes());
      base += font.len() as u32;
    }

    for font in fonts {
      let base = ttc.len() as u32;
      let mut font = font.clone();
      let table_count = u16::from_be_bytes([font[4], font[5]]) as usize;
      for entry in 0..table_count {
        let offset_at = 12 + entry * 16 + 8;
        let offset = u32::from_be_bytes(font[offset_at..offset_at + 4].try_into().unwrap());
        font[offset_at..offset_at + 4].copy_from_slice(&(offset + base).to_be_bytes());
      }
      ttc.extend_from_slice(&font);
    }

    ttc
  }

  #[test]
  fn multi_family_file_registers_in_face_order() {
    let geist = FontBytes::Inline(Cow::Owned(geist_bytes()))
      .into_sfnt()
      .unwrap();
    let mono = FontBytes::Inline(Cow::Owned(geist_mono_bytes()))
      .into_sfnt()
      .unwrap();
    let ttc = build_ttc(&[geist, mono]);

    for _ in 0..32 {
      let mut fonts = Fonts::default();
      let names: Vec<String> = fonts
        .register(FontResource::new(ttc.clone()))
        .unwrap()
        .into_iter()
        .map(|family| family.name)
        .collect();

      assert_eq!(names, ["Geist", "Geist Mono"]);
      assert_eq!(order(&fonts), names);
    }
  }

  #[test]
  fn registering_same_bytes_twice_is_idempotent_in_order() {
    let mut fonts = Fonts::default();
    let first = register_named(&mut fonts, geist_bytes(), "Geist Test");
    let second = register_named(&mut fonts, geist_bytes(), "Geist Test");

    assert_eq!(format!("{second:?}"), format!("{first:?}"));

    // Registration order dedups by name: registering the same family name twice does not
    // list it twice, so default fallback selection stays stable.
    assert_eq!(
      order(&fonts)
        .iter()
        .filter(|name| *name == "Geist Test")
        .count(),
      1
    );
  }

  #[test]
  fn font_override_family_name_renames_family() {
    let mut fonts = Fonts::default();
    let families = register_named(&mut fonts, geist_bytes(), "Renamed Family");

    assert_eq!(families[0].name, "Renamed Family");
    assert!(order(&fonts).contains(&"Renamed Family".to_string()));
  }

  #[test]
  fn ranged_subsets_join_one_family() {
    let mut fonts = Fonts::default();

    for (bytes, range) in [
      (geist_bytes(), UnicodeRange::new(0x41, 0x5A)),
      (geist_mono_bytes(), UnicodeRange::new(0x61, 0x7A)),
    ] {
      let families = fonts
        .register(
          FontResource::new(bytes)
            .override_info(FontOverride {
              family_name: Some("Logical".into()),
              ..Default::default()
            })
            .unicode_range(UnicodeRangeSet::from_iter([range])),
        )
        .unwrap();

      assert_eq!(families[0].name, "Logical");
    }

    assert_eq!(order(&fonts), vec!["Logical".to_string()]);
    assert_eq!(
      fonts
        .registry
        .selector
        .font_faces()
        .filter(|face| !face.ranges().is_entire_range())
        .count(),
      2
    );
  }

  #[test]
  fn registration_order_defines_default_fallbacks() {
    let mut fonts = Fonts::default();
    register_named(&mut fonts, geist_bytes(), "B Family");
    register_named(&mut fonts, geist_mono_bytes(), "A Family");

    assert_eq!(
      order(&fonts),
      vec!["B Family".to_string(), "A Family".to_string()]
    );
  }

  #[test]
  fn last_resort_families_sort_after_normal_registrations() {
    let mut fonts = Fonts::default();
    fonts
      .register(
        FontResource::new(geist_bytes())
          .override_info(FontOverride {
            family_name: Some("Embedded".into()),
            ..Default::default()
          })
          .last_resort(),
      )
      .unwrap();
    register_named(&mut fonts, geist_mono_bytes(), "User Font");

    // Registered first, but selected last: a last-resort family never shadows caller fonts.
    assert_eq!(order(&fonts), vec!["User Font".to_string()]);
    assert_eq!(
      fonts
        .registry
        .selector
        .last_resort_families()
        .map(ToString::to_string)
        .collect::<Vec<_>>(),
      vec!["Embedded".to_string()]
    );
  }

  #[test]
  fn a_resolved_subset_holds_what_registering_it_would() {
    let basic_latin =
      || -> UnicodeRangeSet { [UnicodeRange::new(0x20, 0x7e)].into_iter().collect() };
    let (data, id) = FontSource::from(geist_bytes()).into_sfnt_with_id().unwrap();
    let (expected, expected_id) = basic_latin().restrict_sfnt(data, id);
    let resolved = FontResource::new(geist_bytes())
      .unicode_range(basic_latin())
      .into_resolved()
      .unwrap();
    let again = resolved.into_resolved().unwrap();
    let (data, id) = again.source.into_sfnt_with_id().unwrap();

    assert_eq!(id, expected_id);
    assert_eq!((*data).as_ref(), (*expected).as_ref());
  }

  #[test]
  fn same_bytes_produce_the_same_blob_id() {
    let first = blob(FontSource::from(geist_bytes()));
    // Decoded up front rather than on registration, so the id has to come from the
    // decoded sfnt and not from whatever the caller happened to hand over.
    let second = blob(resolved(FontSource::from(geist_bytes())));
    let other = blob(FontSource::from(geist_mono_bytes()));

    // The glyph caches key on this id, so a re-registered face has to keep the entries
    // it already resolved.
    assert_eq!(first.id(), second.id());
    assert_ne!(first.id(), other.id());
  }

  #[test]
  fn unknown_family_in_fallbacks_does_not_panic() {
    let fonts = Fonts::default();
    let unknown = FontFamily::from_css_str("Never Registered").unwrap();

    // Must not panic: an unresolved name simply yields an empty fallback bucket.
    let _snapshot = fonts.snapshot_with_fallbacks(Some(&unknown));
  }
}
