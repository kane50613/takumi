//! Blink's [`BlockBreakToken`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/block_break_token.h),
//! for pages alone: no column spanners, repeated content, out-of-flow fragmentation, or list
//! markers left unpositioned.

use crate::{geometry::NodeId, layout::break_token::BreakToken, layout_unit::LayoutUnit};

/// Blink's `BlockBreakToken`: where a box resumes in the next fragmentainer.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BlockBreakToken {
  pub(crate) node: NodeId,
  /// Blink's `ConsumedBlockSize`: the block size the box's earlier fragments took.
  pub(crate) consumed_block_size: LayoutUnit,
  /// Blink's `SequenceNumber`: the fragment's index, undefined for a break before the box.
  pub(crate) sequence_number: u32,
  /// Blink's `MonolithicOverflow`: how far monolithic content overflowed the page.
  pub(crate) monolithic_overflow: LayoutUnit,
  /// Blink's `IsBreakBefore`: the box made no fragment before the break.
  pub(crate) is_break_before: bool,
  pub(crate) is_forced_break: bool,
  /// Blink's `IsAtBlockEnd`: layout was past the block-end border edge, so only overflow
  /// resumes.
  pub(crate) is_at_block_end: bool,
  /// Blink's `HasSeenAllChildren`: every child finished or has a break token.
  pub(crate) has_seen_all_children: bool,
  /// Blink's `ChildBreakTokens`: where each child laid out so far resumes.
  pub(crate) child_break_tokens: Vec<BreakToken>,
}

impl BlockBreakToken {
  /// Blink's `CreateBreakBefore`: a break before `node`, which made no fragment yet.
  pub(crate) fn break_before(node: NodeId, is_forced_break: bool) -> Self {
    Self {
      node,
      consumed_block_size: LayoutUnit::ZERO,
      sequence_number: 0,
      monolithic_overflow: LayoutUnit::ZERO,
      is_break_before: true,
      is_forced_break,
      is_at_block_end: false,
      has_seen_all_children: false,
      child_break_tokens: Vec::new(),
    }
  }
}
