//! Blink's block fragmentation in
//! [`BlockLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/block_layout_algorithm.cc)
//! and the parts of
//! [`fragmentation_utils`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/fragmentation_utils.cc)
//! that read the fragment builder, for pages alone. Approximate: floats and out-of-flow boxes do
//! not break, a block-in-inline does not break inside its line, and `box-decoration-break: clone`
//! sizes a box without the border and padding its earlier fragments repeat.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::rc::Rc;

use super::{BlockLayoutAlgorithm, ChildBreakToken, InlineChild, PreviousInflowPosition};
use crate::{
  geometry::NodeId,
  layout::{
    block_break_token::BlockBreakToken,
    break_appeal::BreakAppeal,
    break_token::BreakToken,
    constraint_space::{BlockFragmentation, ConstraintSpace},
    early_break::EarlyBreak,
    fragmentation_layout_input::FragmentationLayoutInput,
    fragmentation_utils::{
      adjust_margins_for_fragmentation, block_size_for_fragmentation,
      calculate_break_appeal_before, calculate_break_appeal_inside, calculate_break_between_value,
      enter_early_break_in_child, fragment_index, is_break_inside,
      is_breakable_at_start_of_resumed_container, is_early_break_target, is_forced_break_value,
    },
    inline::inline_layout_algorithm::LineBoxFragment,
    layout_result::{BreakInfo, LayoutResult, LayoutStatus},
    layout_style::LayoutStyle,
    length_utils::LengthResolver,
    logical_rect::LogicalBoxStrut,
    margin_strut::MarginStrut,
  },
  layout_unit::LayoutUnit,
  style::BoxDecorationBreak,
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

  /// Blink's `SetupFragmentBuilderForFragmentation`, and the early break the constructor keeps.
  pub(super) fn setup_fragment_builder_for_fragmentation(&mut self) {
    let previous_break_token = self.input.break_token.clone();

    if !self.space.has_block_fragmentation() && previous_break_token.is_none() {
      return;
    }

    if let Some(token) = previous_break_token
      .as_deref()
      .filter(|token| !token.is_break_before)
    {
      self.builder.sequence_number = token.sequence_number + 1;
      self.builder.is_first_for_node = false;
    }

    let space_left = self.builder.fragmentainer_space_left(false);
    let clone_box_start_decorations = self.should_clone_block_start_border_padding();
    let mut clone_box_end_decorations = clone_box_start_decorations;

    if self.space.has_block_fragmentation() {
      let mut requires_content_before_breaking = self.space.requires_content_before_breaking();
      let resolver = LengthResolver::new(self.space, &self.style);
      let max_block_size = resolver
        .block_size_for_fragment(Some(LayoutUnit::MAX), Some(self.inline_size), None)
        .unwrap_or(LayoutUnit::MAX);

      if max_block_size != LayoutUnit::MAX
        && max_block_size - self.previously_consumed_block_size() <= space_left
      {
        self.builder.is_known_to_fit_in_fragmentainer = true;
        clone_box_end_decorations = false;
        // Blink's `MustStayInCurrentFragmentainer`.
        if self.builder.is_first_for_node {
          requires_content_before_breaking = true;
        }
      }
      if clone_box_end_decorations {
        self.builder.should_clone_box_end_decorations = true;
        self
          .builder
          .should_prevent_break_before_block_end_decorations = true;
      }
      self.builder.requires_content_before_breaking = requires_content_before_breaking;
    }

    if is_break_inside(previous_break_token.as_deref()) && !clone_box_start_decorations {
      self.border_padding.block_start = LayoutUnit::ZERO;
      self.builder.border_padding.block_start = LayoutUnit::ZERO;
    }
  }

  /// Blink's `ShouldCloneBlockStartBorderPadding`.
  fn should_clone_block_start_border_padding(&self) -> bool {
    if self.builder.box_decoration_break != BoxDecorationBreak::Clone {
      return false;
    }

    let Some(previous_break_token) = self.builder.previous_break_token.as_deref() else {
      return true;
    };

    if previous_break_token.monolithic_overflow > LayoutUnit::ZERO
      && self.builder.fragmentainer_space_left(false)
        < self.builder.border_padding.block_start + self.builder.border_padding.block_end
    {
      return false;
    }
    !previous_break_token.is_at_block_end
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
  pub(super) fn block_child_entries(&self) -> Vec<(NodeId, ChildBreakToken)> {
    let children = self.tree.child_nodes(self.node);
    let Some(container_break_token) = self.input.break_token.as_deref() else {
      return children.into_iter().map(|child| (child, None)).collect();
    };
    let child_break_tokens: Vec<Rc<BlockBreakToken>> = container_break_token
      .child_break_tokens
      .iter()
      .filter_map(|token| match token {
        BreakToken::Block(token) => Some(Rc::clone(token)),
        BreakToken::Inline(_) => None,
      })
      .collect();

    if child_break_tokens.is_empty() && container_break_token.has_seen_all_children {
      return Vec::new();
    }

    let next_unstarted_index = match child_break_tokens.last() {
      Some(last) if !container_break_token.has_seen_all_children => children
        .iter()
        .position(|&child| child == last.node)
        .map_or(children.len(), |index| index + 1),
      Some(_) => children.len(),
      None => 0,
    };

    child_break_tokens
      .into_iter()
      .map(|token| (token.node, Some(token)))
      .chain(
        children[next_unstarted_index..]
          .iter()
          .map(|&child| (child, None)),
      )
      .collect()
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

  /// Blink's `HasInflowChildBreakInside`: something in this flow broke, so no sibling follows.
  pub(super) fn has_inflow_child_break_inside(&self) -> bool {
    self.space.has_block_fragmentation() && self.builder.has_inflow_child_break_inside()
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
  /// to break at inside it: Blink's `GetFragmentationLayoutInputForChild`.
  pub(super) fn lay_out_child(
    &mut self,
    child: NodeId,
    space: &ConstraintSpace,
    token: &ChildBreakToken,
  ) -> Rc<LayoutResult> {
    let input = FragmentationLayoutInput {
      break_token: token.clone(),
      early_break: self
        .input
        .early_break
        .as_deref()
        .and_then(|early_break| enter_early_break_in_child(child, early_break)),
    };

    self.tree.layout_fragment(child, space, &input)
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
    let Some(parent) = self.space.block_fragmentation else {
      return;
    };

    if child.is_some_and(|child| self.tree.is_monolithic(child)) {
      return;
    }

    let fragmentainer_offset_delta = if is_new_fc {
      child_bfc_block_offset.unwrap_or_default() - self.space.expected_bfc_block_offset()
    } else {
      builder.expected_bfc_block_offset() - self.space.expected_bfc_block_offset()
    };
    let fragmentainer_offset = self.builder.fragmentainer_offset(true) + fragmentainer_offset_delta;

    builder.block_fragmentation = Some(BlockFragmentation {
      fragmentainer_block_size: self
        .builder
        .fragmentainer_capacity(true)
        .unwrap_or(parent.fragmentainer_block_size),
      fragmentainer_offset,
      requires_content_before_breaking: self.builder.requires_content_before_breaking,
      should_propagate_child_break_values: true,
      min_break_appeal: parent.min_break_appeal,
    });
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
    let is_floating_or_out_of_flow = child_style.is_floating() || child_style.is_out_of_flow();

    self.builder.add_child_break_token(result);
    self
      .builder
      .propagate_break_info(result, block_offset, is_floating_or_out_of_flow);
    if self.space.should_propagate_child_break_values() {
      self
        .builder
        .propagate_child_break_values(result, &child_style);
    }
  }

  /// The line count and break token Blink's `FragmentBuilder::PropagateFromFragment` reads off a
  /// line it adds.
  pub(super) fn add_line_for_fragmentation(&mut self, line: &LineBoxFragment<'_, 'r>) {
    if !self.space.has_block_fragmentation() {
      return;
    }

    self
      .builder
      .add_line(line.break_token, line.is_empty_line_box);
  }

  /// Where the lines of a box resumed after a break inside start, or `false` when they ended on
  /// an earlier page.
  pub(super) fn resume_inline_child(&mut self, inline_child: &mut InlineChild<'_, 'r>) -> bool {
    let Some(token) = self.input.break_token.as_deref() else {
      return true;
    };
    let inline_break_token = token
      .child_break_tokens
      .iter()
      .rev()
      .find_map(|token| match token {
        BreakToken::Inline(token) => Some(*token),
        BreakToken::Block(_) => None,
      });

    match inline_break_token {
      Some(inline_break_token) => {
        inline_child.break_token = Some(inline_break_token);
        // The marker went beside a line on an earlier page.
        inline_child.unpositioned_list_marker = None;
        true
      }
      None => !token.has_seen_all_children,
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
    let child_node = child.map(|(node, _)| node);

    if has_container_separation {
      let break_between = calculate_break_between_value(child_style, result, &self.builder);

      if is_forced_break_value(self.space, break_between) {
        self.break_before_child(child_node, result, BreakAppeal::Perfect, true);
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

    if self.move_past_breakpoint(child, result, fragmentainer_block_offset, appeal_before) {
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

    if !self.attempt_soft_break(child_node, result, appeal_before) {
      return Err(LayoutStatus::NeedsEarlierBreak);
    }
    self.consume_remaining_fragmentainer_space(previous);
    Ok(true)
  }

  /// Blink's `BreakBeforeChild`.
  fn break_before_child(
    &mut self,
    child: Option<NodeId>,
    result: &LayoutResult,
    appeal: BreakAppeal,
    is_forced_break: bool,
  ) {
    if self.space.should_propagate_child_break_values()
      && !is_forced_break
      && let Some(child) = child
    {
      let child_style = Rc::clone(self.tree.layout_style_rc(child));

      self
        .builder
        .propagate_child_break_values(result, &child_style);
    }

    self
      .builder
      .add_break_before_child(child, appeal, is_forced_break);
  }

  /// Blink's `MovePastBreakpoint`: whether the child fits, or breaking before it is no better
  /// than going on.
  fn move_past_breakpoint(
    &mut self,
    child: Option<(NodeId, &LayoutStyle)>,
    result: &LayoutResult,
    fragmentainer_block_offset: LayoutUnit,
    appeal_before: BreakAppeal,
  ) -> bool {
    if result.status != LayoutStatus::Success {
      return false;
    }
    if child.is_some() && !result.break_info.is_first_for_node {
      return true;
    }

    let Some(fragmentainer_block_size) = self.builder.fragmentainer_capacity(true) else {
      return true;
    };
    let break_token = result.break_info.break_token.as_deref();
    let space_left = fragmentainer_block_size - fragmentainer_block_offset;
    // Breaking at the page start makes no progress.
    let refuse_break_before = space_left >= fragmentainer_block_size
      && !is_breakable_at_start_of_resumed_container(
        self.space,
        &self.builder,
        result.break_info.is_first_for_node,
      );

    if space_left < LayoutUnit::ZERO
      || (space_left == LayoutUnit::ZERO
        && is_break_inside(break_token)
        && !break_token.is_some_and(|token| token.is_at_block_end))
    {
      return false;
    }

    let block_size = block_size_for_fragmentation(result);
    let appeal_inside = calculate_break_appeal_inside(result, None);
    let mut move_past = refuse_break_before;

    if !move_past {
      if block_size <= space_left {
        if is_break_inside(break_token) || appeal_inside < BreakAppeal::Perfect {
          // Breaking inside is better than before only where it is as good.
          if appeal_inside >= appeal_before
            && self
              .builder
              .early_break_appeal()
              .is_none_or(|early_appeal| appeal_inside >= early_appeal)
          {
            move_past = true;
          }
        } else {
          move_past = true;
        }
      } else if appeal_before == BreakAppeal::LastResort
        && self.builder.requires_content_before_breaking
      {
        self.builder.is_block_size_for_fragmentation_clamped = true;
        move_past = true;
      }
    }

    if move_past && let Some((node, style)) = child {
      self.update_early_break_at_block_child(node, style, result, appeal_before);
    }
    move_past
  }

  /// Blink's `UpdateEarlyBreakAtBlockChild`: the breakpoint before or inside `child` the layout
  /// may come back to.
  fn update_early_break_at_block_child(
    &mut self,
    child: NodeId,
    child_style: &LayoutStyle,
    result: &LayoutResult,
    appeal_before: BreakAppeal,
  ) {
    let mut appeal_inside = BreakAppeal::LastResort;

    if let Some(breakpoint) = &result.break_info.early_break {
      appeal_inside = calculate_break_appeal_inside(
        result,
        Some((
          breakpoint.break_appeal,
          self.space,
          child_style.breaks.break_inside,
        )),
      );
      if self
        .builder
        .early_break_appeal()
        .is_none_or(|early_appeal| early_appeal <= breakpoint.break_appeal)
      {
        self.builder.early_break = Some(Rc::new(EarlyBreak::block(
          child,
          appeal_inside,
          Some(Rc::clone(breakpoint)),
        )));
      }
    }
    if appeal_before <= appeal_inside {
      return;
    }
    if self
      .builder
      .early_break_appeal()
      .is_some_and(|early_appeal| early_appeal > appeal_before)
    {
      return;
    }
    self.builder.early_break = Some(Rc::new(EarlyBreak::block(child, appeal_before, None)));
  }

  /// Blink's `AttemptSoftBreak`: breaks before the child, unless a breakpoint seen earlier is
  /// better.
  fn attempt_soft_break(
    &mut self,
    child: Option<NodeId>,
    result: &LayoutResult,
    appeal_before: BreakAppeal,
  ) -> bool {
    if self
      .builder
      .early_break_appeal()
      .is_some_and(|early_appeal| early_appeal > appeal_before)
    {
      return false;
    }

    self.break_before_child(child, result, appeal_before, false);
    true
  }

  /// Blink's `AdjustedMarginAfterFinalChildFragment`: the last child's block-end margin, cut where
  /// the page ends.
  pub(super) fn adjusted_margin_after_final_child_fragment(
    &self,
    block_offset: LayoutUnit,
    block_end_margin: LayoutUnit,
  ) -> LayoutUnit {
    if !self.space.has_block_fragmentation() {
      return block_end_margin;
    }

    let space_left = self.builder.fragmentainer_space_left(true) - block_offset;

    block_end_margin.min(space_left.clamp_negative_to_zero())
  }

  /// Blink's `ShouldIncludeBlockEndBorderPadding`.
  pub(super) fn should_include_block_end_border_padding(&self) -> bool {
    if self
      .builder
      .previous_break_token
      .as_deref()
      .is_some_and(|token| token.is_at_block_end)
    {
      return false;
    }
    if !self.builder.should_break_inside_for_content()
      || self.builder.is_known_to_fit_in_fragmentainer
      || self.builder.should_clone_box_end_decorations
    {
      return true;
    }
    if self.space.is_new_formatting_context {
      return false;
    }
    !self.builder.has_inflow_child_break_inside()
  }

  /// Blink's `ConsumedBlockSize` of the incoming break token.
  pub(super) fn previously_consumed_block_size(&self) -> LayoutUnit {
    self
      .input
      .break_token
      .as_deref()
      .filter(|token| !token.is_break_before)
      .map_or(LayoutUnit::ZERO, |token| token.consumed_block_size)
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
      if self.builder.has_inflow_child_break_inside() || self.first_overflowing_line != 0 {
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

    self.finish_fragmentation()?;
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

  /// Blink's `FinishFragmentation`.
  fn finish_fragmentation(&mut self) -> Result<(), LayoutStatus> {
    let previously_consumed_block_size = self.previously_consumed_block_size();
    let builder = &mut self.builder;
    let mut space_left = builder.fragmentainer_space_left(false);
    let previous_break_token = builder.previous_break_token.clone();
    let is_past_end = previous_break_token
      .as_deref()
      .is_some_and(|token| token.is_at_block_end);
    let desired_block_size = (builder.fragments_total_block_size - previously_consumed_block_size)
      .clamp_negative_to_zero();
    let desired_intrinsic_block_size = builder.intrinsic_block_size;
    let mut final_block_size = desired_block_size;
    let trailing_border_padding = builder.border_padding.block_end;
    let mut subtractable_border_padding = LayoutUnit::ZERO;
    let has_known_space = builder.has_block_fragmentation();

    if !builder.should_prevent_break_before_block_end_decorations
      && (desired_block_size > trailing_border_padding
        || previous_break_token
          .as_deref()
          .is_some_and(|token| token.monolithic_overflow > LayoutUnit::ZERO))
    {
      // A last-resort breakpoint before the trailing border and padding, progress guaranteed.
      subtractable_border_padding = trailing_border_padding;
    }
    if has_known_space {
      // Tall unbreakable content inside lets the fragment take more space than is left.
      space_left = space_left.max(desired_intrinsic_block_size - subtractable_border_padding);
    }

    if is_past_end {
      final_block_size = LayoutUnit::ZERO;
    } else if has_known_space && desired_block_size > space_left {
      let modified_intrinsic_block_size =
        space_left.max(desired_intrinsic_block_size - subtractable_border_padding);

      builder.intrinsic_block_size = modified_intrinsic_block_size;
      final_block_size =
        (desired_block_size - subtractable_border_padding).min(modified_intrinsic_block_size);
      if final_block_size < desired_block_size {
        builder.did_break_self = true;
      }
    }

    builder.consumed_block_size = previously_consumed_block_size + final_block_size;
    builder.fragment_block_size = final_block_size;

    if !has_known_space {
      return Ok(());
    }

    let was_broken_by_child = builder.has_inflow_child_break_inside();

    if final_block_size == LayoutUnit::ZERO
      && let Some(token) = previous_break_token
        .as_deref()
        .filter(|token| token.monolithic_overflow > LayoutUnit::ZERO)
    {
      // Steer clear of the monolithic overflow an earlier page left on the next one too.
      let remaining_overflow =
        token.monolithic_overflow - builder.fragmentainer_capacity(false).unwrap_or_default();

      if remaining_overflow > LayoutUnit::ZERO {
        builder.reserve_space_for_monolithic_overflow(remaining_overflow);
      }
    }

    if builder.should_break_inside_for_content() {
      if is_past_end
        || (desired_block_size <= space_left
          && (!was_broken_by_child || builder.is_known_to_fit_in_fragmentainer))
      {
        builder.is_at_block_end = true;
      }

      if builder.is_at_block_end {
        builder.consumed_block_size =
          previously_consumed_block_size + final_block_size.max(space_left);
      }
      return Ok(());
    }

    if desired_block_size > space_left {
      if previously_consumed_block_size == LayoutUnit::ZERO
        && space_left < self.border_padding.block_start
      {
        // The block-start border and padding do not fit: breaking here is a last resort.
        self.builder.clamp_break_appeal(BreakAppeal::LastResort);
      }
      if desired_block_size <= desired_intrinsic_block_size {
        if self.builder.early_break.is_some() {
          return Err(LayoutStatus::NeedsEarlierBreak);
        }
        self.builder.clamp_break_appeal(BreakAppeal::LastResort);
      }
      return Ok(());
    }

    builder.is_at_block_end = true;
    Ok(())
  }

  /// What the box's layout result tells its container about breaking: the break token where it
  /// resumes, once it breaks.
  pub(super) fn break_info(&mut self) -> BreakInfo {
    if !self.builder.has_block_fragmentation() {
      return BreakInfo {
        is_first_for_node: self.builder.is_first_for_node,
        ..BreakInfo::default()
      };
    }

    let break_token = self
      .builder
      .should_break()
      .then(|| Rc::new(self.builder.create_break_token(self.node)));

    self.builder.break_info(break_token)
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
