//! Blink's [`InlineBreakToken`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_break_token.h),
//! without ruby.

#[cfg(feature = "fragmentation")]
use std::rc::Rc;

use super::inline_item_text_index::InlineItemTextIndex;
#[cfg(feature = "fragmentation")]
use crate::layout::block_break_token::BlockBreakToken;
use crate::layout::fragmentation_layout_input::ChildBreakToken;

/// The style a break token resumes with: the block's, or that of the inline item whose box has it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BreakTokenStyle {
  Block,
  Item(usize),
}

/// Where the next line starts, and the state it starts in.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct InlineBreakToken {
  /// The style at the break, where the next line's items start.
  pub(crate) style: Option<BreakTokenStyle>,
  pub(crate) start: InlineItemTextIndex,
  pub(crate) is_forced_break: bool,
  pub(crate) has_cloned_box_decorations: bool,
  pub(crate) is_past_first_formatted_line: bool,
  pub(crate) is_line_clamp_displaced_line: bool,
  /// Blink's `IsInParallelBlockFlow`: the token resumes a block-in-inline whose content overflowed
  /// its end, beside the lines after it.
  pub(crate) is_in_parallel_block_flow: bool,
  /// Blink's `GetBlockBreakToken`: where the block-in-inline at `start` resumes.
  pub(crate) block_break_token: ChildBreakToken,
}

#[cfg(feature = "fragmentation")]
impl InlineBreakToken {
  /// Blink's `CreateForParallelBlockFlow`: the block-in-inline at `start` resumes at
  /// `block_break_token` in a line of its own.
  pub(crate) fn for_parallel_block_flow(
    start: InlineItemTextIndex,
    block_break_token: Rc<BlockBreakToken>,
  ) -> Self {
    Self {
      style: Some(BreakTokenStyle::Block),
      start,
      is_in_parallel_block_flow: true,
      block_break_token: Some(block_break_token),
      ..Self::default()
    }
  }
}
