//! Blink's [`FontHeight`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/platform/fonts/font_height.h).

use crate::layout_unit::LayoutUnit;

/// Blink's `FontHeight`: how far a box reaches above and below its baseline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct FontHeight {
  pub(crate) ascent: LayoutUnit,
  pub(crate) descent: LayoutUnit,
}

impl FontHeight {
  pub(crate) fn new(ascent: LayoutUnit, descent: LayoutUnit) -> Self {
    Self { ascent, descent }
  }

  /// Blink's `Empty`: nothing yet, which any height unites to.
  pub(crate) fn empty() -> Self {
    Self::new(LayoutUnit::MIN, LayoutUnit::MIN)
  }

  /// Blink's `IsEmpty`.
  pub(crate) fn is_empty(&self) -> bool {
    self.ascent == LayoutUnit::MIN && self.descent == LayoutUnit::MIN
  }

  /// Blink's `LineHeight`.
  pub(crate) fn line_height(&self) -> LayoutUnit {
    self.ascent + self.descent
  }

  /// Blink's `AddLeading`.
  pub(crate) fn add_leading(&mut self, start_and_end_leading: Self) {
    debug_assert!(!self.is_empty());

    self.ascent += start_and_end_leading.ascent;
    self.descent += start_and_end_leading.descent;
  }

  /// Blink's `Move`: moves the box down by `delta`.
  pub(crate) fn move_by(&mut self, delta: LayoutUnit) {
    debug_assert!(!self.is_empty());

    self.ascent -= delta;
    self.descent += delta;
  }

  /// Blink's `Unite`.
  pub(crate) fn unite(&mut self, other: &Self) {
    self.ascent = self.ascent.max(other.ascent);
    self.descent = self.descent.max(other.descent);
  }
}

/// Blink's `CalculateLeadingSpace`: the leading `line_height` adds around `current_height`, the
/// half above floored to a whole pixel.
pub(crate) fn calculate_leading_space(
  line_height: LayoutUnit,
  current_height: &FontHeight,
) -> FontHeight {
  let ascent_leading_spacing =
    LayoutUnit::from_int(((line_height - current_height.line_height()) / 2).floor());
  let descent_leading_spacing = line_height - current_height.line_height() - ascent_leading_spacing;

  FontHeight::new(ascent_leading_spacing, descent_leading_spacing)
}
