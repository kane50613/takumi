//! Blink's [`CSSSegmentedFontFace`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/css_segmented_font_face.cc):
//! the faces of one family that share capabilities and differ by `unicode-range`.
//!
//! Every registered face is one Blink adds from script, so `FontFaceList` keeps only its
//! non-CSS-connected part. Blink's `font_data_table_` cache is left out: the registry is shared
//! immutably across renders.

use std::sync::Arc;

use super::{
  font_custom_platform_data::FontPlatformData,
  font_description::FontSelectionDescription,
  font_face::FontFace,
  font_selection_types::{BOLD_THRESHOLD, FontSelectionCapabilities, ITALIC_SLOPE_VALUE},
};
use crate::resources::unicode_range::UnicodeRangeSet;

/// Blink's `FontDataForRangeSet` before its face becomes a `SimpleFontData`.
#[derive(Clone, Debug)]
pub(crate) struct FontPlatformDataForRangeSet {
  pub(crate) font_platform_data: FontPlatformData,
  pub(crate) range_set: UnicodeRangeSet,
}

/// Blink's `CSSSegmentedFontFace`.
#[derive(Debug)]
pub(crate) struct CssSegmentedFontFace {
  font_selection_capabilities: FontSelectionCapabilities,
  /// Blink's `FontFaceList`, in insertion order.
  font_faces: Vec<Arc<FontFace>>,
}

impl CssSegmentedFontFace {
  pub(crate) fn new(font_selection_capabilities: FontSelectionCapabilities) -> Self {
    Self {
      font_selection_capabilities,
      font_faces: Vec::new(),
    }
  }

  /// Blink's `GetFontSelectionCapabilities`.
  pub(crate) fn get_font_selection_capabilities(&self) -> FontSelectionCapabilities {
    self.font_selection_capabilities
  }

  /// Blink's `AddFontFace`.
  pub(crate) fn add_font_face(&mut self, font_face: Arc<FontFace>) {
    self.font_faces.push(font_face);
  }

  /// Blink's `RemoveFontFace`.
  pub(crate) fn remove_font_face(&mut self, font_face: &Arc<FontFace>) -> bool {
    let Some(index) = self
      .font_faces
      .iter()
      .position(|each| Arc::ptr_eq(each, font_face))
    else {
      return false;
    };

    self.font_faces.remove(index);
    true
  }

  /// Blink's `IsEmpty`.
  pub(crate) fn is_empty(&self) -> bool {
    self.font_faces.is_empty()
  }

  /// Blink's `GetFontData`: every face at the request, the last added first, as later
  /// `unicode-range` faces override earlier ones. <https://drafts.csswg.org/css-fonts/#composite-fonts>
  pub(crate) fn get_font_data(
    &self,
    font_description: &FontSelectionDescription,
  ) -> Option<Vec<FontPlatformDataForRangeSet>> {
    let mut requested_font_description = font_description.clone();
    let font_selection_request = font_description.font_selection_request;
    let capabilities = self.font_selection_capabilities;

    requested_font_description.is_synthetic_bold = capabilities.weight.maximum < BOLD_THRESHOLD
      && font_selection_request.weight >= BOLD_THRESHOLD
      && font_description.synthetic_bold_allowed;
    requested_font_description.is_synthetic_italic = capabilities.slope.maximum
      < ITALIC_SLOPE_VALUE
      && font_selection_request.slope >= ITALIC_SLOPE_VALUE
      && font_description.synthetic_italic_allowed;

    let created_font_data: Vec<FontPlatformDataForRangeSet> = self
      .font_faces
      .iter()
      .rev()
      .map(|font_face| FontPlatformDataForRangeSet {
        font_platform_data: font_face.get_font_data(&requested_font_description),
        range_set: font_face.ranges().clone(),
      })
      .collect();

    (!created_font_data.is_empty()).then_some(created_font_data)
  }
}
