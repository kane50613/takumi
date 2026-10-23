//! Blink's block fragmentation in
//! [`BlockLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/block_layout_algorithm.cc)
//! and the parts of
//! [`fragmentation_utils`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/fragmentation_utils.cc)
//! that read the fragment builder, for pages alone.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::rc::Rc;

use super::{BlockLayoutAlgorithm, InlineChild, PreviousInflowPosition};
use crate::{
  geometry::NodeId,
  layout::{
    break_appeal::BreakAppeal,
    break_token::BreakToken,
    constraint_space::ConstraintSpace,
    early_break::EarlyBreak,
    floats_utils::{FloatFragmentation, PositionedFloat, UnpositionedFloat},
    fragmentation_layout_input::{ChildBreakToken, FragmentationLayoutInput},
    fragmentation_utils::{
      BlockChildIterator, FlexItemBreak, adjust_margins_for_fragmentation,
      adjusted_margin_after_final_child_fragment, attempt_soft_break, break_before_child,
      calculate_break_appeal_before, calculate_break_between_value, finish_fragmentation,
      fragment_index, fragmentation_layout_input_for_child, is_break_inside, is_early_break_target,
      is_forced_break_value, move_past_breakpoint, setup_fragment_builder_for_fragmentation,
      setup_space_builder_for_fragmentation,
    },
    inline::inline_layout_algorithm::LineBoxFragment,
    layout_result::{BreakInfo, LayoutResult, LayoutStatus, OutOfFlowCandidate},
    layout_style::LayoutStyle,
    logical_rect::LogicalBoxStrut,
    margin_strut::MarginStrut,
  },
  layout_unit::LayoutUnit,
  style::{BreakBetween, Overflow},
};

impl<'r> BlockLayoutAlgorithm<'_, 'r> {
  /// Blink's `HasBlockFragmentation` of the space.
  pub(super) fn has_block_fragmentation(&self) -> bool {
    self.space.has_block_fragmentation()
  }

  /// Blink's `is_resuming_`: the layout resumes the box after a break inside it.
  pub(super) fn is_resuming(&self) -> bool {
    is_break_inside(self.input.break_token.as_deref())
  }

  /// Blink's `FragmentIndex` of the box's fragment this layout makes.
  pub(super) fn fragment_index(&self) -> usize {
    fragment_index(self.input.break_token.as_deref())
  }
  /// Whether this fragment holds every line of the box, which places the out-of-flow boxes of its
  /// inline content that sit on no line.
  pub(super) fn holds_every_line(&self) -> bool {
    !self.is_resuming() && !self.builder.should_break()
  }

  /// Blink's `SetupFragmentBuilderForFragmentation`, and the early break the constructor keeps.
  pub(super) fn setup_fragment_builder_for_fragmentation(&mut self) {
    if setup_fragment_builder_for_fragmentation(
      self.space,
      &self.style,
      self.inline_size,
      &mut self.builder,
    ) {
      self.border_padding.block_start = LayoutUnit::ZERO;
    }
  }

  /// The part of Blink's `Layout` that sets the margins up for a resumed fragment.
  pub(super) fn start_fragment(&mut self, previous: &mut PreviousInflowPosition) {
    let Some(token) = self.input.break_token.as_deref() else {
      return;
    };

    // Margins inside a box resumed after an unforced break adjoin the page boundary.
    if is_break_inside(Some(token)) && !token.is_forced_break {
      previous.margin_strut.discard_margins = true;
    }
    // Pushed past monolithic overflow on an earlier page, as if after a perfect breakpoint.
    if token.monolithic_overflow > LayoutUnit::ZERO {
      self.has_break_opportunity_before_next_child = true;
    }
  }

  /// Blink's `BlockChildIterator`: the children this fragment resumes or starts, with the tokens
  /// they resume at.
  pub(super) fn block_child_entries(&self) -> BlockChildIterator {
    BlockChildIterator::new(
      self.tree.child_nodes(self.node),
      self.input.break_token.as_deref(),
    )
  }

  /// The early break check in Blink's `Layout`: whether the layout broke before `child`, or the
  /// next line as `None`, the breakpoint it had to break at.
  pub(super) fn break_before_early_break_target(
    &mut self,
    child: Option<NodeId>,
    previous: &mut PreviousInflowPosition,
  ) -> Result<bool, LayoutStatus> {
    let Some(early_break) = self.input.early_break.clone() else {
      return Ok(false);
    };

    if !is_early_break_target(&early_break, &self.builder, child) {
      return Ok(false);
    }
    // The breakpoint may be where the block offset resolves, which then has to come first.
    if !self.resolve_bfc_block_offset_here(previous) {
      return Err(LayoutStatus::BfcBlockOffsetResolved);
    }

    self
      .builder
      .add_break_before_child(child, BreakAppeal::Perfect, false);
    self.consume_remaining_fragmentainer_space(previous);
    Ok(true)
  }

  /// This box's block size from its first fragment to the end of this one, `block_size` tall:
  /// what it knows of its stitched size without the fragments to come.
  pub(super) fn stitched_block_size(&self, block_size: LayoutUnit) -> LayoutUnit {
    if self.builder.involved_in_block_fragmentation() {
      self.builder.fragments_total_block_size
    } else {
      block_size
    }
  }

  /// Whether the out-of-flow boxes this box lays out break across its fragments. A box that clips
  /// its block overflow and ends in this fragment finishes them here, so they add no pages, as
  /// Blink's `DisableFurtherFragmentation` keeps those of a clipped containing block from it.
  pub(super) fn breaks_out_of_flow_children(&self) -> bool {
    self.builder.involved_in_block_fragmentation()
      && (self.style.overflow.y == Overflow::Visible || self.builder.should_break())
  }

  /// A box that clips overflow marks the out-of-flow boxes deferred to the fragmentation root on
  /// their way up through it.
  pub(super) fn mark_deferred_out_of_flow_inside_clipped_container(&mut self) {
    if !self.builder.involved_in_block_fragmentation() || self.style.overflow.y == Overflow::Visible
    {
      return;
    }
    for candidate in &mut self.out_of_flow {
      candidate.mark_inside_clipped_container();
    }
  }

  /// Blink's `HandleOutOfFlowPositioned` for an out-of-flow `child` an earlier page broke inside
  /// or pushed here: whether it resumes at `token`, which holds its offset. A forced break before
  /// the box leaves it to work its static position out again, as does a box held back for the
  /// block size of this one.
  pub(super) fn resumes_out_of_flow(&mut self, child: NodeId, token: &ChildBreakToken) -> bool {
    let Some(token) = token.as_ref().filter(|token| !token.is_forced_break) else {
      return false;
    };

    let candidate = match token.held_static_position {
      Some(mut static_position) => {
        static_position.offset.y -= self.previously_consumed_block_size();

        let mut candidate = OutOfFlowCandidate::new(child, static_position);

        candidate.requires_content_before_breaking = self.builder.requires_content_before_breaking;
        candidate
      }
      None => OutOfFlowCandidate::resumed(
        child,
        Rc::clone(token),
        self.builder.requires_content_before_breaking,
      ),
    };

    self.out_of_flow.push(candidate);
    true
  }

  /// Blink's `HasInflowChildBreakInside`: something in this flow broke, so no sibling follows.
  pub(super) fn has_inflow_child_break_inside(&self) -> bool {
    self.space.has_block_fragmentation() && self.builder.has_inflow_child_break_inside
  }

  /// Blink's `SetHasSeenAllChildren`.
  pub(super) fn set_has_seen_all_children(&mut self) {
    self.builder.has_seen_all_children = true;
  }

  /// Whether `token` resumes a child after a break inside it.
  pub(super) fn is_break_inside_child(token: &ChildBreakToken) -> bool {
    is_break_inside(token.as_deref())
  }

  /// Blink's `FragmentIndex` of a child resumed at `token`.
  pub(super) fn child_fragment_index(token: &ChildBreakToken) -> usize {
    fragment_index(token.as_deref())
  }

  /// A block child laid out in `space`, resumed at `token`, with the breakpoint this layout has
  /// to break at inside it.
  pub(super) fn lay_out_child(
    &mut self,
    child: NodeId,
    space: &ConstraintSpace,
    token: &ChildBreakToken,
  ) -> Rc<LayoutResult> {
    let input = self.child_layout_input(child, token);

    self.tree.layout_fragment(child, space, &input)
  }

  /// Blink's `GetFragmentationLayoutInputForChild`: `child` resumed at `token`, with the
  /// breakpoint this layout has to break at inside it.
  pub(super) fn child_layout_input(
    &self,
    child: NodeId,
    token: &ChildBreakToken,
  ) -> FragmentationLayoutInput {
    fragmentation_layout_input_for_child(child, token.clone(), self.input.early_break.as_deref())
  }

  /// The forced break check of Blink's `HandleFloat` and `HandleOutOfFlowPositioned`: whether a
  /// break the content before forces goes before the float or out-of-flow box `child`.
  pub(super) fn breaks_before_float_or_out_of_flow(&mut self, child: NodeId) -> bool {
    if !self.space.has_block_fragmentation() {
      return false;
    }

    let break_between = self.builder.joined_break_between_value(BreakBetween::Auto);

    if !is_forced_break_value(self.space, break_between) {
      return false;
    }
    self.builder.add_break_before_float_or_out_of_flow(child);
    true
  }

  /// The monolithic overflow inside a float resumed at `token` that an earlier page pushed into
  /// this one, less this container's own, as floats establish a parallel flow.
  pub(super) fn float_monolithic_overflow(&self, token: &ChildBreakToken) -> LayoutUnit {
    let Some(token) = token.as_deref() else {
      return LayoutUnit::ZERO;
    };

    token.monolithic_overflow
      - self
        .input
        .break_token
        .as_deref()
        .map_or(LayoutUnit::ZERO, |container| container.monolithic_overflow)
  }

  /// Gives a float in a fragmentation context the pages it breaks across.
  pub(super) fn setup_float_for_fragmentation(
    &self,
    unpositioned_float: &mut UnpositionedFloat<'_>,
    child: NodeId,
    token: &ChildBreakToken,
  ) {
    if !self.space.has_block_fragmentation() {
      return;
    }

    unpositioned_float.fragmentation = Some(FloatFragmentation {
      node: child,
      style: Rc::clone(self.tree.layout_style_rc(child)),
      break_token: token.clone(),
      fragmentainer_block_size: self
        .builder
        .fragmentainer_capacity(true)
        .unwrap_or(LayoutUnit::MAX),
      fragmentainer_offset_at_bfc: self.fragmentainer_offset_at_bfc(),
      min_break_appeal: self.space.min_break_appeal(),
      is_monolithic: self.tree.is_monolithic(child),
    });
  }

  /// The break before a float the page pushed to the next one, after which the layout goes on
  /// with this container, as the float establishes a parallel flow.
  pub(super) fn breaks_before_positioned_float(
    &mut self,
    child: NodeId,
    positioned: &PositionedFloat,
  ) -> bool {
    if positioned.break_before_token.is_none() {
      return false;
    }
    self.builder.add_break_token_before(child);
    true
  }

  /// Blink's clearance check in `MovePastBreakpoint`: a child that clears a float a page broke
  /// before or inside goes to the next page too.
  fn needs_clearance_past_fragmentainer(
    &self,
    child: Option<(NodeId, &LayoutStyle)>,
    result: &LayoutResult,
  ) -> bool {
    child.is_some_and(|(_, style)| {
      result.status == LayoutStatus::Success
        && result.break_info.is_first_for_node
        && self
          .exclusion_space
          .needs_clearance_past_fragmentainer(style.clear)
    })
  }

  /// The fragmentation part of Blink's `ComputeChildData`.
  pub(super) fn adjust_child_data_for_fragmentation(
    &self,
    token: Option<&ChildBreakToken>,
    margins: &mut LogicalBoxStrut,
    margin_strut: &mut MarginStrut,
    logical_block_offset: &mut LayoutUnit,
  ) {
    let Some(token) = token.and_then(Option::as_deref) else {
      return;
    };

    adjust_margins_for_fragmentation(Some(token), false, margins);

    // Margins after a forced break stay, where a break otherwise discards them.
    if token.is_forced_break {
      *margin_strut = MarginStrut::default();
    }
    // Only the outermost container that recorded monolithic overflow steers clear of it.
    if token.monolithic_overflow > LayoutUnit::ZERO
      && self
        .input
        .break_token
        .as_deref()
        .is_none_or(|container| container.monolithic_overflow == LayoutUnit::ZERO)
    {
      *logical_block_offset += token.monolithic_overflow;
    }
  }

  /// Blink's `SetupSpaceBuilderForFragmentation` for a block child, or the next line as `None`,
  /// of the space `builder` builds.
  pub(super) fn setup_space_builder_for_fragmentation(
    &self,
    builder: &mut ConstraintSpace,
    child: Option<NodeId>,
    is_new_fc: bool,
    child_bfc_block_offset: Option<LayoutUnit>,
  ) {
    let fragmentainer_offset_delta = if is_new_fc {
      child_bfc_block_offset.unwrap_or_default() - self.space.expected_bfc_block_offset()
    } else {
      builder.expected_bfc_block_offset() - self.space.expected_bfc_block_offset()
    };

    setup_space_builder_for_fragmentation(
      self.space,
      &self.builder,
      child.is_some_and(|child| self.tree.is_monolithic(child)),
      fragmentainer_offset_delta,
      builder,
    );
  }

  /// The block fragmentation part of Blink's `BoxFragmentBuilder::AddResult` for a block child
  /// `node` laid out as `result` at `block_offset`.
  pub(super) fn propagate_break_info(
    &mut self,
    node: NodeId,
    result: &LayoutResult,
    block_offset: LayoutUnit,
  ) {
    if !self.space.has_block_fragmentation() {
      return;
    }

    let child_style = Rc::clone(self.tree.layout_style_rc(node));

    self
      .builder
      .add_result(self.space, &child_style, result, block_offset);
  }

  /// The line count and break token Blink's `FragmentBuilder::PropagateFromFragment` reads off a
  /// line it adds at `block_offset`, and the break data of its block-in-inline, which Blink's
  /// `AddResult` propagates in its place.
  pub(super) fn add_line_for_fragmentation(
    &mut self,
    line: &LineBoxFragment<'_, 'r>,
    block_offset: LayoutUnit,
  ) {
    if !self.space.has_block_fragmentation() {
      return;
    }
    if let Some((render_node, result)) = line.block_in_inline() {
      let node = self.tree.atomic_inline_node(render_node);
      let child_style = Rc::clone(self.tree.layout_style_rc(node));

      self
        .builder
        .propagate_result(self.space, &child_style, result, block_offset);
    }
    // A line that resumes a block-in-inline in a parallel flow counts for neither `orphans` nor
    // `widows`, nor where the lines resume.
    if !line.is_line_for_parallel_flow {
      self
        .builder
        .add_line(line.break_token.clone(), line.is_empty_line_box);
    }
  }

  /// Where the lines of a box resumed after a break inside start, or `false` when they ended on
  /// an earlier page. The lines take the break tokens in order: those of the blocks-in-inline in
  /// parallel flows, then where the lines broke.
  pub(super) fn resume_inline_child(&mut self, inline_child: &mut InlineChild<'_, 'r>) -> bool {
    let Some(token) = self.input.break_token.clone() else {
      return true;
    };

    // The out-of-flow boxes come first, as Blink's `BlockChildIterator` gives them.
    for child_token in &token.child_break_tokens {
      if let BreakToken::Block(child_token) = child_token {
        self.resumes_out_of_flow(child_token.node, &Some(Rc::clone(child_token)));
      }
    }
    inline_child.resumed_break_tokens = token
      .child_break_tokens
      .iter()
      .filter_map(|token| match token {
        BreakToken::Inline(token) => Some(token.clone()),
        BreakToken::Block(_) => None,
      })
      .collect();

    match inline_child.resumed_break_tokens.pop_front() {
      Some(inline_break_token) => {
        inline_child.break_token = Some(inline_break_token);
        // The marker went beside a line on an earlier page.
        inline_child.unpositioned_list_marker = None;
        true
      }
      None => !token.has_seen_all_children,
    }
  }

  /// Blink's `IsLineForParallelFlow`.
  pub(super) fn is_line_for_parallel_flow(line: &LineBoxFragment<'_, 'r>) -> bool {
    line.is_line_for_parallel_flow
  }

  /// The break tokens of the blocks-in-inline the last line resumes in parallel flows, which
  /// Blink's `HandleInflow` adds to the fragment from the inline child layout context.
  pub(super) fn add_parallel_flow_break_tokens(&mut self, inline_child: &mut InlineChild<'_, 'r>) {
    if !self.space.has_block_fragmentation() {
      return;
    }

    for token in inline_child.context.parallel_flow_break_tokens.drain(..) {
      self.builder.add_parallel_flow_break_token(token);
    }
  }

  /// Blink's `BlockChildIterator::NextChild` in an inline formatting context: the next line
  /// resumes where the last one broke, and after a line that ends nothing, at the next break
  /// token the box resumed at.
  pub(super) fn advance_inline_break_token(&mut self, inline_child: &mut InlineChild<'_, 'r>) {
    if inline_child.break_token.is_some() {
      inline_child.resumed_break_tokens.clear();
    } else {
      inline_child.break_token = inline_child.resumed_break_tokens.pop_front();
    }
  }

  /// Blink's `HasBreakOpportunityBeforeNextChild` for a child laid out as `result`, a block child
  /// resumed at `token` or a line as `None`.
  pub(super) fn update_break_opportunity_before_next_child(
    &mut self,
    result: &LayoutResult,
    token: Option<&ChildBreakToken>,
  ) {
    if !self.space.has_block_fragmentation() || self.has_break_opportunity_before_next_child {
      return;
    }

    self.has_break_opportunity_before_next_child = match token {
      Some(token) => token.as_deref().is_none_or(|token| !token.is_at_block_end),
      None => result.size.block_size != LayoutUnit::ZERO,
    };
  }

  /// The fragmentation part of Blink's `ComputeInflowPosition`: a child that broke inside carries
  /// no block-end margin past the break.
  pub(super) fn discard_margins_after_break(result: &LayoutResult, margin_strut: &mut MarginStrut) {
    if result
      .break_info
      .break_token
      .as_deref()
      .is_some_and(|token| !token.is_at_block_end)
    {
      *margin_strut = MarginStrut::default();
    }
  }

  /// Blink's `ConsumeRemainingFragmentainerSpace`: the box fills the rest of the page after a
  /// break.
  fn consume_remaining_fragmentainer_space(&self, previous: &mut PreviousInflowPosition) {
    if self.space.has_block_fragmentation() {
      previous.logical_block_offset = previous
        .logical_block_offset
        .max(self.builder.fragmentainer_space_left(true));
    }
  }

  /// Blink's `FragmentainerOffsetAtBfc` of the builder.
  fn fragmentainer_offset_at_bfc(&self) -> LayoutUnit {
    self.builder.fragmentainer_offset(true) - self.space.expected_bfc_block_offset()
  }

  /// Blink's `BreakBeforeChildIfNeeded`: whether the layout broke before a block child of
  /// `child`, or a line as `None`, laid out as `result` at `bfc_block_offset`.
  pub(super) fn break_before_child_if_needed(
    &mut self,
    child: Option<(NodeId, &LayoutStyle)>,
    result: &LayoutResult,
    previous: &mut PreviousInflowPosition,
    bfc_block_offset: LayoutUnit,
    is_separated_by_floats: bool,
  ) -> Result<bool, LayoutStatus> {
    if !self.space.has_block_fragmentation() {
      return Ok(false);
    }

    let has_container_separation =
      self.has_break_opportunity_before_next_child || is_separated_by_floats;
    let fragmentainer_block_offset = self.fragmentainer_offset_at_bfc() + bfc_block_offset;
    let child_style = child.map(|(_, style)| style);

    if has_container_separation {
      let break_between = calculate_break_between_value(child_style, result, &self.builder);

      if is_forced_break_value(self.space, break_between) {
        break_before_child(
          self.space,
          &mut self.builder,
          child,
          Some(result),
          BreakAppeal::Perfect,
          true,
        );
        self.consume_remaining_fragmentainer_space(previous);
        return Ok(true);
      }
    }

    let mut appeal_before = calculate_break_appeal_before(
      self.space,
      child_style,
      result,
      &self.builder,
      has_container_separation,
    );

    if !self.needs_clearance_past_fragmentainer(child, result)
      && move_past_breakpoint(
        self.space,
        &mut self.builder,
        child,
        result,
        fragmentainer_block_offset,
        appeal_before,
        &mut FlexItemBreak::default(),
      )
    {
      return Ok(false);
    }

    // A line honors `orphans` and `widows` where it can.
    if child.is_none() && self.builder.line_count != 0 {
      let line_count = self.builder.line_count;
      let inherited = &self
        .tree
        .render_node(self.node)
        .context
        .style
        .misc_inherited1_data;
      let orphans = inherited.orphans.get();
      let widows = inherited.widows.get();

      if self.first_overflowing_line == 0 {
        self.first_overflowing_line = line_count;
      }

      let mut minimum_line_count = orphans;

      // Widows before an earlier break are orphans after it.
      if self.input.break_token.is_some() {
        minimum_line_count = minimum_line_count.max(widows);
      }

      if line_count < minimum_line_count {
        appeal_before = appeal_before.min(BreakAppeal::ViolatingOrphansAndWidows);
      } else {
        let widows_found = line_count - self.first_overflowing_line + 1;

        if widows_found < widows || result.size.block_size == LayoutUnit::ZERO {
          // Lay out on to find where to break so the next page gets its widows.
          return Ok(false);
        }
      }
      self.fit_all_lines = true;
    }

    if !attempt_soft_break(
      self.space,
      &mut self.builder,
      child,
      Some(result),
      appeal_before,
      &FlexItemBreak::default(),
    ) {
      return Err(LayoutStatus::NeedsEarlierBreak);
    }
    self.consume_remaining_fragmentainer_space(previous);
    Ok(true)
  }

  /// Blink's `AdjustedMarginAfterFinalChildFragment`.
  pub(super) fn adjusted_margin_after_final_child_fragment(
    &self,
    block_offset: LayoutUnit,
    block_end_margin: LayoutUnit,
  ) -> LayoutUnit {
    adjusted_margin_after_final_child_fragment(
      self.space,
      &self.builder,
      block_offset,
      block_end_margin,
    )
  }

  /// Blink's `ShouldIncludeBlockEndBorderPadding`.
  pub(super) fn should_include_block_end_border_padding(&self) -> bool {
    self
      .builder
      .should_include_block_end_border_padding(self.space.is_new_formatting_context)
  }

  /// Blink's `BorderPadding`: the box's `border_padding`, with the cloned decorations of every
  /// fragment so far.
  pub(super) fn border_padding_for_size(&self, border_padding: LogicalBoxStrut) -> LogicalBoxStrut {
    self.builder.border_padding_for_size(border_padding)
  }

  /// Blink's `ConsumedBlockSize` of the incoming break token.
  pub(super) fn previously_consumed_block_size(&self) -> LayoutUnit {
    self.builder.previously_consumed_block_size()
  }

  /// Blink's `FinalizeForFragmentation`: this fragment's block size, the box `block_size` tall
  /// across its fragments.
  pub(super) fn finalize_for_fragmentation(
    &mut self,
    block_size: LayoutUnit,
  ) -> Result<LayoutUnit, LayoutStatus> {
    if !self.builder.involved_in_block_fragmentation() {
      return Ok(block_size);
    }

    self.builder.fragments_total_block_size = block_size;
    self.builder.intrinsic_block_size = self.intrinsic_block_size;

    if self.space.is_table_cell {
      // Breaking before a cell's trailing decorations would disturb the row's stretching.
      self
        .builder
        .should_prevent_break_before_block_end_decorations = true;
    }
    if self.tree.has_inline_children(self.node)
      && self.input.early_break.is_none()
      && self.space.has_block_fragmentation()
    {
      if self.builder.has_inflow_child_break_inside || self.first_overflowing_line != 0 {
        let line_count = self.builder.line_count;

        if self.first_overflowing_line != 0 && self.first_overflowing_line < line_count {
          let inherited = &self
            .tree
            .render_node(self.node)
            .context
            .style
            .misc_inherited1_data;
          let line_number = if self.fit_all_lines {
            self.first_overflowing_line
          } else {
            // Every line fit, so the widows only come from breaking earlier than the overflow.
            line_count
              .saturating_sub(inherited.widows.get())
              .max(line_count.min(inherited.orphans.get()))
          };

          self.builder.early_break =
            Some(Rc::new(EarlyBreak::line(line_number, BreakAppeal::Perfect)));
          return Err(LayoutStatus::NeedsEarlierBreak);
        }
      } else {
        self.update_early_break_between_lines();
      }
    }

    // A float in this formatting context that broke takes its content past the page, as a
    // child breaking inside would.
    self.builder.has_float_break =
      self.space.is_new_formatting_context && self.exclusion_space.has_fragmentainer_break();
    finish_fragmentation(&mut self.builder)?;
    self.intrinsic_block_size = self.builder.intrinsic_block_size;
    Ok(self.builder.fragment_block_size)
  }

  /// Blink's `UpdateEarlyBreakBetweenLines`: the best breakpoint between lines, for a later sibling
  /// that does not fit to come back to.
  fn update_early_break_between_lines(&mut self) {
    let line_count = self.builder.line_count;

    if line_count < 2 {
      return;
    }

    let inherited = &self
      .tree
      .render_node(self.node)
      .context
      .style
      .misc_inherited1_data;
    let orphans = inherited.orphans.get();
    let widows = inherited.widows.get();
    let mut line_number = line_count
      .saturating_sub(widows)
      .max((line_count - 1).min(orphans));
    let mut appeal = BreakAppeal::Perfect;

    // Too few lines for `orphans` and `widows`: breaking before the last line fits the most.
    if line_number < orphans || line_count - line_number < widows {
      line_number = line_count - 1;
      appeal = BreakAppeal::ViolatingOrphansAndWidows;
    }
    if self
      .builder
      .early_break_appeal()
      .is_some_and(|early_appeal| early_appeal > appeal)
    {
      return;
    }
    self.builder.early_break = Some(Rc::new(EarlyBreak::line(line_number, appeal)));
  }

  /// Blink's `SidesToInclude` on `result`.
  pub(super) fn exclude_broken_sides(&self, result: &mut LayoutResult) {
    self.builder.exclude_broken_sides(result);
  }

  /// What the box's layout result tells its container about breaking.
  pub(super) fn break_info(&mut self) -> BreakInfo {
    self.builder.has_unpositioned_list_marker = self.list_marker.is_some();
    self.builder.finish_break_info(self.node)
  }

  /// Whether the list item still has its outside marker to place: a fresh layout does, and a
  /// resumed one when the page before broke before placing it.
  pub(super) fn keeps_list_marker(&self) -> bool {
    !self.is_resuming()
      || self
        .input
        .break_token
        .as_deref()
        .is_some_and(|token| token.has_unpositioned_list_marker)
  }

  /// Blink's `HandleNonsuccessfulLayoutResult`: the layout again to break at the breakpoint it
  /// found, or the aborted one.
  pub(super) fn handle_nonsuccessful_layout_result(self, status: LayoutStatus) -> LayoutResult {
    if status == LayoutStatus::NeedsEarlierBreak
      && let Some(early_break) = self.builder.early_break.clone()
    {
      let input = FragmentationLayoutInput {
        break_token: self.input.break_token.clone(),
        early_break: Some(early_break),
      };

      return BlockLayoutAlgorithm::new(self.tree, self.node, self.space, &input).layout();
    }

    let mut result = self.abort(status);

    result.break_info.early_break = self.builder.early_break.clone();
    result
  }
}
