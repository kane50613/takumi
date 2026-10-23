//! The block fragmentation state of Blink's
//! [`BoxFragmentBuilder`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/box_fragment_builder.h),
//! for pages alone: no columns, column spanners, or repeated content.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::{mem::take, rc::Rc};

use crate::{
  geometry::{NodeId, Point},
  layout::{
    block_break_token::BlockBreakToken,
    break_appeal::BreakAppeal,
    break_token::BreakToken,
    break_token_algorithm_data::BreakTokenAlgorithmData,
    constraint_space::{BlockFragmentation, ConstraintSpace},
    early_break::EarlyBreak,
    fragmentation_utils::{
      block_size_for_fragmentation, calculate_break_appeal_inside, fragment_index, is_break_inside,
      join_fragmentainer_break_values,
    },
    inline::inline_break_token::InlineBreakToken,
    layout_result::{
      BreakInfo, FragmentedContainingBlock, LayoutResult, LayoutStatus, StaticPosition,
    },
    layout_style::LayoutStyle,
    logical_rect::LogicalBoxStrut,
    out_of_flow::ContainingBlock,
  },
  layout_unit::LayoutUnit,
  style::{BoxDecorationBreak, BreakBetween, BreakInside},
};

/// Blink's `BoxFragmentBuilder` as block fragmentation reads and writes it.
#[derive(Debug, Clone)]
pub(crate) struct BoxFragmentBuilder {
  /// The space's fragmentation fields.
  pub(crate) block_fragmentation: Option<BlockFragmentation>,
  pub(crate) box_decoration_break: BoxDecorationBreak,
  pub(crate) break_inside: BreakInside,
  /// Blink's `BorderScrollbarPadding`, which a resumed or broken fragment clears.
  pub(crate) border_padding: LogicalBoxStrut,
  /// Blink's `PreviousBreakToken`: where this fragment resumes the box.
  pub(crate) previous_break_token: Option<Rc<BlockBreakToken>>,
  child_break_tokens: Vec<BreakToken>,
  /// The break tokens of the out-of-flow boxes this box lays out, which go before the others, as
  /// Blink sorts them first: resuming an in-flow child that breaks again stops the layout.
  out_of_flow_break_tokens: Vec<BreakToken>,
  /// The break token of the last line added, where the lines resume.
  last_inline_break_token: Option<InlineBreakToken>,
  /// Blink's `HasInflowChildBreakInside`.
  pub(crate) has_inflow_child_break_inside: bool,
  pub(crate) did_break_self: bool,
  pub(crate) has_forced_break: bool,
  pub(crate) break_appeal: BreakAppeal,
  pub(crate) early_break: Option<Rc<EarlyBreak>>,
  pub(crate) initial_break_before: Option<BreakBetween>,
  pub(crate) previous_break_after: BreakBetween,
  pub(crate) consumed_block_size: LayoutUnit,
  pub(crate) sequence_number: u32,
  pub(crate) is_at_block_end: bool,
  pub(crate) has_seen_all_children: bool,
  /// Whether a list item breaks with its outside marker yet to place beside a baseline.
  pub(crate) has_unpositioned_list_marker: bool,
  /// Blink's `has_subsequent_children_`: a grid's items start on a later page, though it inserted
  /// no break before them.
  pub(crate) has_subsequent_children: bool,
  pub(crate) monolithic_overflow: LayoutUnit,
  pub(crate) is_known_to_fit_in_fragmentainer: bool,
  pub(crate) should_clone_box_end_decorations: bool,
  pub(crate) should_prevent_break_before_block_end_decorations: bool,
  /// The fragments whose cloned block-start and block-end decorations the box's size counts,
  /// this one included: one unless they clone after a break inside.
  pub(crate) cloned_decorations_fragment_count: i32,
  pub(crate) requires_content_before_breaking: bool,
  /// Blink's `FragmentsTotalBlockSize`: the box's block size as if it did not fragment.
  pub(crate) fragments_total_block_size: LayoutUnit,
  /// Blink's `FragmentBlockSize`: this fragment's block size.
  pub(crate) fragment_block_size: LayoutUnit,
  pub(crate) intrinsic_block_size: LayoutUnit,
  pub(crate) block_size_for_fragmentation: LayoutUnit,
  pub(crate) is_block_size_for_fragmentation_clamped: bool,
  /// Blink's `LineCount`: the non-empty lines added.
  pub(crate) line_count: usize,
  pub(crate) is_first_for_node: bool,
  /// Blink's `SidesToInclude` on the block start and end.
  pub(crate) includes_block_start: bool,
  pub(crate) includes_block_end: bool,
  /// Blink's `SetBreakTokenData`: what the algorithm resumes from in the next fragment.
  pub(crate) break_token_data: Option<BreakTokenAlgorithmData>,
  /// Blink's `HasFragmentainerBreak` of a new formatting context's exclusion space: a float in it
  /// broke before or inside.
  pub(crate) has_float_break: bool,
}

impl BoxFragmentBuilder {
  /// A builder for `style`'s box laid out in `space`, resuming at `previous_break_token`.
  pub(crate) fn new(
    space: &ConstraintSpace,
    style: &LayoutStyle,
    border_padding: LogicalBoxStrut,
    previous_break_token: Option<Rc<BlockBreakToken>>,
  ) -> Self {
    Self {
      block_fragmentation: space.block_fragmentation,
      box_decoration_break: style.breaks.box_decoration_break,
      break_inside: style.breaks.break_inside,
      border_padding,
      previous_break_token,
      child_break_tokens: Vec::new(),
      out_of_flow_break_tokens: Vec::new(),
      last_inline_break_token: None,
      has_inflow_child_break_inside: false,
      did_break_self: false,
      has_forced_break: false,
      break_appeal: BreakAppeal::Perfect,
      early_break: None,
      initial_break_before: None,
      previous_break_after: BreakBetween::Auto,
      consumed_block_size: LayoutUnit::ZERO,
      sequence_number: 0,
      is_at_block_end: false,
      has_seen_all_children: false,
      has_unpositioned_list_marker: false,
      has_subsequent_children: false,
      monolithic_overflow: LayoutUnit::ZERO,
      is_known_to_fit_in_fragmentainer: false,
      should_clone_box_end_decorations: false,
      should_prevent_break_before_block_end_decorations: false,
      cloned_decorations_fragment_count: 1,
      requires_content_before_breaking: space.requires_content_before_breaking(),
      fragments_total_block_size: LayoutUnit::ZERO,
      fragment_block_size: LayoutUnit::ZERO,
      intrinsic_block_size: LayoutUnit::ZERO,
      block_size_for_fragmentation: LayoutUnit::ZERO,
      is_block_size_for_fragmentation_clamped: false,
      line_count: 0,
      is_first_for_node: true,
      includes_block_start: true,
      includes_block_end: true,
      break_token_data: None,
      has_float_break: false,
    }
  }

  /// Blink's `HasBlockFragmentation` of the builder's space.
  pub(crate) fn has_block_fragmentation(&self) -> bool {
    self.block_fragmentation.is_some()
  }

  /// Blink's `InvolvedInBlockFragmentation`.
  pub(crate) fn involved_in_block_fragmentation(&self) -> bool {
    self.has_block_fragmentation() || self.previous_break_token.is_some()
  }

  /// Blink's `ClampBreakAppeal`.
  pub(crate) fn clamp_break_appeal(&mut self, appeal: BreakAppeal) {
    self.break_appeal = self.break_appeal.min(appeal);
  }

  /// Blink's `HasEarlyBreak`, with its appeal.
  pub(crate) fn early_break_appeal(&self) -> Option<BreakAppeal> {
    self
      .early_break
      .as_ref()
      .map(|early_break| early_break.break_appeal)
  }

  /// Blink's `JoinedBreakBetweenValue`.
  pub(crate) fn joined_break_between_value(&self, break_before: BreakBetween) -> BreakBetween {
    join_fragmentainer_break_values(self.previous_break_after, break_before)
  }

  /// Blink's `AddBreakBeforeChild`, for an inflow block child or, as `None`, the inline content.
  pub(crate) fn add_break_before_child(
    &mut self,
    child: Option<NodeId>,
    appeal: BreakAppeal,
    is_forced_break: bool,
  ) {
    self.has_inflow_child_break_inside = true;
    self.add_break_before(child, appeal, is_forced_break);
  }

  /// Blink's `AddBreakBeforeChild` for a float or out-of-flow box a forced break pushes, which is a
  /// parallel flow.
  pub(crate) fn add_break_before_float_or_out_of_flow(&mut self, child: NodeId) {
    self.add_break_before(Some(child), BreakAppeal::Perfect, true);
  }

  fn add_break_before(
    &mut self,
    child: Option<NodeId>,
    appeal: BreakAppeal,
    is_forced_break: bool,
  ) {
    if is_forced_break {
      self.has_forced_break = true;
    } else {
      self.clamp_break_appeal(appeal);
    }

    let Some(child) = child else {
      // The lines resume where the last one added broke, or where this fragment resumed them.
      if self.last_inline_break_token.is_none() {
        self.last_inline_break_token = self
          .previous_break_token
          .as_ref()
          .and_then(|token| match token.child_break_tokens.last() {
            Some(BreakToken::Inline(inline_break_token)) => Some(inline_break_token.clone()),
            _ => None,
          })
          .or(Some(InlineBreakToken::default()));
      }
      return;
    };

    self
      .child_break_tokens
      .push(BreakToken::Block(Rc::new(BlockBreakToken::break_before(
        child,
        is_forced_break,
      ))));
  }

  /// The fragment `fragment_index` of `node`, the box this builder builds with its padding box
  /// `padding_box`, as the out-of-flow boxes it leaves to the fragmentation root see it.
  pub(crate) fn fragmented_containing_block(
    &self,
    node: NodeId,
    fragment_index: usize,
    padding_box: ContainingBlock,
  ) -> FragmentedContainingBlock {
    FragmentedContainingBlock {
      node,
      fragment_index,
      offset: Point::default(),
      consumed_block_size: self.previously_consumed_block_size(),
      padding_box,
      is_inside_clipped_container: false,
    }
  }

  /// Blink's `AddBreakToken`: a break before `node`, which the next fragment resumes at, with no
  /// break of its own here.
  pub(crate) fn add_break_token_before(&mut self, node: NodeId) {
    self
      .child_break_tokens
      .push(BreakToken::Block(Rc::new(BlockBreakToken::break_before(
        node, false,
      ))));
  }

  /// The block fragmentation part of Blink's `AddResult` for a child of `child_style` laid out as
  /// `result` at `block_offset`, in a box laid out in `space`.
  pub(crate) fn add_result(
    &mut self,
    space: &ConstraintSpace,
    child_style: &LayoutStyle,
    result: &LayoutResult,
    block_offset: LayoutUnit,
  ) {
    self.add_child_break_token(result);
    self.propagate_result(space, child_style, result, block_offset);
  }

  /// The break data Blink's `AddResult` propagates from a child of `child_style` laid out as
  /// `result` at `block_offset`.
  pub(crate) fn propagate_result(
    &mut self,
    space: &ConstraintSpace,
    child_style: &LayoutStyle,
    result: &LayoutResult,
    block_offset: LayoutUnit,
  ) {
    let flow = if child_style.is_out_of_flow() {
      ChildFlow::OutOfFlow
    } else if child_style.is_floating() {
      ChildFlow::Float
    } else {
      ChildFlow::InFlow
    };

    self.propagate_break_info(result, block_offset, flow);
    if space.should_propagate_child_break_values() {
      self.propagate_child_break_values(result, child_style);
    }
  }

  /// Blink's `AddBreakToken` for a line's block-in-inline that resumes in a parallel flow.
  pub(crate) fn add_parallel_flow_break_token(&mut self, token: InlineBreakToken) {
    self.child_break_tokens.push(BreakToken::Inline(token));
  }

  /// Blink's `FragmentBuilder::PropagateFromFragment` collecting a block child's break token.
  fn add_child_break_token(&mut self, result: &LayoutResult) {
    if let Some(token) = &result.break_info.break_token {
      self
        .child_break_tokens
        .push(BreakToken::Block(Rc::clone(token)));
    }
  }

  /// Blink's `FragmentBuilder::PropagateFromFragment` collecting a line's break token: the lines
  /// resume after `break_token`, none after the last line.
  pub(crate) fn add_line(&mut self, break_token: Option<InlineBreakToken>, is_empty: bool) {
    self.last_inline_break_token = break_token;

    if !is_empty {
      self.line_count += 1;
    }
  }

  /// Blink's `PropagateBreakInfo` for a child `result` at `block_offset`.
  fn propagate_break_info(
    &mut self,
    result: &LayoutResult,
    block_offset: LayoutUnit,
    flow: ChildFlow,
  ) {
    let block_end_in_container = block_offset + block_size_for_fragmentation(result);

    self.block_size_for_fragmentation = self
      .block_size_for_fragmentation
      .max(block_end_in_container);

    if self.space_requires_content_before_breaking()
      && result.break_info.is_block_size_for_fragmentation_clamped
    {
      self.is_block_size_for_fragmentation_clamped = true;
    }

    let token = result.break_info.break_token.as_deref();
    let child_is_in_same_flow =
      token.is_none_or(|token| !token.is_at_block_end) && flow == ChildFlow::InFlow;

    if child_is_in_same_flow {
      let fragmentainer_overflow =
        block_offset + result.size.block_size - self.fragmentainer_space_left(false);

      if fragmentainer_overflow > LayoutUnit::ZERO {
        self.reserve_space_for_monolithic_overflow(fragmentainer_overflow);
      }
    }

    if is_break_inside(token) {
      if child_is_in_same_flow {
        self.has_inflow_child_break_inside = true;
      }
      if flow != ChildFlow::OutOfFlow {
        self.clamp_break_appeal(calculate_break_appeal_inside(result, None));
      }
    }
    if result.break_info.has_forced_break {
      self.has_forced_break = true;
    }
    if flow == ChildFlow::InFlow {
      self.requires_content_before_breaking = false;
    }
  }

  /// Blink's `PropagateChildBreakValues` for a block child of `style` laid out as `result`.
  pub(crate) fn propagate_child_break_values(
    &mut self,
    result: &LayoutResult,
    style: &LayoutStyle,
  ) {
    if result.status != LayoutStatus::Success || style.is_floating() || style.is_out_of_flow() {
      return;
    }

    let break_before = join_fragmentainer_break_values(
      result.break_info.initial_break_before.unwrap_or_default(),
      style.breaks.break_before,
    );

    self.initial_break_before.get_or_insert(break_before);
    self.previous_break_after = join_fragmentainer_break_values(
      result.break_info.final_break_after.unwrap_or_default(),
      style.breaks.break_after,
    );
  }

  /// Blink's `ReserveSpaceForMonolithicOverflow`.
  pub(crate) fn reserve_space_for_monolithic_overflow(&mut self, overflow: LayoutUnit) {
    self.monolithic_overflow = self.monolithic_overflow.max(overflow);
  }

  /// Blink's `HasInsertedChildBreak`.
  pub(crate) fn has_inserted_child_break(&self) -> bool {
    !self.child_break_tokens.is_empty() || !self.out_of_flow_break_tokens.is_empty()
  }

  /// Blink's check in `LayoutCandidates` that an out-of-flow `node` at `offset` starts past the
  /// end of this fragment: it breaks before the box, which starts as far into the next, in a
  /// parallel flow.
  pub(crate) fn breaks_before_out_of_flow(
    &mut self,
    node: NodeId,
    offset: Point<LayoutUnit>,
  ) -> bool {
    if !self.has_block_fragmentation() {
      return false;
    }

    let block_overflow = offset.y - self.fragmentainer_space_left(true);

    if block_overflow <= LayoutUnit::ZERO {
      return false;
    }
    self
      .out_of_flow_break_tokens
      .push(BreakToken::Block(Rc::new(
        BlockBreakToken::break_before_out_of_flow(node, Point::new(offset.x, block_overflow)),
      )));
    true
  }

  /// Holds the out-of-flow box `node` back to this box's last fragment with a break before it,
  /// which keeps its `static_position` in the coordinates of the fragments stitched together.
  pub(crate) fn holds_out_of_flow(&mut self, node: NodeId, mut static_position: StaticPosition) {
    static_position.offset.y += self.previously_consumed_block_size();
    self
      .out_of_flow_break_tokens
      .push(BreakToken::Block(Rc::new(
        BlockBreakToken::break_before_held_out_of_flow(node, static_position),
      )));
  }

  /// The block fragmentation part of Blink's `AddResult` for an out-of-flow box laid out as
  /// `result` at `offset`, and the clamp `LayoutCandidates` makes after it.
  pub(crate) fn add_out_of_flow_result(
    &mut self,
    result: &LayoutResult,
    offset: Point<LayoutUnit>,
  ) {
    if self.has_block_fragmentation() {
      // The next fragment cannot work out the inline offset again from a static position.
      if let Some(token) = &result.break_info.break_token {
        self
          .out_of_flow_break_tokens
          .push(BreakToken::Block(Rc::new(BlockBreakToken {
            oof_start_offset: Point::new(offset.x, LayoutUnit::ZERO),
            ..(**token).clone()
          })));
      }
      self.propagate_break_info(result, offset.y, ChildFlow::OutOfFlow);
    }
    // The box overflows the page, but stays on it with a container whose block size fits.
    if self.requires_content_before_breaking
      && block_size_for_fragmentation(result) > self.fragmentainer_space_left(true)
    {
      self.is_block_size_for_fragmentation_clamped = true;
    }
  }

  /// Blink's `ShouldBreakInsideForContent`.
  pub(crate) fn should_break_inside_for_content(&self) -> bool {
    self.has_inserted_child_break()
      || self.last_inline_break_token.is_some()
      || self.monolithic_overflow > LayoutUnit::ZERO
      || self.has_subsequent_children
  }

  /// Blink's `ShouldBreak`.
  pub(crate) fn should_break(&self) -> bool {
    self.did_break_self || self.should_break_inside_for_content()
  }

  /// Blink's `RequiresContentBeforeBreaking` of the space, which the builder's starts out as.
  fn space_requires_content_before_breaking(&self) -> bool {
    self
      .block_fragmentation
      .is_some_and(|fragmentation| fragmentation.requires_content_before_breaking)
  }

  /// Blink's `FragmentainerCapacity`.
  pub(crate) fn fragmentainer_capacity(&self, is_for_children: bool) -> Option<LayoutUnit> {
    let mut size = self.block_fragmentation?.fragmentainer_block_size;

    if self.box_decoration_break == BoxDecorationBreak::Clone && is_for_children {
      size -= self.border_padding.block_start;
      if self.should_clone_box_end_decorations {
        size -= self.border_padding.block_end;
      }
    }

    Some(self.clamped_to_valid_fragmentainer_capacity(size, is_for_children))
  }

  /// Blink's `ClampedToValidFragmentainerCapacity`: css-break-3 §4.4 makes a page take at least
  /// one pixel of content.
  fn clamped_to_valid_fragmentainer_capacity(
    &self,
    length: LayoutUnit,
    is_for_children: bool,
  ) -> LayoutUnit {
    let mut minimum = LayoutUnit::from_int(1);

    if self.should_clone_box_end_decorations && !is_for_children {
      minimum += self.border_padding.block_start + self.border_padding.block_end;
    }
    length.max(minimum)
  }

  /// Blink's `FragmentainerOffset`.
  pub(crate) fn fragmentainer_offset(&self, is_for_children: bool) -> LayoutUnit {
    let Some(fragmentation) = self.block_fragmentation else {
      return LayoutUnit::ZERO;
    };
    let mut offset = fragmentation.fragmentainer_offset;

    // A child at the content edge of a cloned box starts the page, as far as breaking goes.
    if self.box_decoration_break == BoxDecorationBreak::Clone
      && is_for_children
      && self
        .previous_break_token
        .as_ref()
        .is_none_or(|token| !token.is_at_block_end)
    {
      offset -= self.border_padding.block_start;
    }
    offset
  }

  /// Blink's `FragmentainerSpaceLeft`, the page's block size outside a fragmentation context.
  pub(crate) fn fragmentainer_space_left(&self, is_for_children: bool) -> LayoutUnit {
    self
      .fragmentainer_capacity(is_for_children)
      .map_or(LayoutUnit::MAX, |capacity| {
        (capacity - self.fragmentainer_offset(is_for_children)).clamp_negative_to_zero()
      })
  }

  /// Blink's `BlockBreakToken::Create`: where `node` resumes, once [`Self::should_break`].
  pub(crate) fn create_break_token(&mut self, node: NodeId) -> BlockBreakToken {
    if let Some(inline_break_token) = self.last_inline_break_token.take() {
      self
        .child_break_tokens
        .push(BreakToken::Inline(inline_break_token));
    }

    BlockBreakToken {
      node,
      consumed_block_size: self.consumed_block_size,
      sequence_number: self.sequence_number,
      monolithic_overflow: self.monolithic_overflow,
      is_break_before: false,
      is_forced_break: false,
      is_repeated: false,
      is_at_block_end: self.is_at_block_end,
      has_seen_all_children: self.has_seen_all_children,
      has_unpositioned_list_marker: self.has_unpositioned_list_marker,
      child_break_tokens: take(&mut self.out_of_flow_break_tokens)
        .into_iter()
        .chain(take(&mut self.child_break_tokens))
        .collect(),
      algorithm_data: self.break_token_data.take(),
      oof_start_offset: Point::default(),
      held_static_position: None,
    }
  }

  /// Blink's `BorderPadding` from the box's `border_padding`: the block size resolves with the
  /// block-start and block-end decorations of every fragment so far when they clone.
  pub(crate) fn border_padding_for_size(
    &self,
    mut border_padding: LogicalBoxStrut,
  ) -> LogicalBoxStrut {
    border_padding.block_start =
      border_padding.block_start * self.cloned_decorations_fragment_count;
    border_padding.block_end = border_padding.block_end * self.cloned_decorations_fragment_count;
    border_padding
  }

  /// Blink's `LastChildBreakToken`: the break token of the last child that broke.
  pub(crate) fn last_child_break_token(&self) -> Option<&BlockBreakToken> {
    match self.child_break_tokens.last()? {
      BreakToken::Block(token) => Some(token),
      BreakToken::Inline(_) => None,
    }
  }

  /// Blink's `ClonedBlockStartDecoration`: the block-start border and padding a resumed fragment
  /// repeats when its decorations clone.
  pub(crate) fn cloned_block_start_decoration(&self) -> LayoutUnit {
    let is_cloned = self.box_decoration_break == BoxDecorationBreak::Clone
      && self
        .previous_break_token
        .as_deref()
        .is_some_and(|token| is_break_inside(Some(token)) && !token.is_at_block_end);

    if is_cloned {
      self.border_padding.block_start
    } else {
      LayoutUnit::ZERO
    }
  }

  /// Blink's `ShouldCloneBlockStartBorderPadding`.
  pub(crate) fn should_clone_block_start_border_padding(&self) -> bool {
    if self.box_decoration_break != BoxDecorationBreak::Clone {
      return false;
    }

    let Some(previous_break_token) = self.previous_break_token.as_deref() else {
      return true;
    };

    if previous_break_token.monolithic_overflow > LayoutUnit::ZERO
      && self.fragmentainer_space_left(false)
        < self.border_padding.block_start + self.border_padding.block_end
    {
      return false;
    }
    !previous_break_token.is_at_block_end
  }

  /// Blink's `ConsumedBlockSize` of the incoming break token.
  pub(crate) fn previously_consumed_block_size(&self) -> LayoutUnit {
    self
      .previous_break_token
      .as_deref()
      .filter(|token| !token.is_break_before)
      .map_or(LayoutUnit::ZERO, |token| token.consumed_block_size)
  }

  /// Blink's `ShouldIncludeBlockEndBorderPadding`, for a box that establishes a new formatting
  /// context when `is_new_formatting_context`.
  pub(crate) fn should_include_block_end_border_padding(
    &self,
    is_new_formatting_context: bool,
  ) -> bool {
    if self
      .previous_break_token
      .as_deref()
      .is_some_and(|token| token.is_at_block_end)
    {
      return false;
    }
    if !self.should_break_inside_for_content()
      || self.is_known_to_fit_in_fragmentainer
      || self.should_clone_box_end_decorations
    {
      return true;
    }
    if is_new_formatting_context {
      return false;
    }
    !self.has_inflow_child_break_inside
  }

  /// Blink's `SidesToInclude`: a fragment's border and padding on the side a break cut, which
  /// layout left out, are none in `result` either, so they paint nowhere.
  pub(crate) fn exclude_broken_sides(&self, result: &mut LayoutResult) {
    if !self.includes_block_start {
      result.border.top = 0.0;
      result.padding.top = 0.0;
    }
    if !self.includes_block_end {
      result.border.bottom = 0.0;
      result.padding.bottom = 0.0;
    }
  }

  /// What `node`'s layout result tells its container about breaking: the break token where it
  /// resumes, once it breaks.
  pub(crate) fn finish_break_info(&mut self, node: NodeId) -> BreakInfo {
    if !self.has_block_fragmentation() {
      return BreakInfo {
        is_first_for_node: self.is_first_for_node,
        fragment_index: fragment_index(self.previous_break_token.as_deref()),
        ..BreakInfo::default()
      };
    }

    let break_token = self
      .should_break()
      .then(|| Rc::new(self.create_break_token(node)));

    self.break_info(break_token)
  }

  /// What the box's layout result tells its container about breaking, with its `break_token`.
  fn break_info(&self, break_token: Option<Rc<BlockBreakToken>>) -> BreakInfo {
    BreakInfo {
      break_token,
      break_appeal: self.break_appeal,
      has_forced_break: self.has_forced_break,
      initial_break_before: self.initial_break_before,
      final_break_after: Some(self.previous_break_after),
      early_break: self.early_break.clone(),
      block_size_for_fragmentation: Some(self.block_size_for_fragmentation),
      is_block_size_for_fragmentation_clamped: self.is_block_size_for_fragmentation_clamped,
      is_first_for_node: self.is_first_for_node,
      fragment_index: fragment_index(self.previous_break_token.as_deref()),
      is_repeated: false,
      avoids_break_inside: self.has_block_fragmentation() && self.break_inside.avoids_page_break(),
    }
  }
}

/// Which flow a child is in, as Blink's `PropagateBreakInfo` tells them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChildFlow {
  InFlow,
  Float,
  OutOfFlow,
}
