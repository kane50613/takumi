//! Blink's [`FontDataForRangeSet`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/font_data_for_range_set.h).

use std::rc::Rc;

use super::simple_font_data::SimpleFontData;
use crate::resources::unicode_range::UnicodeRangeSet;

/// Blink's `FontDataForRangeSet`.
#[derive(Debug, Clone, Default)]
pub(crate) struct FontDataForRangeSet {
  font_data: Option<Rc<SimpleFontData>>,
}

impl FontDataForRangeSet {
  pub(crate) fn new(font_data: Rc<SimpleFontData>) -> Self {
    Self {
      font_data: Some(font_data),
    }
  }

  /// Blink's `Contains`.
  pub(crate) fn contains(&self, ch: char) -> bool {
    self.font_data.as_ref().is_some_and(|font_data| {
      font_data
        .face()
        .unicode_range
        .as_ref()
        .is_none_or(|range| range.contains(u32::from(ch)))
    })
  }

  /// Blink's `IsEntireRange`.
  pub(crate) fn is_entire_range(&self) -> bool {
    self.font_data.as_ref().is_none_or(|font_data| {
      font_data
        .face()
        .unicode_range
        .as_ref()
        .is_none_or(UnicodeRangeSet::is_entire_range)
    })
  }

  pub(crate) fn font_data(&self) -> Option<&Rc<SimpleFontData>> {
    self.font_data.as_ref()
  }
}
