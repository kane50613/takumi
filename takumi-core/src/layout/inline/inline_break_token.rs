//! Blink's [`InlineBreakToken`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_break_token.h),
//! without block fragmentation or ruby.

use super::inline_item_text_index::InlineItemTextIndex;
use crate::context::RenderContext;

/// Where the next line starts, and the state it starts in.
#[derive(Clone, Copy)]
pub(crate) struct InlineBreakToken<'c> {
  /// The style at the break, where the next line's items start.
  pub(crate) style: Option<&'c RenderContext>,
  pub(crate) start: InlineItemTextIndex,
  pub(crate) is_forced_break: bool,
  pub(crate) has_cloned_box_decorations: bool,
  pub(crate) is_past_first_formatted_line: bool,
  pub(crate) is_line_clamp_displaced_line: bool,
}

impl InlineBreakToken<'_> {}
