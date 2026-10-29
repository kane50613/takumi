//! Blink's [`floats_utils`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/floats_utils.cc),
//! [`UnpositionedFloat`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/unpositioned_float.h)
//! and [`PositionedFloat`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/positioned_float.h),
//! without `shape-outside`.

use std::rc::Rc;

#[cfg(feature = "fragmentation")]
use crate::{
  geometry::NodeId,
  layout::{
    block_break_token::BlockBreakToken,
    break_appeal::BreakAppeal,
    constraint_space::BlockFragmentation,
    fragmentation_utils::{
      adjust_margins_for_fragmentation, is_break_inside, move_past_breakpoint_without_builder,
    },
    inline::{inline_break_token::InlineBreakToken, inline_item_text_index::InlineItemTextIndex},
    layout_style::LayoutStyle,
    logical_rect::LogicalBoxStrut,
  },
};

use crate::{
  layout::{
    bfc_rect::{BfcOffset, BfcRect},
    constraint_space::{CacheSlot, ConstraintSpace},
    exclusions::{
      exclusion_area::ExclusionArea, exclusion_space::ExclusionSpace,
      layout_opportunity::LayoutOpportunity,
    },
    fragmentation_layout_input::FragmentationLayoutInput,
    inline::{inline_item_result::LineBoxStrut, line_breaker::AtomicInlineLayout},
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
  /// The pages the float may break across, none outside a fragmentation context.
  #[cfg(feature = "fragmentation")]
  pub(crate) fragmentation: Option<FloatFragmentation>,
}

/// What a float in a fragmentation context breaks against: Blink's `UnpositionedFloat`
/// fragmentation fields.
#[cfg(feature = "fragmentation")]
#[derive(Clone)]
pub(crate) struct FloatFragmentation {
  pub(crate) node: NodeId,
  pub(crate) style: Rc<LayoutStyle>,
  /// Where the float resumes after a break inside it on an earlier page.
  pub(crate) break_token: Option<Rc<BlockBreakToken>>,
  /// The page's block size for its container's children.
  pub(crate) fragmentainer_block_size: LayoutUnit,
  /// Blink's `FragmentainerOffsetAtBfc`: the page offset of the block formatting context's
  /// block start.
  pub(crate) fragmentainer_offset_at_bfc: LayoutUnit,
  pub(crate) min_break_appeal: BreakAppeal,
  /// Whether nothing inside it can break, so it moves to the next page whole.
  pub(crate) is_monolithic: bool,
}

impl<'c> UnpositionedFloat<'c> {
  /// A float `node` of a line laid out in `space`, from `origin_bfc_block_offset`, resumed as
  /// `input` says.
  pub(crate) fn in_line(
    node: &'c RenderNode,
    space: &ConstraintSpace,
    origin_bfc_block_offset: LayoutUnit,
    input: &FragmentationLayoutInput,
    atomic_inline: &mut dyn AtomicInlineLayout<'c>,
  ) -> Self {
    let mut unpositioned_float = Self::new(
      node,
      space.available_and_percentage_sizes(),
      BfcOffset::new(space.bfc_offset.line_offset, origin_bfc_block_offset),
      space,
      space.line_clamp_data.float_state(),
    );

    atomic_inline.setup_float_for_fragmentation(&mut unpositioned_float, space, input);
    unpositioned_float
  }

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
      #[cfg(feature = "fragmentation")]
      fragmentation: None,
    }
  }

  /// Blink's `CreateConstraintSpaceForFloat`, without fragmentation.
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
  /// The break before the float that pushes it to the next page.
  #[cfg(feature = "fragmentation")]
  pub(crate) break_before_token: Option<Rc<BlockBreakToken>>,
}

impl PositionedFloat {
  /// Whether the page pushed the float to the next one whole, which leaves it out of this page.
  pub(crate) fn is_pushed_to_next_page(&self) -> bool {
    #[cfg(feature = "fragmentation")]
    return self.break_before_token.is_some();
    #[cfg(not(feature = "fragmentation"))]
    return false;
  }

  /// Blink's `BreakToken`: where the float resumes on a later page, none once it ended.
  #[cfg(feature = "fragmentation")]
  pub(crate) fn break_token(&self) -> Option<&Rc<BlockBreakToken>> {
    self
      .break_before_token
      .as_ref()
      .or(self.layout_result.break_info.break_token.as_ref())
  }

  /// Blink's `CreateForParallelBlockFlow` for a float of inline content whose item starts at
  /// `start`: where the float the page broke resumes, beside the lines after it.
  #[cfg(feature = "fragmentation")]
  pub(crate) fn parallel_flow_break_token(
    &self,
    start: InlineItemTextIndex,
  ) -> Option<InlineBreakToken> {
    self
      .break_token()
      .map(|token| InlineBreakToken::for_parallel_block_flow(start, Rc::clone(token)))
  }
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
  #[cfg(feature = "fragmentation")]
  if let Some(fragmentation) = unpositioned_float.fragmentation.clone() {
    return position_fragmented_float(unpositioned_float, exclusion_space, layout, &fragmentation);
  }

  let layout_result = unpositioned_float.layout_without_fragmentation(layout);
  let opportunity = find_layout_opportunity_for_float(
    unpositioned_float,
    exclusion_space,
    layout_result.size.inline_size,
  );
  let margins = unpositioned_float.margins;
  let float_margin_bfc_offset =
    float_margin_bfc_offset(unpositioned_float, &opportunity, &layout_result, margins);

  exclusion_space.add(create_exclusion_area(
    float_margin_bfc_offset,
    &layout_result,
    margins,
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
    #[cfg(feature = "fragmentation")]
    break_before_token: None,
  }
}

/// Where the margin box of a float of `layout_result` and `margins` goes in `opportunity`: at its
/// line-right edge for a line-right float.
fn float_margin_bfc_offset(
  unpositioned_float: &UnpositionedFloat<'_>,
  opportunity: &LayoutOpportunity,
  layout_result: &LayoutResult,
  margins: LineBoxStrut,
) -> BfcOffset {
  let mut offset = opportunity.rect.start_offset;

  if unpositioned_float.is_line_right() {
    let float_margin_box_inline_size = layout_result.size.inline_size + margins.inline_sum();

    offset.line_offset += opportunity.rect.inline_size() - float_margin_box_inline_size;
  }
  offset
}

/// Blink's `CreateExclusionArea`: the margin box of a float of `layout_result` and `margins` at
/// `float_margin_bfc_offset`.
fn create_exclusion_area(
  float_margin_bfc_offset: BfcOffset,
  layout_result: &LayoutResult,
  margins: LineBoxStrut,
  float_type: Float,
) -> ExclusionArea {
  let end_offset = BfcOffset::new(
    float_margin_bfc_offset.line_offset
      + (layout_result.size.inline_size + margins.inline_sum()).clamp_negative_to_zero(),
    float_margin_bfc_offset.block_offset
      + (layout_result.size.block_size + margins.line_over + margins.line_under)
        .clamp_negative_to_zero(),
  );

  ExclusionArea::new(
    BfcRect::new(float_margin_bfc_offset, end_offset),
    float_type,
  )
}

/// Blink's `PositionFloat` in a fragmentation context.
#[cfg(feature = "fragmentation")]
fn position_fragmented_float<'c>(
  unpositioned_float: &mut UnpositionedFloat<'c>,
  exclusion_space: &mut ExclusionSpace,
  layout: &mut LayoutFloat<'_, 'c>,
  fragmentation: &FloatFragmentation,
) -> PositionedFloat {
  let mut fragment_margins = LogicalBoxStrut::from_physical(
    unpositioned_float
      .node
      .margin_px()
      .map(LayoutUnit::from_float),
    unpositioned_float.direction,
  );

  adjust_margins_for_fragmentation(
    fragmentation.break_token.as_deref(),
    true,
    &mut fragment_margins,
  );

  let mut margins = LineBoxStrut::from(fragment_margins);
  // Without a layout to size it by, the float goes at its origin, and moves once it has one.
  let mut opportunity = unpositioned_float
    .layout_result
    .as_ref()
    .map(|layout_result| {
      find_layout_opportunity_for_float(
        unpositioned_float,
        exclusion_space,
        layout_result.size.inline_size,
      )
    });
  let mut fragmentainer_delta = opportunity.map_or(
    unpositioned_float.origin_bfc_offset.block_offset,
    |opportunity| opportunity.rect.start_offset.block_offset,
  ) + margins.line_over;
  let mut is_placed_optimistically = opportunity.is_none();

  unpositioned_float.margins = margins;

  let (layout_result, is_at_fragmentainer_start, opportunity) = loop {
    let fragmentainer_offset = fragmentation.fragmentainer_offset_at_bfc + fragmentainer_delta;
    let mut space = unpositioned_float.create_constraint_space_for_float();

    if !fragmentation.is_monolithic {
      space.block_fragmentation = Some(BlockFragmentation {
        fragmentainer_block_size: fragmentation.fragmentainer_block_size,
        fragmentainer_offset,
        requires_content_before_breaking: false,
        should_propagate_child_break_values: true,
        min_break_appeal: fragmentation.min_break_appeal,
        min_block_size_should_encompass_intrinsic_size: false,
      });
    }

    let is_at_fragmentainer_start = fragmentainer_offset - margins.line_over <= LayoutUnit::ZERO;
    let layout_result = layout(unpositioned_float.node, &space);
    let found = match opportunity {
      Some(opportunity) if !is_placed_optimistically => opportunity,
      _ => find_layout_opportunity_for_float(
        unpositioned_float,
        exclusion_space,
        layout_result.size.inline_size,
      ),
    };
    let found_delta = found.rect.start_offset.block_offset + margins.line_over;

    opportunity = Some(found);
    if is_placed_optimistically && fragmentainer_delta < found_delta {
      // Its width pushes it down, so it lays out again there.
      fragmentainer_delta = found_delta;
      is_placed_optimistically = false;
      continue;
    }
    break (layout_result, is_at_fragmentainer_start, found);
  };
  let break_token = layout_result.break_info.break_token.as_deref();
  let is_at_block_end = break_token.is_none_or(|token| token.is_at_block_end);
  let mut needs_break_before = false;

  if !is_at_fragmentainer_start {
    let fragmentainer_block_offset = fragmentation.fragmentainer_offset_at_bfc
      + opportunity.rect.start_offset.block_offset
      + margins.line_over;

    // The block-end margin goes to the fragment the float ends in.
    if !is_at_block_end {
      margins.line_under = LayoutUnit::ZERO;
    }
    // A float that ends past the page with its block-end margin moves to the next one, unless it
    // resumes there already.
    let overflows_page_end = is_at_block_end
      && fragmentainer_block_offset + layout_result.size.block_size + margins.line_under
        > fragmentation.fragmentainer_block_size
      && !is_break_inside(fragmentation.break_token.as_deref());

    needs_break_before = !move_past_breakpoint_without_builder(
      (
        fragmentation.fragmentainer_block_size,
        fragmentation.min_break_appeal,
      ),
      (fragmentation.node, &fragmentation.style),
      &layout_result,
      fragmentainer_block_offset,
      BreakAppeal::Perfect,
    ) || overflows_page_end;
  }

  let float_type = unpositioned_float.float_type();
  let float_margin_bfc_offset =
    float_margin_bfc_offset(unpositioned_float, &opportunity, &layout_result, margins);

  if !needs_break_before
    && !is_break_inside(fragmentation.break_token.as_deref())
    && exclusion_space.needs_break_before_float(unpositioned_float.clear_type())
  {
    needs_break_before = true;
  }

  if needs_break_before {
    // An exclusion past everything on the page keeps later floats from starting above it.
    let past_everything = BfcOffset::new(
      LayoutUnit::ZERO,
      fragmentation.fragmentainer_block_size - fragmentation.fragmentainer_offset_at_bfc,
    );

    exclusion_space.add(ExclusionArea::new(
      BfcRect::new(past_everything, past_everything),
      float_type,
    ));
    exclusion_space.set_has_break_before_float(float_type);
  } else {
    exclusion_space.add(create_exclusion_area(
      float_margin_bfc_offset,
      &layout_result,
      margins,
      float_type,
    ));
    if !is_at_block_end {
      exclusion_space.set_has_break_inside_float(float_type);
    }
  }

  PositionedFloat {
    bfc_offset: BfcOffset::new(
      float_margin_bfc_offset.line_offset + margins.line_left(unpositioned_float.direction),
      float_margin_bfc_offset.block_offset + margins.line_over,
    ),
    break_before_token: needs_break_before
      .then(|| Rc::new(BlockBreakToken::break_before(fragmentation.node, false))),
    layout_result,
  }
}
