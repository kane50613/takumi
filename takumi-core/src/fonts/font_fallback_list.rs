//! Blink's `FontFallbackList`, the fonts the `font-family` list resolves to, after
//! [`font_fallback_list.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/font_fallback_list.cc).
//!
//! Blink resolves each family as fallback reaches it; this resolves them all at once, with the same
//! fonts in the same order. Blink's standard font is the generic `serif` family here. Follows Blink
//! under the notice in LICENSE-CHROMIUM.

use std::rc::Rc;

use parley::{GenericFamily, fontique::QueryFamily};

use super::{
  font_cache::FontCache, font_data_for_range_set::FontDataForRangeSet,
  font_description::FontDescription, simple_font_data::SimpleFontData,
};
use crate::resources::font::{Fonts, MatchedFace};

/// Blink's `FontData`: one face, or the faces a `unicode-range` splits a family into.
#[derive(Debug)]
pub(crate) enum FontData {
  Simple(Rc<SimpleFontData>),
  Segmented(Vec<FontDataForRangeSet>),
}

impl FontData {
  /// Blink's `IsSegmented`.
  pub(crate) fn is_segmented(&self) -> bool {
    matches!(self, Self::Segmented(_))
  }
}

/// Blink's `FontFallbackList`, with the `FontCache` it falls back to past it.
#[derive(Debug)]
pub(crate) struct FontFallbackList {
  font_list: Vec<FontData>,
  font_cache: FontCache,
}

impl FontFallbackList {
  pub(crate) fn new(fonts: &mut Fonts, description: &FontDescription) -> Self {
    let font_data = |face: MatchedFace| {
      SimpleFontData::new(
        face,
        description.computed_size,
        description.specified_size,
        &description.variation_settings,
      )
      .map(Rc::new)
    };
    let resolve = |faces: Vec<MatchedFace>| {
      // A family split by `unicode-range` stays segmented even with one face.
      if let [face] = faces.as_slice()
        && face.unicode_range.is_none()
      {
        return font_data(face.clone()).map(FontData::Simple);
      }

      let faces: Vec<FontDataForRangeSet> = faces
        .into_iter()
        .filter_map(font_data)
        .map(FontDataForRangeSet::new)
        .collect();

      (!faces.is_empty()).then_some(FontData::Segmented(faces))
    };
    let mut font_list: Vec<FontData> = fonts
      .font_group(description.family.query_families(), description.attributes)
      .into_iter()
      .filter_map(resolve)
      .collect();
    let font_cache = FontCache::new(
      fonts
        .fallback_bucket(description.attributes)
        .into_iter()
        .filter_map(font_data)
        .collect(),
    );
    // Past the families, Blink takes the standard font, else the last resort font.
    let standard = fonts
      .font_group(
        [QueryFamily::Generic(GenericFamily::Serif)],
        description.attributes,
      )
      .into_iter()
      .find_map(resolve)
      .or_else(|| font_cache.last_resort_fallback_font().map(FontData::Simple));

    font_list.extend(standard);

    Self {
      font_list,
      font_cache,
    }
  }

  /// Blink's `FontDataAt`.
  pub(crate) fn font_data_at(&self, realized_font_index: usize) -> Option<&FontData> {
    self.font_list.get(realized_font_index)
  }

  pub(crate) fn font_cache(&self) -> &FontCache {
    &self.font_cache
  }
}
