use std::{
  borrow::Cow,
  collections::{HashMap, HashSet},
  hash::Hash,
  iter::once,
  ops::{Deref, DerefMut},
  sync::Arc,
};

use parley::{
  FontStyle, GenericFamily, GlyphRun, LayoutContext, TextStyle, TreeBuilder,
  fontique::{Blob, Collection, CollectionOptions, FallbackKey, FontInfoOverride, Script},
};
use swash::{
  FontRef,
  scale::{ScaleContext, StrikeWith, image::Image, outline::Outline},
};
use thiserror::Error;
use xxhash_rust::xxh3::xxh3_64;
use zeno::{Angle as ZenoAngle, Transform as ZenoTransform};

use crate::{
  Xxh3HashSet,
  layout::inline::{InlineBrush, InlineLayout},
};

/// Represents a resolved glyph that can be either a bitmap image or an outline
#[derive(Clone)]
pub(crate) enum ResolvedGlyph {
  /// A bitmap glyph image
  Image(Image),
  /// A vector outline glyph
  Outline(Outline),
}

/// Matches the typical faux-bold expansion used by text rasterizers.
const SYNTHESIS_EMBOLDEN_FACTOR: f32 = 1.0 / 24.0;

pub(crate) fn synthesis_embolden_strength(font_size: f32) -> f32 {
  font_size * SYNTHESIS_EMBOLDEN_FACTOR
}

/// Errors that can occur during font loading and conversion.
#[derive(Debug, Error)]
pub enum FontError {
  /// Error occurred during WOFF conversion
  #[cfg(any(feature = "woff", feature = "woff2"))]
  #[error("Error occurred during WOFF conversion.")]
  Woff(wuff::WuffErr),
  /// Unsupported Font Format
  #[error("Unsupported font format")]
  UnsupportedFormat,
  /// Font index is invalid
  #[error("Font index is invalid")]
  InvalidFontIndex,
}

/// Supported font formats for loading and processing
#[derive(Copy, Clone)]
pub enum FontFormat {
  #[cfg(feature = "woff")]
  /// Web Open Font Format (WOFF) - compressed web font format
  Woff,
  #[cfg(feature = "woff2")]
  /// Web Open Font Format 2 (WOFF2) - improved compression web font format
  Woff2,
  /// TrueType Font format - standard desktop font format
  Ttf,
  /// OpenType Font format - extended font format with advanced typography
  Otf,
  /// TrueType Collection - multiple fonts in one file; the first font is extracted
  Ttc,
}

/// Loads and processes font data, optionally using format hint for detection
pub fn load_font(
  source: Cow<'_, [u8]>,
  format_hint: Option<FontFormat>,
) -> Result<Vec<u8>, FontError> {
  let format = if let Some(format) = format_hint {
    format
  } else {
    guess_font_format(&source)?
  };

  match format {
    FontFormat::Ttf | FontFormat::Otf => Ok(source.into_owned()),
    FontFormat::Ttc => extract_ttf_from_ttc(&source, 0),
    #[cfg(feature = "woff2")]
    FontFormat::Woff2 => {
      let ttf = wuff::decompress_woff2(&source).map_err(FontError::Woff)?;
      Ok(ttf)
    }
    #[cfg(feature = "woff")]
    FontFormat::Woff => {
      let ttf = wuff::decompress_woff1(&source).map_err(FontError::Woff)?;
      Ok(ttf)
    }
  }
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

fn read_u32_be(source: &[u8], offset: usize) -> Option<u32> {
  source
    .get(offset..offset + 4)
    .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

fn read_u16_be(source: &[u8], offset: usize) -> Option<u16> {
  source
    .get(offset..offset + 2)
    .map(|b| u16::from_be_bytes([b[0], b[1]]))
}

/// Extracts a single TrueType font from a TrueType Collection (.ttc) buffer.
///
/// TTC files store font tables at absolute offsets from the start of the file.
/// This function rebuilds the selected font as a standalone TTF by copying each
/// table and rewriting the offsets relative to the new buffer.
fn extract_ttf_from_ttc(source: &[u8], font_index: usize) -> Result<Vec<u8>, FontError> {
  let num_fonts = read_u32_be(source, 8).ok_or(FontError::UnsupportedFormat)? as usize;
  if font_index >= num_fonts {
    return Err(FontError::InvalidFontIndex);
  }

  let font_offset =
    read_u32_be(source, 12 + font_index * 4).ok_or(FontError::UnsupportedFormat)? as usize;

  let sf_version = read_u32_be(source, font_offset).ok_or(FontError::UnsupportedFormat)?;
  let num_tables =
    read_u16_be(source, font_offset + 4).ok_or(FontError::UnsupportedFormat)? as usize;
  let search_range = read_u16_be(source, font_offset + 6).ok_or(FontError::UnsupportedFormat)?;
  let entry_selector =
    read_u16_be(source, font_offset + 8).ok_or(FontError::UnsupportedFormat)?;
  let range_shift = read_u16_be(source, font_offset + 10).ok_or(FontError::UnsupportedFormat)?;

  struct TableRecord {
    tag: [u8; 4],
    check_sum: u32,
    offset: usize,
    length: usize,
  }

  let mut records = Vec::with_capacity(num_tables);
  for i in 0..num_tables {
    let ro = font_offset + 12 + i * 16;
    let tag = source.get(ro..ro + 4).ok_or(FontError::UnsupportedFormat)?;
    let check_sum = read_u32_be(source, ro + 4).ok_or(FontError::UnsupportedFormat)?;
    let offset = read_u32_be(source, ro + 8).ok_or(FontError::UnsupportedFormat)? as usize;
    let length = read_u32_be(source, ro + 12).ok_or(FontError::UnsupportedFormat)? as usize;
    records.push(TableRecord {
      tag: [tag[0], tag[1], tag[2], tag[3]],
      check_sum,
      offset,
      length,
    });
  }

  // Assign new 4-byte-aligned offsets relative to the start of the output TTF
  let header_size = 12 + num_tables * 16;
  let mut pos = header_size;
  let new_offsets: Vec<usize> = records
    .iter()
    .map(|r| {
      let new_offset = (pos + 3) & !3;
      pos = new_offset + ((r.length + 3) & !3);
      new_offset
    })
    .collect();

  let mut out = vec![0u8; pos];

  // Write sfnt header
  out[0..4].copy_from_slice(&sf_version.to_be_bytes());
  out[4..6].copy_from_slice(&(num_tables as u16).to_be_bytes());
  out[6..8].copy_from_slice(&search_range.to_be_bytes());
  out[8..10].copy_from_slice(&entry_selector.to_be_bytes());
  out[10..12].copy_from_slice(&range_shift.to_be_bytes());

  // Write table records and copy table data
  for (i, (record, &new_offset)) in records.iter().zip(new_offsets.iter()).enumerate() {
    let ro = 12 + i * 16;
    out[ro..ro + 4].copy_from_slice(&record.tag);
    out[ro + 4..ro + 8].copy_from_slice(&record.check_sum.to_be_bytes());
    out[ro + 8..ro + 12].copy_from_slice(&(new_offset as u32).to_be_bytes());
    out[ro + 12..ro + 16].copy_from_slice(&(record.length as u32).to_be_bytes());

    let src_end = record.offset + record.length;
    if src_end > source.len() {
      return Err(FontError::UnsupportedFormat);
    }
    out[new_offset..new_offset + record.length]
      .copy_from_slice(&source[record.offset..src_end]);
  }

  Ok(out)
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Builds a minimal but valid TTC containing one TTF-like font with a single
  /// dummy table, so we can test extraction without needing a real font file.
  fn make_minimal_ttc() -> Vec<u8> {
    // Dummy table: 8 bytes of payload
    let table_data: &[u8] = b"TAKUMITC";
    let table_tag = b"test";
    let table_len = table_data.len();

    // sfnt header: sfVersion(4) + numTables(2) + searchRange(2) +
    //              entrySelector(2) + rangeShift(2) = 12 bytes
    // table record: tag(4) + checkSum(4) + offset(4) + length(4) = 16 bytes
    // total header: 28 bytes; table data follows at offset 28 (already 4-aligned)
    let sfnt_offset_in_ttc: u32 = 16; // TTC header is 16 bytes (tag+ver+count+1 offset)
    let table_offset_in_ttc: u32 = sfnt_offset_in_ttc + 28; // sfnt header + 1 record

    let mut ttc: Vec<u8> = Vec::new();
    // TTC header
    ttc.extend_from_slice(b"ttcf"); // tag
    ttc.extend_from_slice(&0x00010000u32.to_be_bytes()); // version 1.0
    ttc.extend_from_slice(&1u32.to_be_bytes()); // numFonts = 1
    ttc.extend_from_slice(&sfnt_offset_in_ttc.to_be_bytes()); // offset to font 0

    // sfnt header for font 0
    ttc.extend_from_slice(&0x00010000u32.to_be_bytes()); // sfVersion (TrueType)
    ttc.extend_from_slice(&1u16.to_be_bytes()); // numTables
    ttc.extend_from_slice(&16u16.to_be_bytes()); // searchRange
    ttc.extend_from_slice(&0u16.to_be_bytes()); // entrySelector
    ttc.extend_from_slice(&0u16.to_be_bytes()); // rangeShift

    // table record
    ttc.extend_from_slice(table_tag);
    ttc.extend_from_slice(&0u32.to_be_bytes()); // checkSum
    ttc.extend_from_slice(&table_offset_in_ttc.to_be_bytes()); // absolute offset
    ttc.extend_from_slice(&(table_len as u32).to_be_bytes());

    // table data
    ttc.extend_from_slice(table_data);

    ttc
  }

  #[test]
  fn ttc_is_detected() {
    let ttc = make_minimal_ttc();
    assert!(matches!(
      guess_font_format(&ttc),
      Ok(FontFormat::Ttc)
    ));
  }

  #[test]
  fn ttc_extraction_produces_valid_sfnt_header() {
    let ttc = make_minimal_ttc();
    let ttf = extract_ttf_from_ttc(&ttc, 0).expect("extraction should succeed");

    // sfVersion should be preserved
    assert_eq!(&ttf[0..4], &0x00010000u32.to_be_bytes());
    // numTables
    assert_eq!(u16::from_be_bytes([ttf[4], ttf[5]]), 1);
  }

  #[test]
  fn ttc_extraction_copies_table_data() {
    let ttc = make_minimal_ttc();
    let ttf = extract_ttf_from_ttc(&ttc, 0).expect("extraction should succeed");

    // Table record starts at byte 12; new offset is at bytes 8..12 of that record
    let new_offset = u32::from_be_bytes([ttf[20], ttf[21], ttf[22], ttf[23]]) as usize;
    assert_eq!(&ttf[new_offset..new_offset + 8], b"TAKUMITC");
  }

  #[test]
  fn ttc_extraction_invalid_index_errors() {
    let ttc = make_minimal_ttc();
    assert!(matches!(
      extract_ttf_from_ttc(&ttc, 1),
      Err(FontError::InvalidFontIndex)
    ));
  }

  #[test]
  fn load_font_accepts_ttc() {
    let ttc = make_minimal_ttc();
    // Should not return UnsupportedFormat
    let result = load_font(Cow::Owned(ttc), None);
    assert!(result.is_ok(), "load_font should handle TTC: {result:?}");
  }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(crate) struct FontCacheKey {
  data_hash: u64,
  family_name: Option<Box<str>>,
  style: Option<FontStyleHash>,
  weight: Option<u32>,
  width: Option<u32>,
  axes: Option<Box<[(u32, u32)]>>,
  generic_family: Option<GenericFamily>,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum FontStyleHash {
  Normal,
  Italic,
  Oblique(Option<u32>),
}

impl From<FontStyle> for FontStyleHash {
  fn from(style: FontStyle) -> Self {
    match style {
      FontStyle::Normal => Self::Normal,
      FontStyle::Italic => Self::Italic,
      FontStyle::Oblique(angle) => Self::Oblique(angle.map(f32::to_bits)),
    }
  }
}

/// A context for managing fonts in the rendering system.
#[derive(Clone)]
pub struct FontContext {
  inner: parley::FontContext,
  cache: Xxh3HashSet<FontCacheKey>,
}

impl Default for FontContext {
  fn default() -> Self {
    Self {
      inner: parley::FontContext {
        collection: Collection::new(CollectionOptions {
          system_fonts: false,
          shared: false,
        }),
        source_cache: Default::default(),
      },
      cache: Xxh3HashSet::default(),
    }
  }
}

impl Deref for FontContext {
  type Target = parley::FontContext;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl DerefMut for FontContext {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}

impl FontContext {
  pub(crate) fn resolve_glyphs(
    &self,
    run: &GlyphRun<'_, InlineBrush>,
    font_ref: FontRef,
    glyph_ids: impl Iterator<Item = u32> + Clone,
  ) -> HashMap<u32, ResolvedGlyph> {
    // Collect unique glyph IDs to avoid duplicate work
    let unique_glyph_ids: HashSet<u32> = glyph_ids.collect();

    let mut result = HashMap::new();

    if unique_glyph_ids.is_empty() {
      return result;
    }

    let mut scale = ScaleContext::with_max_entries(0);
    let mut scaler = scale
      .builder(font_ref)
      .size(run.run().font_size())
      .normalized_coords(run.run().normalized_coords())
      .build();

    let has_emoji_cluster = run
      .run()
      .visual_clusters()
      .any(|cluster| cluster.is_emoji());
    let embolden = if !has_emoji_cluster
      && run.run().synthesis().embolden()
      && run.style().brush.font_synthesis.weight.is_allowed()
    {
      Some(synthesis_embolden_strength(run.run().font_size()))
    } else {
      None
    };
    let skew = run
      .run()
      .synthesis()
      .skew()
      .filter(|_| !has_emoji_cluster)
      .filter(|_| run.style().brush.font_synthesis.style.is_allowed())
      .map(|degrees| ZenoTransform::skew(ZenoAngle::from_degrees(degrees), ZenoAngle::ZERO));

    // Process each unique glyph ID
    for &glyph_id in &unique_glyph_ids {
      let mut resolved = scaler
        .scale_color_bitmap(glyph_id as u16, StrikeWith::BestFit)
        .map(|image| (ResolvedGlyph::Image(image), false))
        .or_else(|| {
          scaler
            .scale_color_outline(glyph_id as u16)
            .map(|outline| (ResolvedGlyph::Outline(outline), false))
        })
        .or_else(|| {
          scaler
            .scale_outline(glyph_id as u16)
            .map(|outline| (ResolvedGlyph::Outline(outline), true))
        });

      if let Some(embolden_strength) = embolden
        && let Some((ResolvedGlyph::Outline(ref mut outline), true)) = resolved
      {
        outline.embolden(embolden_strength, embolden_strength);
      }
      if let Some(ref skew_transform) = skew
        && let Some((ResolvedGlyph::Outline(ref mut outline), true)) = resolved
      {
        outline.transform(skew_transform);
      }

      if let Some((glyph, _)) = resolved {
        result.insert(glyph_id, glyph);
      }
    }

    result
  }

  /// Create an inline layout with the given root style and function
  pub(crate) fn tree_builder(
    &self,
    root_style: TextStyle<'_, InlineBrush>,
    func: impl FnOnce(&mut TreeBuilder<'_, InlineBrush>),
  ) -> (InlineLayout, String) {
    let mut font_context = self.clone();
    let mut layout_context = LayoutContext::new();

    let mut builder = layout_context.tree_builder(&mut font_context, 1.0, true, &root_style);

    func(&mut builder);

    builder.build()
  }

  /// Loads font into internal font db with caching
  pub fn load_and_store(
    &mut self,
    source: Cow<'_, [u8]>,
    info_override: Option<FontInfoOverride<'_>>,
    generic_family: Option<GenericFamily>,
  ) -> Result<(), FontError> {
    let cache_key = FontCacheKey {
      data_hash: xxh3_64(&source),
      family_name: info_override
        .and_then(|info| info.family_name)
        .map(Into::into),
      style: info_override.and_then(|info| info.style).map(Into::into),
      weight: info_override
        .and_then(|info| info.weight)
        .map(|weight| weight.value().to_bits()),
      width: info_override
        .and_then(|info| info.width)
        .map(|width| width.ratio().to_bits()),
      axes: info_override.and_then(|info| info.axes).map(|axes| {
        axes
          .iter()
          .map(|(tag, value)| (u32::from_be_bytes(tag.to_be_bytes()), value.to_bits()))
          .collect()
      }),
      generic_family,
    };

    if self.cache.contains(&cache_key) {
      return Ok(());
    }

    let font_data = Blob::new(Arc::new(load_font(source, None)?));

    let fonts = self
      .inner
      .collection
      .register_fonts(font_data, info_override);

    for (family, _) in fonts {
      if let Some(generic_family) = generic_family {
        self
          .inner
          .collection
          .append_generic_families(generic_family, once(family));
      }

      for (script, _) in Script::all_samples() {
        self
          .inner
          .collection
          .append_fallbacks(FallbackKey::new(*script, None), once(family));
      }
    }

    self.cache.insert(cache_key);

    Ok(())
  }
}
