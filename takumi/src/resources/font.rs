use std::{
  borrow::Cow,
  collections::{HashMap, HashSet},
  ops::{Deref, DerefMut},
  sync::Arc,
};

use parley::{
  GenericFamily, LayoutContext, Run, TextStyle, TreeBuilder,
  fontique::{Blob, Collection, CollectionOptions, FallbackKey, FontInfoOverride, Script},
};
use swash::{
  FontRef,
  scale::{ScaleContext, StrikeWith, image::Image, outline::Outline},
};
use thiserror::Error;

use crate::layout::inline::{InlineBrush, InlineLayout};

/// Represents a resolved glyph that can be either a bitmap image or an outline
#[derive(Clone)]
pub(crate) enum ResolvedGlyph {
  /// A bitmap glyph image
  Image(Image),
  /// A vector outline glyph
  Outline(Outline),
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
}

/// Loads and processes font data from raw bytes, optionally using format hint for detection
pub fn load_font<'source>(
  source: &'source [u8],
  format_hint: Option<FontFormat>,
) -> Result<Cow<'source, [u8]>, FontError> {
  let format = if let Some(format) = format_hint {
    format
  } else {
    guess_font_format(source)?
  };

  match format {
    FontFormat::Ttf | FontFormat::Otf => Ok(Cow::Borrowed(source)),
    #[cfg(feature = "woff2")]
    FontFormat::Woff2 => {
      let ttf = wuff::decompress_woff2(source).map_err(FontError::Woff)?;
      Ok(Cow::Owned(ttf))
    }
    #[cfg(feature = "woff")]
    FontFormat::Woff => {
      let ttf = wuff::decompress_woff1(source).map_err(FontError::Woff)?;
      Ok(Cow::Owned(ttf))
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
    _ => Err(FontError::UnsupportedFormat),
  }
}

/// A context for managing fonts in the rendering system.
#[derive(Clone)]
pub struct FontContext(parley::FontContext);

impl Default for FontContext {
  fn default() -> Self {
    Self(parley::FontContext {
      collection: Collection::new(CollectionOptions {
        system_fonts: false,
        shared: false,
      }),
      source_cache: Default::default(),
    })
  }
}

impl Deref for FontContext {
  type Target = parley::FontContext;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for FontContext {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl FontContext {
  pub(crate) fn resolve_glyphs(
    &self,
    run: &Run<'_, InlineBrush>,
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
      .size(run.font_size())
      .normalized_coords(run.normalized_coords())
      .build();

    // Process each unique glyph ID
    for &glyph_id in &unique_glyph_ids {
      let resolved = scaler
        .scale_color_bitmap(glyph_id as u16, StrikeWith::BestFit)
        .map(ResolvedGlyph::Image)
        .or_else(|| {
          scaler
            .scale_color_outline(glyph_id as u16)
            .map(ResolvedGlyph::Outline)
        })
        .or_else(|| {
          scaler
            .scale_outline(glyph_id as u16)
            .map(ResolvedGlyph::Outline)
        });

      if let Some(glyph) = resolved {
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

  /// Loads font into internal font db
  pub fn load_and_store(
    &mut self,
    source: &[u8],
    info_override: Option<FontInfoOverride<'_>>,
    generic_family: Option<GenericFamily>,
  ) -> Result<(), FontError> {
    let font_data = Blob::new(Arc::new(match load_font(source, None)? {
      Cow::Owned(vec) => vec,
      Cow::Borrowed(slice) => slice.to_vec(),
    }));

    let fonts = self.0.collection.register_fonts(font_data, info_override);

    for (family, _) in fonts {
      if let Some(generic_family) = generic_family {
        self
          .collection
          .append_generic_families(generic_family, std::iter::once(family));
      }

      for (script, _) in Script::all_samples() {
        self
          .collection
          .append_fallbacks(FallbackKey::new(*script, None), std::iter::once(family));
      }
    }

    Ok(())
  }
}
