//! Blink's [`LayoutOpportunity`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/exclusions/layout_opportunity.cc)
//! and [`LineLayoutOpportunity`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/exclusions/line_layout_opportunity.h).

use crate::{
  layout::bfc_rect::{BfcOffset, BfcRect},
  layout_unit::LayoutUnit,
  style::Direction as TextDirection,
};

/// Blink's `LayoutOpportunity`: an area beside the floats where a box fits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LayoutOpportunity {
  pub(crate) rect: BfcRect,
}

impl Default for LayoutOpportunity {
  fn default() -> Self {
    Self {
      rect: BfcRect::new(
        BfcOffset::new(LayoutUnit::MIN, LayoutUnit::MIN),
        BfcOffset::new(LayoutUnit::MAX, LayoutUnit::MAX),
      ),
    }
  }
}

impl LayoutOpportunity {
  pub(crate) fn new(rect: BfcRect) -> Self {
    Self { rect }
  }

  /// Blink's `ComputeLineLayoutOpportunity`, for a container whose content box starts at
  /// `bfc_line_offset` and spans `available_inline_size`.
  pub(crate) fn compute_line_layout_opportunity(
    &self,
    bfc_line_offset: LayoutUnit,
    available_inline_size: LayoutUnit,
    direction: TextDirection,
    line_block_size: LayoutUnit,
    block_delta: LayoutUnit,
  ) -> LineLayoutOpportunity {
    let mut line_left = self.rect.line_start_offset();
    let mut line_right = self.rect.line_end_offset();

    // The content box bounds the line on its start side.
    match direction {
      TextDirection::Ltr => {
        let available_line_right = bfc_line_offset + available_inline_size;

        line_right = line_right.min(available_line_right).max(line_left);
      }
      TextDirection::Rtl => {
        line_left = line_left.max(bfc_line_offset).min(line_right);
      }
    }

    LineLayoutOpportunity {
      line_left_offset: line_left,
      line_right_offset: line_right,
      float_line_left_offset: self.rect.line_start_offset(),
      float_line_right_offset: self.rect.line_end_offset(),
      bfc_block_offset: self.rect.block_start_offset() + block_delta,
      line_block_size,
    }
  }
}

/// Blink's `LineLayoutOpportunity`: the inline extent a line may take, beside the floats.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct LineLayoutOpportunity {
  pub(crate) line_left_offset: LayoutUnit,
  pub(crate) line_right_offset: LayoutUnit,
  /// The extent floats may take on the line, the opportunity's own.
  pub(crate) float_line_left_offset: LayoutUnit,
  pub(crate) float_line_right_offset: LayoutUnit,
  pub(crate) bfc_block_offset: LayoutUnit,
  /// The block size the opportunity was found for.
  pub(crate) line_block_size: LayoutUnit,
}

impl LineLayoutOpportunity {
  /// An opportunity spanning `inline_size`.
  pub(crate) fn new(inline_size: LayoutUnit) -> Self {
    Self {
      line_right_offset: inline_size,
      float_line_right_offset: inline_size,
      ..Self::default()
    }
  }

  /// Blink's `AvailableInlineSize`.
  pub(crate) fn available_inline_size(&self) -> LayoutUnit {
    debug_assert!(self.line_right_offset >= self.line_left_offset);

    self.line_right_offset - self.line_left_offset
  }

  /// Blink's `AvailableFloatInlineSize`.
  pub(crate) fn available_float_inline_size(&self) -> LayoutUnit {
    debug_assert!(self.float_line_right_offset >= self.float_line_left_offset);

    self.float_line_right_offset - self.float_line_left_offset
  }

  /// Blink's `IsEqualToAvailableFloatInlineSize`.
  pub(crate) fn is_equal_to_available_float_inline_size(&self, inline_size: LayoutUnit) -> bool {
    self.float_line_left_offset + inline_size == self.float_line_right_offset
  }
}
