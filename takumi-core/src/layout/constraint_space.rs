//! What a layout reads of Blink's [`ConstraintSpace`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/constraint_space.h):
//! the available inline size and where the container sits in its block formatting context.

use crate::{layout::bfc_rect::BfcOffset, layout_unit::LayoutUnit, style::Direction};

/// Blink's `ConstraintSpace`, as inline layout reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ConstraintSpace {
  /// Blink's `AvailableSize().inline_size`, none while computing intrinsic sizes.
  pub(crate) available_inline_size: Option<LayoutUnit>,
  /// Blink's `GetBfcOffset`: the content box's offset.
  pub(crate) bfc_offset: BfcOffset,
  /// Blink's `Direction`: the container's.
  pub(crate) direction: Direction,
  /// Blink's `PercentageResolutionSize`, none where indefinite.
  pub(crate) percentage_resolution_inline_size: Option<LayoutUnit>,
  pub(crate) percentage_resolution_block_size: Option<LayoutUnit>,
  /// Blink's `GetLineClampData().LinesUntilClamp()`, for a box clamped by lines.
  pub(crate) lines_until_clamp: Option<i32>,
}

impl ConstraintSpace {
  /// A space for a container of `direction`, its percentages against an indefinite block size
  /// until the caller knows one.
  pub(crate) fn new(available_inline_size: Option<LayoutUnit>, direction: Direction) -> Self {
    Self {
      available_inline_size,
      bfc_offset: BfcOffset::default(),
      direction,
      percentage_resolution_inline_size: available_inline_size,
      percentage_resolution_block_size: None,
      lines_until_clamp: None,
    }
  }

  /// Blink's `LineClampData::IsAtClampPoint`: the line the clamp ellipsizes.
  pub(crate) fn is_at_clamp_point(&self) -> bool {
    self.lines_until_clamp == Some(1)
  }

  /// The available inline size, zero while computing intrinsic sizes.
  pub(crate) fn inline_size(&self) -> LayoutUnit {
    self.available_inline_size.unwrap_or(LayoutUnit::ZERO)
  }
}
