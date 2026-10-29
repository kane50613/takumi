//! Blink's [`CSSSegmentedFontFace`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/css/css_segmented_font_face.cc):
//! the faces of one family that share capabilities and differ by `unicode-range`.
//!
//! A face registered through [`crate::Fonts`] is one Blink adds from script, and a face an
//! `@font-face` rule makes is CSS-connected. Blink's `font_data_table_` cache is left out: the
//! registry is shared immutably across renders.

use std::sync::Arc;

use super::{
  font_custom_platform_data::FontPlatformData,
  font_description::FontSelectionDescription,
  font_face::{FaceData, FontFace},
  font_selection_types::{BOLD_THRESHOLD, FontSelectionCapabilities, ITALIC_SLOPE_VALUE},
};
use crate::resources::unicode_range::UnicodeRangeSet;

/// Blink's `FontDataForRangeSet` before its face becomes a `SimpleFontData`.
#[derive(Clone, Debug)]
pub(crate) struct FontPlatformDataForRangeSet {
  pub(crate) font_platform_data: FontPlatformData,
  pub(crate) range_set: UnicodeRangeSet,
}

impl FaceData<FontPlatformDataForRangeSet> {
  /// Blink's `FontDataForRangeSet::Ranges`.
  pub(crate) fn range_set(&self) -> &UnicodeRangeSet {
    match self {
      Self::Loaded(face) => &face.range_set,
      Self::Pending(face) => &face.range_set,
    }
  }
}

/// Blink's `CSSSegmentedFontFace`.
#[derive(Clone, Debug)]
pub(crate) struct CssSegmentedFontFace {
  font_selection_capabilities: FontSelectionCapabilities,
  font_faces: FontFaceList,
}

/// Blink's `FontFaceList`: CSS-connected faces in cascade order, then the faces added from script.
#[derive(Clone, Debug, Default)]
struct FontFaceList {
  css_connected_face: Vec<Arc<FontFace>>,
  non_css_connected_face: Vec<Arc<FontFace>>,
}

impl FontFaceList {
  /// Blink's `Insert`. The CSS-connected faces arrive in cascade order, so each goes last.
  fn insert(&mut self, font_face: Arc<FontFace>) {
    if font_face.css_connected {
      self.css_connected_face.push(font_face);
    } else {
      self.non_css_connected_face.push(font_face);
    }
  }

  /// Blink's `Erase`.
  fn erase(&mut self, font_face: &Arc<FontFace>) -> bool {
    [
      &mut self.css_connected_face,
      &mut self.non_css_connected_face,
    ]
    .into_iter()
    .any(|part| {
      let Some(index) = part.iter().position(|each| Arc::ptr_eq(each, font_face)) else {
        return false;
      };

      part.remove(index);
      true
    })
  }

  fn is_empty(&self) -> bool {
    self.css_connected_face.is_empty() && self.non_css_connected_face.is_empty()
  }

  /// Blink's `ForEachReverse`: the faces added from script, then the CSS-connected ones, each
  /// last first.
  fn reverse(&self) -> impl Iterator<Item = &Arc<FontFace>> {
    self
      .non_css_connected_face
      .iter()
      .rev()
      .chain(self.css_connected_face.iter().rev())
  }
}

impl CssSegmentedFontFace {
  pub(crate) fn new(font_selection_capabilities: FontSelectionCapabilities) -> Self {
    Self {
      font_selection_capabilities,
      font_faces: FontFaceList::default(),
    }
  }

  /// Blink's `GetFontSelectionCapabilities`.
  pub(crate) fn get_font_selection_capabilities(&self) -> FontSelectionCapabilities {
    self.font_selection_capabilities
  }

  /// Blink's `AddFontFace`.
  pub(crate) fn add_font_face(&mut self, font_face: Arc<FontFace>) {
    self.font_faces.insert(font_face);
  }

  /// Blink's `RemoveFontFace`.
  pub(crate) fn remove_font_face(&mut self, font_face: &Arc<FontFace>) -> bool {
    self.font_faces.erase(font_face)
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
  ) -> Option<Vec<FaceData<FontPlatformDataForRangeSet>>> {
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

    let created_font_data: Vec<FaceData<FontPlatformDataForRangeSet>> = self
      .font_faces
      .reverse()
      .map(|font_face| {
        font_face
          .get_font_data(&requested_font_description)
          .map(|font_platform_data| FontPlatformDataForRangeSet {
            font_platform_data,
            range_set: font_face.ranges().clone(),
          })
      })
      .collect();

    (!created_font_data.is_empty()).then_some(created_font_data)
  }
}
