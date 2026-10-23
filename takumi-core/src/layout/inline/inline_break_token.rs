//! Blink's [`InlineBreakToken`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_break_token.h),
//! without block fragmentation or ruby.

use super::inline_item_text_index::InlineItemTextIndex;

/// The style a break token resumes with: the block's, or that of the inline item whose box has it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BreakTokenStyle {
  Block,
  Item(usize),
}

/// Where the next line starts, and the state it starts in.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct InlineBreakToken {
  /// The style at the break, where the next line's items start.
  pub(crate) style: Option<BreakTokenStyle>,
  pub(crate) start: InlineItemTextIndex,
  pub(crate) is_forced_break: bool,
  pub(crate) has_cloned_box_decorations: bool,
  pub(crate) is_past_first_formatted_line: bool,
  pub(crate) is_line_clamp_displaced_line: bool,
}
