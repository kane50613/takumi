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
  /// TrueType Collection - multiple fonts in one file
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
    FontFormat::Ttf | FontFormat::Otf | FontFormat::Ttc => Ok(source.into_owned()),
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
