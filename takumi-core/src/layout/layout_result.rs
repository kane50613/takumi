//! Blink's [`LayoutResult`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/layout_result.h):
//! what laying out a block-level box gives its container, without fragmentation, line clamp, or
//! `text-box-trim`.

use std::rc::Rc;

use crate::{
  geometry::{ComputedLayout, NodeId, Point, Rect, Size},
  layout::{
    constraint_space::{AdjoiningObjectTypes, ConstraintSpace},
    exclusions::exclusion_space::ExclusionSpace,
    layout_style::LayoutStyle,
    logical_rect::{LogicalOffset, LogicalSize},
    margin_strut::MarginStrut,
  },
  layout_unit::LayoutUnit,
  style::{BaselineSource, Direction},
};

/// Blink's `LayoutResult::EStatus`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum LayoutStatus {
  #[default]
  Success,
  /// The layout stopped once it resolved its block offset, which moved the floats before it.
  BfcBlockOffsetResolved,
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

/// Blink's `PhysicalOofNodeForFragmentation` without fragmentation: an out-of-flow box on its way
/// up to its containing block, its static position in the border box of the box it reached.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OutOfFlowCandidate {
  pub(crate) node: NodeId,
  pub(crate) static_position: StaticPosition,
}

impl OutOfFlowCandidate {
  /// The candidate in the border box of a box that placed the one it came from at `offset`.
  pub(crate) fn moved_by(self, offset: Point<LayoutUnit>) -> Self {
    Self {
      static_position: StaticPosition {
        offset: self.static_position.offset + offset,
        ..self.static_position
      },
      ..self
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
  /// The out-of-flow boxes inside that their containing block has yet to place.
  pub(crate) out_of_flow: Vec<OutOfFlowCandidate>,
}

impl LayoutResult {
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
      end_margin_strut: MarginStrut::default(),
      exclusion_space: Rc::default(),
      adjoining_object_types: AdjoiningObjectTypes::empty(),
      is_self_collapsing: false,
      is_pushed_by_floats: false,
      first_baseline: None,
      last_baseline: None,
      use_last_baseline_for_inline_baseline: false,
      out_of_flow: Vec::new(),
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
