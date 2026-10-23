//! Blink's [`BlockBreakToken`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/block_break_token.h),
//! for pages alone: no column spanners.

use crate::{
  geometry::{NodeId, Point},
  layout::{
    break_token::BreakToken, break_token_algorithm_data::BreakTokenAlgorithmData,
    layout_result::StaticPosition,
  },
  layout_unit::LayoutUnit,
};

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
  /// Blink's `IsRepeated`: the box repeats, as a table header or footer does on each page.
  pub(crate) is_repeated: bool,
  /// Blink's `IsAtBlockEnd`: layout was past the block-end border edge, so only overflow
  /// resumes.
  pub(crate) is_at_block_end: bool,
  /// Blink's `HasSeenAllChildren`: every child finished or has a break token.
  pub(crate) has_seen_all_children: bool,
  /// Blink's `HasUnpositionedListMarker`: the list item's outside marker waits for a later page.
  pub(crate) has_unpositioned_list_marker: bool,
  /// Blink's `ChildBreakTokens`: where each child laid out so far resumes.
  pub(crate) child_break_tokens: Vec<BreakToken>,
  /// Blink's `TokenData`: what the box's algorithm resumes from.
  pub(crate) algorithm_data: Option<BreakTokenAlgorithmData>,
  /// Blink's `OofInlineStartOffset` and `OofBlockStartOffset`, for an out-of-flow box laid out
  /// in its containing block's fragments: where in the next fragment's border box it resumes. The
  /// inline offset may come from a static position the next fragment cannot work out again.
  pub(crate) oof_start_offset: Point<LayoutUnit>,
  /// The static position of an out-of-flow box its containing block holds back until its last
  /// fragment, in the containing block's stitched fragments. Takumi's own, Blink has no such token.
  pub(crate) held_static_position: Option<StaticPosition>,
}

impl BlockBreakToken {
  /// Blink's `CreateRepeated`: where `node` repeats after its fragment `sequence_number`.
  pub(crate) fn repeated(node: NodeId, sequence_number: u32) -> Self {
    Self {
      sequence_number,
      is_break_before: false,
      is_repeated: true,
      ..Self::break_before(node, false)
    }
  }

  /// Blink's `CreateBreakBefore`: a break before `node`, which made no fragment yet.
  pub(crate) fn break_before(node: NodeId, is_forced_break: bool) -> Self {
    Self {
      node,
      consumed_block_size: LayoutUnit::ZERO,
      sequence_number: 0,
      monolithic_overflow: LayoutUnit::ZERO,
      is_break_before: true,
      is_forced_break,
      is_repeated: false,
      is_at_block_end: false,
      has_seen_all_children: false,
      has_unpositioned_list_marker: false,
      child_break_tokens: Vec::new(),
      algorithm_data: None,
      oof_start_offset: Point::default(),
      held_static_position: None,
    }
  }

  /// A break before an out-of-flow box that its containing block holds back, which lays it out
  /// from `static_position` once its block size is final.
  pub(crate) fn break_before_held_out_of_flow(
    node: NodeId,
    static_position: StaticPosition,
  ) -> Self {
    Self {
      held_static_position: Some(static_position),
      ..Self::break_before(node, false)
    }
  }

  /// Blink's `CreateBreakBefore` for an out-of-flow box pushed past the end of its containing
  /// block's fragment: it starts at `oof_start_offset` in the next.
  pub(crate) fn break_before_out_of_flow(
    node: NodeId,
    oof_start_offset: Point<LayoutUnit>,
  ) -> Self {
    Self {
      oof_start_offset,
      ..Self::break_before(node, false)
    }
  }
}
