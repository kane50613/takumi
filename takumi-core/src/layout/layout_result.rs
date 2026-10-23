//! Blink's [`LayoutResult`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/layout_result.h):
//! what laying out a block-level box gives its container, without columns, line clamp, or
//! `text-box-trim`.

use std::rc::Rc;

use crate::{
  geometry::{ComputedLayout, NodeId, Point, Rect, Size},
  layout::{
    constraint_space::{AdjoiningObjectTypes, ConstraintSpace},
    exclusions::exclusion_space::ExclusionSpace,
    fragmentation_layout_input::FragmentationLayoutInput,
    layout_style::LayoutStyle,
    logical_rect::{LogicalOffset, LogicalSize},
    margin_strut::MarginStrut,
  },
  layout_unit::LayoutUnit,
  style::{BaselineSource, Direction},
};
#[cfg(feature = "fragmentation")]
use crate::{
  layout::{
    block_break_token::BlockBreakToken, break_appeal::BreakAppeal, early_break::EarlyBreak,
    fragmentation_utils::fragment_index, out_of_flow::ContainingBlock,
  },
  style::BreakBetween,
};

/// Blink's `LayoutResult::EStatus`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum LayoutStatus {
  #[default]
  Success,
  /// The layout stopped once it resolved its block offset, which moved the floats before it.
  BfcBlockOffsetResolved,
  /// The layout ran out of space where a breakpoint seen earlier is better, so it lays out again
  /// to break there.
  #[cfg(feature = "fragmentation")]
  NeedsEarlierBreak,
  /// A flex row grew to fit its items on the page, so the container lays out again with it.
  #[cfg(feature = "fragmentation")]
  NeedsRelayoutWithRowCrossSizeChanges,
}

/// Blink's `LogicalStaticPosition::InlineEdge`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum InlineEdge {
  #[default]
  Start,
  Center,
  End,
}

/// Blink's `LogicalStaticPosition::BlockEdge`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum BlockEdge {
  #[default]
  Start,
  Center,
  End,
}

/// Blink's `PhysicalStaticPosition::HorizontalEdge`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum HorizontalEdge {
  #[default]
  Left,
  Center,
  Right,
}

/// Blink's `PhysicalStaticPosition::VerticalEdge`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum VerticalEdge {
  #[default]
  Top,
  Center,
  Bottom,
}

/// Blink's `LogicalStaticPosition::LogicalAlignmentDirection`: the direction `align-self` applies
/// in, `justify-self` applying in the other.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum LogicalAlignmentDirection {
  #[default]
  Block,
  Inline,
}

/// Blink's `LogicalStaticPosition`: where an out-of-flow box would sit in flow, and which edge of
/// its margin box sits there.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct LogicalStaticPosition {
  pub(crate) offset: LogicalOffset,
  pub(crate) inline_edge: InlineEdge,
  pub(crate) block_edge: BlockEdge,
  pub(crate) align_self_direction: LogicalAlignmentDirection,
}

impl LogicalStaticPosition {
  /// Blink's `ConvertToPhysical` in a box `inline_size` wide laid out in `direction`.
  pub(crate) fn to_physical(self, direction: Direction, inline_size: LayoutUnit) -> StaticPosition {
    let (x, horizontal_edge) = match (direction, self.inline_edge) {
      (Direction::Ltr, InlineEdge::Start) => (self.offset.inline_offset, HorizontalEdge::Left),
      (Direction::Ltr, InlineEdge::End) => (self.offset.inline_offset, HorizontalEdge::Right),
      (Direction::Rtl, InlineEdge::Start) => (
        inline_size - self.offset.inline_offset,
        HorizontalEdge::Right,
      ),
      (Direction::Rtl, InlineEdge::End) => (
        inline_size - self.offset.inline_offset,
        HorizontalEdge::Left,
      ),
      (Direction::Ltr, InlineEdge::Center) => (self.offset.inline_offset, HorizontalEdge::Center),
      (Direction::Rtl, InlineEdge::Center) => (
        inline_size - self.offset.inline_offset,
        HorizontalEdge::Center,
      ),
    };

    StaticPosition {
      offset: Point::new(x, self.offset.block_offset),
      horizontal_edge,
      vertical_edge: match self.block_edge {
        BlockEdge::Start => VerticalEdge::Top,
        BlockEdge::Center => VerticalEdge::Center,
        BlockEdge::End => VerticalEdge::Bottom,
      },
      align_self_direction: self.align_self_direction,
    }
  }
}

/// Blink's `PhysicalStaticPosition`, in the border box of the box holding the out-of-flow box. A
/// horizontal writing mode keeps the block direction vertical, so the alignment direction stays
/// logical.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct StaticPosition {
  pub(crate) offset: Point<LayoutUnit>,
  pub(crate) horizontal_edge: HorizontalEdge,
  pub(crate) vertical_edge: VerticalEdge,
  pub(crate) align_self_direction: LogicalAlignmentDirection,
}

impl StaticPosition {
  /// Blink's `ConvertToLogical` for a box laid out in `direction` in a containing block
  /// `inline_size` wide.
  pub(crate) fn to_logical(
    self,
    direction: Direction,
    inline_size: LayoutUnit,
  ) -> LogicalStaticPosition {
    let (inline_offset, inline_edge) = match (direction, self.horizontal_edge) {
      (Direction::Ltr, HorizontalEdge::Left) => (self.offset.x, InlineEdge::Start),
      (Direction::Ltr, HorizontalEdge::Right) => (self.offset.x, InlineEdge::End),
      (Direction::Ltr, HorizontalEdge::Center) => (self.offset.x, InlineEdge::Center),
      (Direction::Rtl, HorizontalEdge::Left) => (inline_size - self.offset.x, InlineEdge::End),
      (Direction::Rtl, HorizontalEdge::Right) => (inline_size - self.offset.x, InlineEdge::Start),
      (Direction::Rtl, HorizontalEdge::Center) => (inline_size - self.offset.x, InlineEdge::Center),
    };

    LogicalStaticPosition {
      offset: LogicalOffset::new(inline_offset, self.offset.y),
      inline_edge,
      block_edge: match self.vertical_edge {
        VerticalEdge::Top => BlockEdge::Start,
        VerticalEdge::Center => BlockEdge::Center,
        VerticalEdge::Bottom => BlockEdge::End,
      },
      align_self_direction: self.align_self_direction,
    }
  }
}

/// Blink's `PhysicalOofNodeForFragmentation`: an out-of-flow box on its way up to its containing
/// block, its static position in the border box of the box it reached.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OutOfFlowCandidate {
  pub(crate) node: NodeId,
  pub(crate) static_position: StaticPosition,
  /// Blink's `OofPositionedNode::BreakToken`: where the box resumes in a later fragment of its
  /// containing block, which a page found it in before.
  #[cfg(feature = "fragmentation")]
  pub(crate) break_token: Option<Rc<BlockBreakToken>>,
  /// Blink's `RequiresContentBeforeBreaking` of the box that found the box: the box stays on the
  /// page it starts on.
  #[cfg(feature = "fragmentation")]
  pub(crate) requires_content_before_breaking: bool,
  /// Blink's fragmentainer descendant: the containing block that left the box to the
  /// fragmentation root, which lays it out across pages.
  #[cfg(feature = "fragmentation")]
  pub(crate) fragmented_containing_block: Option<FragmentedContainingBlock>,
}

/// A containing block that lays out with fragmentation, as an out-of-flow box it contains sees
/// it: the fragment the box's static position is in.
#[cfg(feature = "fragmentation")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FragmentedContainingBlock {
  pub(crate) node: NodeId,
  pub(crate) fragment_index: usize,
  /// The fragment's border box in the border box of the box the candidate reached.
  pub(crate) offset: Point<LayoutUnit>,
  /// The block size the containing block's earlier fragments took.
  pub(crate) consumed_block_size: LayoutUnit,
  /// Its padding box in its border box, the block size that of this fragment.
  pub(crate) padding_box: ContainingBlock,
  /// Blink's `IsFragmentedInsideClippedContainer`: a box from it up clips overflow, so the box
  /// adds no pages past the last.
  pub(crate) is_inside_clipped_container: bool,
}

impl OutOfFlowCandidate {
  /// The candidate whose static position is `static_position`, on its way to its containing block.
  pub(crate) fn new(node: NodeId, static_position: StaticPosition) -> Self {
    Self {
      node,
      static_position,
      #[cfg(feature = "fragmentation")]
      break_token: None,
      #[cfg(feature = "fragmentation")]
      requires_content_before_breaking: false,
      #[cfg(feature = "fragmentation")]
      fragmented_containing_block: None,
    }
  }

  /// Blink's `AddOutOfFlowChildCandidate` for a box resumed at `break_token`, which its
  /// containing block lays out where the token says.
  #[cfg(feature = "fragmentation")]
  pub(crate) fn resumed(
    node: NodeId,
    break_token: Rc<BlockBreakToken>,
    requires_content_before_breaking: bool,
  ) -> Self {
    Self {
      break_token: Some(break_token),
      requires_content_before_breaking,
      ..Self::new(node, StaticPosition::default())
    }
  }

  /// Whether a page found the box before and it resumes at its break token: Blink's check that a
  /// containing block holding the token contains the box. A forced break before the box may not
  /// have reached its containing block.
  pub(crate) fn is_resumed(&self) -> bool {
    #[cfg(feature = "fragmentation")]
    return self
      .break_token
      .as_ref()
      .is_some_and(|token| !token.is_forced_break);
    #[cfg(not(feature = "fragmentation"))]
    return false;
  }

  /// Where the box's layout resumes it.
  pub(crate) fn layout_input(&self) -> FragmentationLayoutInput {
    FragmentationLayoutInput {
      #[cfg(feature = "fragmentation")]
      break_token: self.break_token.clone(),
      ..FragmentationLayoutInput::default()
    }
  }

  /// Blink's `FragmentIndex` of the box's fragment its containing block lays out.
  pub(crate) fn fragment_index(&self) -> usize {
    #[cfg(feature = "fragmentation")]
    return fragment_index(self.break_token.as_deref());
    #[cfg(not(feature = "fragmentation"))]
    return 0;
  }

  /// Whether a containing block that lays out with fragmentation left the box to the
  /// fragmentation root.
  pub(crate) fn is_deferred(&self) -> bool {
    #[cfg(feature = "fragmentation")]
    return self.fragmented_containing_block.is_some();
    #[cfg(not(feature = "fragmentation"))]
    return false;
  }

  /// The candidate in the border box of a box that placed the one it came from at `offset`.
  pub(crate) fn moved_by(&self, offset: Point<LayoutUnit>) -> Self {
    Self {
      static_position: StaticPosition {
        offset: self.static_position.offset + offset,
        ..self.static_position
      },
      #[cfg(feature = "fragmentation")]
      fragmented_containing_block: self.fragmented_containing_block.map(|containing_block| {
        FragmentedContainingBlock {
          offset: containing_block.offset + offset,
          ..containing_block
        }
      }),
      ..self.clone()
    }
  }
}

/// Blink's `LayoutResult` with its `PhysicalBoxFragment`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutResult {
  pub(crate) status: LayoutStatus,
  /// The border box.
  pub(crate) size: LogicalSize,
  /// The used physical border and padding.
  pub(crate) border: Rect<f32>,
  pub(crate) padding: Rect<f32>,
  /// Blink's `IntrinsicBlockSize`: the block size the content gave, before `height` applied.
  pub(crate) intrinsic_block_size: LayoutUnit,
  pub(crate) bfc_line_offset: LayoutUnit,
  /// Blink's `BfcBlockOffset`, `None` while the box collapses through.
  pub(crate) bfc_block_offset: Option<LayoutUnit>,
  /// Blink's `LineBoxBfcBlockOffset`: a line box's top, below any floats it moved under.
  pub(crate) line_box_bfc_block_offset: Option<LayoutUnit>,
  /// Blink's `ClearanceAfterLine`: the space a `<br clear>` adds after a line box.
  pub(crate) clearance_after_line: LayoutUnit,
  /// Blink's `LinesUntilClamp`, after a line box.
  pub(crate) lines_until_clamp: Option<i32>,
  pub(crate) end_margin_strut: MarginStrut,
  /// The floats of the block formatting context after the box.
  pub(crate) exclusion_space: Rc<ExclusionSpace>,
  pub(crate) adjoining_object_types: AdjoiningObjectTypes,
  pub(crate) is_self_collapsing: bool,
  pub(crate) is_pushed_by_floats: bool,
  /// Below the border box's block start.
  pub(crate) first_baseline: Option<LayoutUnit>,
  pub(crate) last_baseline: Option<LayoutUnit>,
  /// Blink's `UseLastBaselineForInlineBaseline`.
  pub(crate) use_last_baseline_for_inline_baseline: bool,
  /// Blink's `IsLineClampClippedFloat`: a float its line-clamp container clips.
  pub(crate) is_line_clamp_clipped_float: bool,
  /// The out-of-flow boxes inside that their containing block has yet to place.
  pub(crate) out_of_flow: Vec<OutOfFlowCandidate>,
  /// Blink's `TableSectionCollapsedBordersGeometry`: the table row a section fragment's rows start
  /// at, then each row's block-start edge and the last row's block-end edge.
  pub(crate) table_section_row_offsets: Option<(usize, Vec<LayoutUnit>)>,
  #[cfg(feature = "fragmentation")]
  pub(crate) break_info: BreakInfo,
}

/// What Blink's `LayoutResult` and its fragment tell the container about breaking.
#[cfg(feature = "fragmentation")]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BreakInfo {
  /// Blink's `PhysicalBoxFragment::GetBreakToken`: where the box resumes, none once it ended.
  pub(crate) break_token: Option<Rc<BlockBreakToken>>,
  /// Blink's `GetBreakAppeal`: how good the break inside is.
  pub(crate) break_appeal: BreakAppeal,
  /// Blink's `HasForcedBreak`.
  pub(crate) has_forced_break: bool,
  /// Blink's `InitialBreakBefore`: the joined `break-before` of the first children.
  pub(crate) initial_break_before: Option<BreakBetween>,
  /// Blink's `FinalBreakAfter`: the joined `break-after` of the last children.
  pub(crate) final_break_after: Option<BreakBetween>,
  /// Blink's `GetEarlyBreak`: the best breakpoint inside to go back to.
  pub(crate) early_break: Option<Rc<EarlyBreak>>,
  /// Blink's `BlockSizeForFragmentation`, none to take the border box.
  pub(crate) block_size_for_fragmentation: Option<LayoutUnit>,
  /// Blink's `IsBlockSizeForFragmentationClamped`.
  pub(crate) is_block_size_for_fragmentation_clamped: bool,
  /// Blink's `IsFirstForNode`: the box's first fragment.
  pub(crate) is_first_for_node: bool,
  /// Blink's `FragmentIndex` of the break token the layout resumed at: which fragment of the box
  /// this is.
  pub(crate) fragment_index: usize,
  /// Whether the layout repeats the box's first, as a table header's does on each later page.
  pub(crate) is_repeated: bool,
  /// Whether the box's `break-inside` avoids a page break, which Blink reads off the fragment's
  /// style.
  pub(crate) avoids_break_inside: bool,
}

#[cfg(feature = "fragmentation")]
impl Default for BreakInfo {
  /// A layout that did not break: a perfect break appeal, the box's first fragment.
  fn default() -> Self {
    Self {
      break_token: None,
      break_appeal: BreakAppeal::Perfect,
      has_forced_break: false,
      initial_break_before: None,
      final_break_after: None,
      early_break: None,
      block_size_for_fragmentation: None,
      is_block_size_for_fragmentation_clamped: false,
      is_first_for_node: true,
      fragment_index: 0,
      is_repeated: false,
      avoids_break_inside: false,
    }
  }
}

impl LayoutResult {
  /// Which fragment of its box the layout is.
  pub(crate) fn fragment_index(&self) -> usize {
    #[cfg(feature = "fragmentation")]
    return self.break_info.fragment_index;
    #[cfg(not(feature = "fragmentation"))]
    return 0;
  }

  /// Whether the layout repeats its box's first, as a table header's does on each later page.
  pub(crate) fn is_repeated(&self) -> bool {
    #[cfg(feature = "fragmentation")]
    return self.break_info.is_repeated;
    #[cfg(not(feature = "fragmentation"))]
    return false;
  }

  /// A successful layout of a border box `size`.
  pub(crate) fn new(size: LogicalSize) -> Self {
    Self {
      status: LayoutStatus::Success,
      size,
      border: Rect::default(),
      padding: Rect::default(),
      intrinsic_block_size: size.block_size,
      bfc_line_offset: LayoutUnit::ZERO,
      bfc_block_offset: None,
      line_box_bfc_block_offset: None,
      clearance_after_line: LayoutUnit::ZERO,
      lines_until_clamp: None,
      end_margin_strut: MarginStrut::default(),
      exclusion_space: ExclusionSpace::empty(),
      adjoining_object_types: AdjoiningObjectTypes::empty(),
      is_self_collapsing: false,
      is_pushed_by_floats: false,
      first_baseline: None,
      last_baseline: None,
      use_last_baseline_for_inline_baseline: false,
      is_line_clamp_clipped_float: false,
      out_of_flow: Vec::new(),
      table_section_row_offsets: None,
      #[cfg(feature = "fragmentation")]
      break_info: BreakInfo::default(),
    }
  }

  /// A successful layout of `style`'s border box `size` under `space`.
  pub(crate) fn of_box(size: LogicalSize, style: &LayoutStyle, space: &ConstraintSpace) -> Self {
    let percentage_inline_size = space.percentage_resolution_inline_size;

    Self {
      border: style.border,
      padding: style
        .padding
        .map(|padding| padding.resolve_or_zero(percentage_inline_size).to_float()),
      ..Self::new(size)
    }
  }

  /// The box's layout with its border box at `location`.
  pub(crate) fn computed_layout(&self, location: Point<LayoutUnit>) -> ComputedLayout {
    ComputedLayout::new(
      location.map(LayoutUnit::to_float),
      Size {
        width: self.size.inline_size.to_float(),
        height: self.size.block_size.to_float(),
      },
      self.border,
      self.padding,
    )
  }

  /// Blink's `ForceInlineBaselineSynthesis`: an inline-block's scroll container that scrolls in
  /// the block axis takes its block-end margin edge for a baseline.
  pub(crate) fn forces_inline_baseline_synthesis(&self, style: &LayoutStyle) -> bool {
    self.use_last_baseline_for_inline_baseline && !style.overflow.y.is_clip_or_visible()
  }

  /// Blink's `LogicalBoxFragment::BaselineMetrics` baseline, none where the line synthesizes one.
  pub(crate) fn inline_baseline(&self, style: &LayoutStyle) -> Option<LayoutUnit> {
    match style.baseline_source {
      BaselineSource::Auto if self.forces_inline_baseline_synthesis(style) => None,
      BaselineSource::Auto if self.use_last_baseline_for_inline_baseline => self.last_baseline,
      BaselineSource::Auto | BaselineSource::First => self.first_baseline,
      BaselineSource::Last => self.last_baseline,
    }
  }

  /// A layout that stopped at `bfc_block_offset`, which its container resolves and lays it out at
  /// again.
  pub(crate) fn aborted(
    status: LayoutStatus,
    bfc_block_offset: Option<LayoutUnit>,
    is_pushed_by_floats: bool,
  ) -> Self {
    Self {
      status,
      bfc_block_offset,
      is_pushed_by_floats,
      ..Self::new(LogicalSize::default())
    }
  }
}
