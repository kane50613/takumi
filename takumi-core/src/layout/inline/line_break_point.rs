//! Blink's [`LineBreakPoint`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/line_break_point.h).

use super::inline_item_text_index::InlineItemTextIndex;
use crate::layout_unit::LayoutUnit;

/// Where a line should break: before `offset`, past trailing spaces up to `end`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct LineBreakPoint {
  pub(crate) offset: InlineItemTextIndex,
  pub(crate) end: InlineItemTextIndex,
  pub(crate) is_hyphenated: bool,
  pub(crate) line_width: LayoutUnit,
}

impl LineBreakPoint {
  pub(crate) fn new(
    offset: InlineItemTextIndex,
    end: InlineItemTextIndex,
    is_hyphenated: bool,
  ) -> Self {
    Self {
      offset,
      end,
      is_hyphenated,
      line_width: LayoutUnit::ZERO,
    }
  }

  /// Blink's `operator bool`.
  pub(crate) fn is_set(&self) -> bool {
    self.offset.text_offset != 0
  }
}
