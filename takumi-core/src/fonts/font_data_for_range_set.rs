//! Blink's [`FontDataForRangeSet`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/font_data_for_range_set.h).

use std::rc::Rc;

use super::{
  font_face::{FaceData, PendingFace},
  simple_font_data::SimpleFontData,
};
use crate::resources::unicode_range::UnicodeRangeSet;

/// Blink's `FontDataForRangeSet`.
#[derive(Debug, Clone, Default)]
pub(crate) struct FontDataForRangeSet {
  font_data: Option<FaceData<Rc<SimpleFontData>>>,
}

impl FontDataForRangeSet {
  pub(crate) fn new(font_data: Rc<SimpleFontData>) -> Self {
    Self {
      font_data: Some(FaceData::Loaded(font_data)),
    }
  }

  /// A loaded or pending face.
  pub(crate) fn from_face_data(font_data: FaceData<Rc<SimpleFontData>>) -> Self {
    Self {
      font_data: Some(font_data),
    }
  }

  /// The face's `unicode-range`, none for every codepoint.
  fn range_set(&self) -> Option<&UnicodeRangeSet> {
    match self.font_data.as_ref()? {
      FaceData::Loaded(font_data) => font_data.face.unicode_range.as_ref(),
      FaceData::Pending(face) => Some(&face.range_set),
    }
  }

  /// Blink's `Contains`.
  pub(crate) fn contains(&self, ch: char) -> bool {
    self.font_data.is_some()
      && self
        .range_set()
        .is_none_or(|range| range.contains(u32::from(ch)))
  }

  /// Blink's `IsEntireRange`.
  pub(crate) fn is_entire_range(&self) -> bool {
    self
      .range_set()
      .is_none_or(UnicodeRangeSet::is_entire_range)
  }

  /// Blink's `FontData`, none for a pending face, whose loading fallback takumi does not make.
  pub(crate) fn font_data(&self) -> Option<&Rc<SimpleFontData>> {
    self.font_data.as_ref()?.loaded()
  }

  /// The pending face, Blink's `IsPendingCustomFont`.
  pub(crate) fn pending_face(&self) -> Option<&PendingFace> {
    match self.font_data.as_ref()? {
      FaceData::Loaded(_) => None,
      FaceData::Pending(face) => Some(face),
    }
  }
}
