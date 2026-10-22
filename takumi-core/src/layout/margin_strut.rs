//! Blink's [`MarginStrut`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/geometry/margin_strut.h),
//! without quirks mode or `margin-trim`.

use crate::layout_unit::LayoutUnit;

/// Blink's `MarginStrut`: the adjoining margins collapsed so far, as their most positive and most
/// negative.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct MarginStrut {
  pub(crate) positive_margin: LayoutUnit,
  pub(crate) negative_margin: LayoutUnit,
  /// Blink's `discard_margins`: every adjoining margin is dropped.
  pub(crate) discard_margins: bool,
}

impl MarginStrut {
  /// Blink's `Append`.
  pub(crate) fn append(&mut self, value: LayoutUnit) {
    if value < LayoutUnit::ZERO {
      self.negative_margin = self.negative_margin.min(value);
    } else {
      self.positive_margin = self.positive_margin.max(value);
    }
  }

  /// `self` with `other`'s margins collapsed into it.
  pub(crate) fn collapse_with(mut self, other: Self) -> Self {
    self.append(other.positive_margin);
    self.append(other.negative_margin);
    self.discard_margins |= other.discard_margins;
    self
  }

  /// Blink's `Sum`.
  pub(crate) fn sum(&self) -> LayoutUnit {
    if self.discard_margins {
      return LayoutUnit::ZERO;
    }

    self.positive_margin + self.negative_margin
  }
}

#[cfg(test)]
mod tests {
  use super::MarginStrut;
  use crate::layout_unit::LayoutUnit;

  fn px(value: i32) -> LayoutUnit {
    LayoutUnit::from_int(value)
  }

  /// The largest positive and the most negative margin add up.
  #[test]
  fn margins_collapse_to_the_largest_positive_and_most_negative() {
    let mut strut = MarginStrut::default();

    for margin in [10, 30, -5, -20] {
      strut.append(px(margin));
    }

    assert_eq!(strut.sum(), px(10));
  }
}
