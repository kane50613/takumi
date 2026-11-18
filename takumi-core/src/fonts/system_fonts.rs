//! The platform fonts behind Blink's [`FontCache`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/font_cache.h):
//! what `CSSFontSelector::GetFontData` looks a family up in when no registered face has it, and
//! what per-character fallback reaches after every listed family.
//!
//! Approximate: takumi reads no fonts from the platform, so the system fonts are the ones the host
//! installs through [`crate::Fonts::with_system_fonts`]. Fallback tries them family by family in
//! install order, for every script.

use std::sync::Arc;

use super::{
  css_segmented_font_face::FontPlatformDataForRangeSet,
  font_description::FontSelectionDescription,
  font_face::{FaceData, FontFace, FontFaceDescriptors},
  font_face_cache::{FontFaceCache, insert_family},
};
use crate::resources::font::SharedBytes;

/// Blink's platform fonts, which the host installs once and no registration changes.
#[derive(Debug, Default)]
pub(crate) struct SystemFonts {
  font_face_cache: FontFaceCache,
  /// Every installed face, in install order.
  font_faces: Vec<Arc<FontFace>>,
  /// Each installed family once, in the order it first installed.
  families: Vec<Arc<str>>,
}

impl SystemFonts {
  /// Installs each usable face of `data` under `descriptors`.
  pub(crate) fn install(&mut self, data: &SharedBytes, id: u64, descriptors: &FontFaceDescriptors) {
    for font_face in FontFace::from_data(data, id, descriptors, |_| None) {
      let font_face = Arc::new(font_face);

      insert_family(&mut self.families, &font_face.family);
      self.font_face_cache.add_font_face(Arc::clone(&font_face));
      self.font_faces.push(font_face);
    }
  }

  /// Blink's `FontCache::GetFontData`: the faces of `family` matching `font_description`.
  pub(crate) fn get_font_data(
    &self,
    font_description: &FontSelectionDescription,
    family: &str,
  ) -> Option<Vec<FaceData<FontPlatformDataForRangeSet>>> {
    self.font_face_cache.get_font_data(font_description, family)
  }

  /// The faces matching `font_description`, family by family, which fallback tries after every
  /// listed family.
  pub(crate) fn fallback<'a>(
    &'a self,
    font_description: &'a FontSelectionDescription,
  ) -> impl Iterator<Item = Vec<FaceData<FontPlatformDataForRangeSet>>> + 'a {
    self
      .families
      .iter()
      .filter_map(|family| self.get_font_data(font_description, family))
  }

  /// Every installed face, in install order.
  pub(crate) fn font_faces(&self) -> impl Iterator<Item = &Arc<FontFace>> + '_ {
    self.font_faces.iter()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::fonts::{
    font_custom_platform_data::FontCustomPlatformData,
    font_selection_types::{
      FontSelectionRequest, NORMAL_SLOPE_VALUE, NORMAL_WEIGHT_VALUE, NORMAL_WIDTH_VALUE,
    },
  };

  #[test]
  fn fallback_walks_each_family_once_in_install_order() {
    let mut system_fonts = SystemFonts::default();
    let description = FontSelectionDescription::for_request(FontSelectionRequest::new(
      NORMAL_WEIGHT_VALUE,
      NORMAL_WIDTH_VALUE,
      NORMAL_SLOPE_VALUE,
    ));

    for (path, family) in [
      ("../assets/fonts/geist/GeistMono[wght].woff2", "Mono"),
      ("../assets/fonts/geist/Geist[wght].woff2", "Sans"),
      ("../assets/fonts/geist/geist-latin-wght-300-800.ttf", "mono"),
    ] {
      let (data, id) = FontCustomPlatformData::read_asset(path);

      system_fonts.install(
        &data,
        id,
        &FontFaceDescriptors {
          family: Some(family.into()),
          ..Default::default()
        },
      );
    }

    assert_eq!(system_fonts.font_faces().count(), 3);
    assert_eq!(system_fonts.fallback(&description).count(), 2);
    assert!(system_fonts.get_font_data(&description, "SANS").is_some());
    assert!(system_fonts.get_font_data(&description, "Serif").is_none());
  }
}
