//! Blink's [`floats_utils`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/floats_utils.cc),
//! [`UnpositionedFloat`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/unpositioned_float.h)
//! and [`PositionedFloat`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/positioned_float.h),
//! without block fragmentation or `shape-outside`.

use taffy::{Clear, Float};

use crate::{
  layout::{
    bfc_rect::{BfcOffset, BfcRect},
    exclusions::{
      exclusion_area::ExclusionArea, exclusion_space::ExclusionSpace,
      layout_opportunity::LayoutOpportunity,
    },
    inline::{
      inline_item_result::{LayoutResult, LineBoxStrut},
      line_breaker::AtomicInlineLayout,
    },
    tree::RenderNode,
  },
  layout_unit::LayoutUnit,
  style::Direction as TextDirection,
};

/// Blink's `UnpositionedFloat`: a float to place, and where its search starts.
pub(crate) struct UnpositionedFloat<'c> {
  pub(crate) node: &'c RenderNode,
  /// The containing block's available inline size.
  pub(crate) available_inline_size: LayoutUnit,
  pub(crate) origin_bfc_offset: BfcOffset,
  /// The containing block's direction, which `float` and `clear` resolve against.
  pub(crate) direction: TextDirection,
  pub(crate) layout_result: Option<LayoutResult>,
  /// The margins, in the containing block's line-relative directions.
  pub(crate) margins: LineBoxStrut,
}

impl<'c> UnpositionedFloat<'c> {
  pub(crate) fn new(
    node: &'c RenderNode,
    available_inline_size: LayoutUnit,
    origin_bfc_offset: BfcOffset,
    direction: TextDirection,
  ) -> Self {
    Self {
      node,
      available_inline_size,
      origin_bfc_offset,
      direction,
      layout_result: None,
      margins: LineBoxStrut::default(),
    }
  }

  /// Blink's `ClearType`.
  pub(crate) fn clear_type(&self) -> Clear {
    self
      .node
      .context
      .style
      .base_data
      .clear
      .resolve(self.direction)
  }

  /// Blink's `Floating`: `Left` or `Right`.
  pub(crate) fn float_type(&self) -> Float {
    self
      .node
      .context
      .style
      .base_data
      .float
      .resolve(self.direction)
  }

  /// Blink's `IsLineRight`.
  pub(crate) fn is_line_right(&self) -> bool {
    self.float_type() == Float::Right
  }

  /// Blink's `LayoutFloatWithoutFragmentation`: lays the float out once, keeping its margins.
  fn layout_without_fragmentation(
    &mut self,
    layout: &mut dyn AtomicInlineLayout<'c>,
  ) -> LayoutResult {
    if let Some(layout_result) = self.layout_result {
      return layout_result;
    }

    let layout_result = layout.layout(self.node);
    let margins = self.node.margin_px();
    let (inline_start, inline_end) = match self.direction {
      TextDirection::Ltr => (margins.left, margins.right),
      TextDirection::Rtl => (margins.right, margins.left),
    };

    self.layout_result = Some(layout_result);
    self.margins = LineBoxStrut {
      inline_start: LayoutUnit::from_float(inline_start),
      inline_end: LayoutUnit::from_float(inline_end),
      line_over: LayoutUnit::from_float(margins.top),
      line_under: LayoutUnit::from_float(margins.bottom),
    };

    layout_result
  }

  /// The margins on the line-left side.
  fn margin_line_left(&self) -> LayoutUnit {
    match self.direction {
      TextDirection::Ltr => self.margins.inline_start,
      TextDirection::Rtl => self.margins.inline_end,
    }
  }
}

/// Blink's `PositionedFloat`: a float placed in the block formatting context.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PositionedFloat {
  pub(crate) layout_result: LayoutResult,
  /// The border box's offset.
  pub(crate) bfc_offset: BfcOffset,
}

/// Blink's `AdjustToClearance`.
pub(crate) fn adjust_to_clearance(clearance_offset: LayoutUnit, offset: &mut BfcOffset) {
  if clearance_offset > offset.block_offset {
    offset.block_offset = clearance_offset;
  }
}

/// Blink's `AdjustToTopEdgeAlignmentRule`: a float's outer top is no higher than any earlier
/// float's.
fn adjust_to_top_edge_alignment_rule(
  exclusion_space: &ExclusionSpace,
  offset: BfcOffset,
) -> BfcOffset {
  BfcOffset::new(
    offset.line_offset,
    offset
      .block_offset
      .max(exclusion_space.last_float_block_start()),
  )
}

/// Blink's `FindLayoutOpportunityForFloat`.
fn find_layout_opportunity_for_float(
  unpositioned_float: &UnpositionedFloat<'_>,
  exclusion_space: &ExclusionSpace,
  inline_size: LayoutUnit,
) -> LayoutOpportunity {
  let mut adjusted_origin_point =
    adjust_to_top_edge_alignment_rule(exclusion_space, unpositioned_float.origin_bfc_offset);

  adjust_to_clearance(
    exclusion_space.clearance_offset(unpositioned_float.clear_type()),
    &mut adjusted_origin_point,
  );

  exclusion_space.find_layout_opportunity(
    adjusted_origin_point,
    unpositioned_float.available_inline_size,
    unpositioned_float.direction,
    inline_size + unpositioned_float.margins.inline_sum(),
  )
}

/// Blink's `ComputeMarginBoxInlineSizeForUnpositionedFloat`.
pub(crate) fn compute_margin_box_inline_size_for_unpositioned_float<'c>(
  unpositioned_float: &mut UnpositionedFloat<'c>,
  layout: &mut dyn AtomicInlineLayout<'c>,
) -> LayoutUnit {
  let layout_result = unpositioned_float.layout_without_fragmentation(layout);

  (layout_result.inline_size + unpositioned_float.margins.inline_sum()).clamp_negative_to_zero()
}

/// Blink's `PositionFloat`: places the float in the first opportunity it fits, and adds it to the
/// exclusion space.
pub(crate) fn position_float<'c>(
  unpositioned_float: &mut UnpositionedFloat<'c>,
  exclusion_space: &mut ExclusionSpace,
  layout: &mut dyn AtomicInlineLayout<'c>,
) -> PositionedFloat {
  let layout_result = unpositioned_float.layout_without_fragmentation(layout);
  let opportunity = find_layout_opportunity_for_float(
    unpositioned_float,
    exclusion_space,
    layout_result.inline_size,
  );
  let margins = unpositioned_float.margins;
  // The margin box's offset.
  let mut float_margin_bfc_offset = opportunity.rect.start_offset;

  if unpositioned_float.is_line_right() {
    let float_margin_box_inline_size = layout_result.inline_size + margins.inline_sum();

    float_margin_bfc_offset.line_offset +=
      opportunity.rect.inline_size() - float_margin_box_inline_size;
  }

  // Blink's `CreateExclusionArea`.
  let end_offset = BfcOffset::new(
    float_margin_bfc_offset.line_offset
      + (layout_result.inline_size + margins.inline_sum()).clamp_negative_to_zero(),
    float_margin_bfc_offset.block_offset
      + (layout_result.block_size + margins.line_over + margins.line_under)
        .clamp_negative_to_zero(),
  );

  exclusion_space.add(ExclusionArea::new(
    BfcRect::new(float_margin_bfc_offset, end_offset),
    unpositioned_float.float_type(),
  ));

  // The border box's offset.
  PositionedFloat {
    layout_result,
    bfc_offset: BfcOffset::new(
      float_margin_bfc_offset.line_offset + unpositioned_float.margin_line_left(),
      float_margin_bfc_offset.block_offset + margins.line_over,
    ),
  }
}
