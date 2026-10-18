//! What a layout reads of Blink's [`ConstraintSpace`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/constraint_space.h):
//! the available inline size and where the container sits in its block formatting context.

use crate::{layout::bfc_rect::BfcOffset, layout_unit::LayoutUnit};

/// Blink's `ConstraintSpace`, as inline layout reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ConstraintSpace {
  /// Blink's `AvailableSize().inline_size`, none while computing intrinsic sizes.
  pub(crate) available_inline_size: Option<LayoutUnit>,
  /// Blink's `GetBfcOffset`: the content box's offset.
  pub(crate) bfc_offset: BfcOffset,
}

impl ConstraintSpace {
  pub(crate) fn new(available_inline_size: Option<LayoutUnit>) -> Self {
    Self {
      available_inline_size,
      bfc_offset: BfcOffset::default(),
    }
  }

  /// The available inline size, zero while computing intrinsic sizes.
  pub(crate) fn inline_size(&self) -> LayoutUnit {
    self.available_inline_size.unwrap_or(LayoutUnit::ZERO)
  }
}
