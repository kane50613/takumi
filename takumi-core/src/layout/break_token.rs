//! Blink's [`BreakToken`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/break_token.h).

use std::rc::Rc;

use crate::layout::{
  block_break_token::BlockBreakToken, inline::inline_break_token::InlineBreakToken,
};

/// Blink's `BreakToken`: where a node resumes in the next fragmentainer.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BreakToken {
  Block(Rc<BlockBreakToken>),
  /// The lines of an inline formatting context, which resume at this break.
  Inline(InlineBreakToken),
}
