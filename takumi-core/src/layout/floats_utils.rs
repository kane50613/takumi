//! Blink's [`floats_utils`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/floats_utils.cc),
//! [`UnpositionedFloat`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/unpositioned_float.h)
//! and [`PositionedFloat`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/positioned_float.h),
//! without block fragmentation or `shape-outside`.

use std::rc::Rc;

use crate::{
  layout::{
    bfc_rect::{BfcOffset, BfcRect},
    constraint_space::{CacheSlot, ConstraintSpace},
    exclusions::{
      exclusion_area::ExclusionArea, exclusion_space::ExclusionSpace,
      layout_opportunity::LayoutOpportunity,
    },
    inline::inline_item_result::LineBoxStrut,
    layout_result::LayoutResult,
    line_clamp_data::LineClampFloatState,
    logical_rect::AvailableLogicalSize,
    tree::RenderNode,
  },
  layout_unit::LayoutUnit,
  style::Direction as TextDirection,
};

/// Blink's `EFloat` resolved to a physical side, as `ComputedStyle::Floating` returns it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Float {
  #[default]
  None,
  Left,
  Right,
}

/// Blink's `EClear` resolved to physical sides, as `ComputedStyle::Clear` returns it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Clear {
  #[default]
  None,
  Left,
  Right,
  Both,
}

/// Lays a float out in a space, as Blink's `BlockNode::Layout` lays out the float's node.
pub(crate) type LayoutFloat<'l, 'c> =
  dyn FnMut(&'c RenderNode, &ConstraintSpace) -> Rc<LayoutResult> + 'l;

/// Blink's `UnpositionedFloat`: a float to place, and where its search starts. It keeps the
/// parent space's direction and cache slot rather than the space itself.
pub(crate) struct UnpositionedFloat<'c> {
  pub(crate) node: &'c RenderNode,
  /// The containing block's available size.
  pub(crate) available_size: AvailableLogicalSize,
  /// The size the float's percentages resolve against.
  pub(crate) percentage_size: AvailableLogicalSize,
  pub(crate) origin_bfc_offset: BfcOffset,
  /// The containing block's direction, which `float` and `clear` resolve against.
  pub(crate) direction: TextDirection,
  cache_slot: CacheSlot,
  pub(crate) line_clamp_state: LineClampFloatState,
  pub(crate) layout_result: Option<Rc<LayoutResult>>,
  /// The margins, in the containing block's line-relative directions.
  pub(crate) margins: LineBoxStrut,
}

impl<'c> UnpositionedFloat<'c> {
  pub(crate) fn new(
    node: &'c RenderNode,
    (available_size, percentage_size): (AvailableLogicalSize, AvailableLogicalSize),
    origin_bfc_offset: BfcOffset,
    parent_space: &ConstraintSpace,
    line_clamp_state: LineClampFloatState,
  ) -> Self {
    Self {
      node,
      available_size,
      percentage_size,
      origin_bfc_offset,
      direction: parent_space.direction,
      cache_slot: parent_space.cache_slot,
      line_clamp_state,
      layout_result: None,
      margins: LineBoxStrut::default(),
    }
  }

  /// Blink's `CreateConstraintSpaceForFloat`, without fragmentation: a new formatting context in
  /// the containing block's available size. Approximate: a float past the clamp point is not
  /// hidden here, as the line that holds it hides it.
  fn create_constraint_space_for_float(&self) -> ConstraintSpace {
    let mut space = ConstraintSpace::new(
      self.available_size.inline_size,
      self.node.context.style.inherited_data.direction,
    );

    space.available_block_size = self.available_size.block_size;
    space.percentage_resolution_inline_size = self.percentage_size.inline_size;
    space.percentage_resolution_block_size = self.percentage_size.block_size;
    space.is_new_formatting_context = true;
    space.cache_slot = self.cache_slot;
    space.is_line_clamp_clipped_float = self.line_clamp_state == LineClampFloatState::Clip;
    space
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
  fn layout_without_fragmentation(&mut self, layout: &mut LayoutFloat<'_, 'c>) -> Rc<LayoutResult> {
    if let Some(layout_result) = &self.layout_result {
      return Rc::clone(layout_result);
    }

    let layout_result = layout(self.node, &self.create_constraint_space_for_float());

    self.layout_result = Some(Rc::clone(&layout_result));
    self.margins = LineBoxStrut::from_physical(self.node.margin_px(), self.direction);

    layout_result
  }
}

/// Blink's `PositionedFloat`: a float placed in the block formatting context.
#[derive(Debug, Clone)]
pub(crate) struct PositionedFloat {
  pub(crate) layout_result: Rc<LayoutResult>,
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
    unpositioned_float
      .available_size
      .inline_size
      .unwrap_or(LayoutUnit::ZERO),
    unpositioned_float.direction,
    inline_size + unpositioned_float.margins.inline_sum(),
  )
}

/// Blink's `ComputeMarginBoxInlineSizeForUnpositionedFloat`.
pub(crate) fn compute_margin_box_inline_size_for_unpositioned_float<'c>(
  unpositioned_float: &mut UnpositionedFloat<'c>,
  layout: &mut LayoutFloat<'_, 'c>,
) -> LayoutUnit {
  let layout_result = unpositioned_float.layout_without_fragmentation(layout);

  (layout_result.size.inline_size + unpositioned_float.margins.inline_sum())
    .clamp_negative_to_zero()
}

/// Blink's `PositionFloat`: places the float in the first opportunity it fits, and adds it to the
/// exclusion space.
pub(crate) fn position_float<'c>(
  unpositioned_float: &mut UnpositionedFloat<'c>,
  exclusion_space: &mut ExclusionSpace,
  layout: &mut LayoutFloat<'_, 'c>,
) -> PositionedFloat {
  let layout_result = unpositioned_float.layout_without_fragmentation(layout);
  let opportunity = find_layout_opportunity_for_float(
    unpositioned_float,
    exclusion_space,
    layout_result.size.inline_size,
  );
  let margins = unpositioned_float.margins;
  // The margin box's offset.
  let mut float_margin_bfc_offset = opportunity.rect.start_offset;

  if unpositioned_float.is_line_right() {
    let float_margin_box_inline_size = layout_result.size.inline_size + margins.inline_sum();

    float_margin_bfc_offset.line_offset +=
      opportunity.rect.inline_size() - float_margin_box_inline_size;
  }

  // Blink's `CreateExclusionArea`.
  let end_offset = BfcOffset::new(
    float_margin_bfc_offset.line_offset
      + (layout_result.size.inline_size + margins.inline_sum()).clamp_negative_to_zero(),
    float_margin_bfc_offset.block_offset
      + (layout_result.size.block_size + margins.line_over + margins.line_under)
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
      float_margin_bfc_offset.line_offset
        + unpositioned_float
          .margins
          .line_left(unpositioned_float.direction),
      float_margin_bfc_offset.block_offset + margins.line_over,
    ),
  }
}
