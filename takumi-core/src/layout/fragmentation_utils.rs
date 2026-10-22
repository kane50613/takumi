//! Blink's [`fragmentation_utils`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/fragmentation_utils.cc),
//! for pages alone: no columns, column balancing, page names, or space shortage.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::rc::Rc;

use crate::{
  geometry::NodeId,
  layout::{
    block_break_token::BlockBreakToken,
    box_fragment_builder::BoxFragmentBuilder,
    break_appeal::BreakAppeal,
    break_token::BreakToken,
    constraint_space::{BlockFragmentation, ConstraintSpace},
    early_break::{EarlyBreak, EarlyBreakTarget},
    fragmentation_layout_input::FragmentationLayoutInput,
    layout_result::{LayoutResult, LayoutStatus},
    layout_style::LayoutStyle,
    length_utils::LengthResolver,
    logical_rect::LogicalBoxStrut,
  },
  layout_unit::LayoutUnit,
  style::{BoxDecorationBreak, BreakBetween, BreakInside},
};

/// Blink's `FragmentainerBreakPrecedence`.
fn fragmentainer_break_precedence(value: BreakBetween) -> u8 {
  match value {
    BreakBetween::Auto => 0,
    BreakBetween::AvoidColumn => 1,
    BreakBetween::AvoidPage => 2,
    BreakBetween::Avoid => 3,
    BreakBetween::Column => 4,
    BreakBetween::Page => 5,
    BreakBetween::Left | BreakBetween::Right | BreakBetween::Recto | BreakBetween::Verso => 6,
  }
}

/// Blink's `JoinFragmentainerBreakValues`: the value that takes precedence, the second on a tie.
pub(crate) fn join_fragmentainer_break_values(
  first_value: BreakBetween,
  second_value: BreakBetween,
) -> BreakBetween {
  if fragmentainer_break_precedence(second_value) >= fragmentainer_break_precedence(first_value) {
    second_value
  } else {
    first_value
  }
}

/// Blink's `IsForcedBreakValue`, for pages.
pub(crate) fn is_forced_break_value(space: &ConstraintSpace, value: BreakBetween) -> bool {
  space.has_block_fragmentation() && value.forces_page_break()
}

/// Blink's `IsAvoidBreakValue` for `break-before` and `break-after`, for pages.
pub(crate) fn is_avoid_break_value(space: &ConstraintSpace, value: BreakBetween) -> bool {
  space.has_block_fragmentation() && value.avoids_page_break()
}

/// Blink's `IsAvoidBreakValue` for `break-inside`, for pages.
pub(crate) fn is_avoid_break_inside_value(space: &ConstraintSpace, value: BreakInside) -> bool {
  space.has_block_fragmentation() && value.avoids_page_break()
}

/// Blink's `IsBreakInside`: the token resumes a box that made a fragment, rather than repeating
/// it.
pub(crate) fn is_break_inside(token: Option<&BlockBreakToken>) -> bool {
  token.is_some_and(|token| !token.is_break_before && !token.is_repeated)
}

/// Blink's `FragmentIndex`: which of the box's fragments a layout resuming at `token` makes.
pub(crate) fn fragment_index(token: Option<&BlockBreakToken>) -> usize {
  match token {
    Some(token) if !token.is_break_before => token.sequence_number as usize + 1,
    _ => 0,
  }
}

/// Blink's `CalculateBreakBetweenValue` for a block child of `child_style` laid out as `result`,
/// or for a line as `None`.
pub(crate) fn calculate_break_between_value(
  child_style: Option<&LayoutStyle>,
  result: &LayoutResult,
  builder: &BoxFragmentBuilder,
) -> BreakBetween {
  let initial_break_before = result.break_info.initial_break_before.unwrap_or_default();
  let Some(child_style) = child_style else {
    return builder.joined_break_between_value(initial_break_before);
  };

  if result.status == LayoutStatus::Success && !result.break_info.is_first_for_node {
    return BreakBetween::Auto;
  }

  builder.joined_break_between_value(join_fragmentainer_break_values(
    child_style.breaks.break_before,
    initial_break_before,
  ))
}

/// Blink's `IsBreakableAtStartOfResumedContainer`.
pub(crate) fn is_breakable_at_start_of_resumed_container(
  space: &ConstraintSpace,
  builder: &BoxFragmentBuilder,
  is_first_for_node: bool,
) -> bool {
  space.min_break_appeal() != BreakAppeal::LastResort
    && is_break_inside(builder.previous_break_token.as_deref())
    && is_first_for_node
}

/// Blink's `CalculateBreakAppealBefore` for a block child of `child_style` laid out as `result`,
/// or for a line as `None`.
pub(crate) fn calculate_break_appeal_before(
  space: &ConstraintSpace,
  child_style: Option<&LayoutStyle>,
  result: &LayoutResult,
  builder: &BoxFragmentBuilder,
  has_container_separation: bool,
) -> BreakAppeal {
  let breakable_at_start_of_container = result.status == LayoutStatus::Success
    && is_breakable_at_start_of_resumed_container(
      space,
      builder,
      result.break_info.is_first_for_node,
    );
  let break_between = calculate_break_between_value(child_style, result, builder);
  let mut break_appeal = BreakAppeal::Perfect;

  if !has_container_separation && result.status == LayoutStatus::Success {
    if !breakable_at_start_of_container {
      return BreakAppeal::LastResort;
    }
    break_appeal = space.min_break_appeal();
  }
  if is_avoid_break_value(space, break_between) {
    break_appeal = break_appeal.min(BreakAppeal::ViolatingBreakAvoid);
  }
  break_appeal
}

/// Blink's `CalculateBreakAppealInside`, for a box whose `break-inside` is read off `result`.
pub(crate) fn calculate_break_appeal_inside(
  result: &LayoutResult,
  hypothetical: Option<(BreakAppeal, &ConstraintSpace, BreakInside)>,
) -> BreakAppeal {
  if result.break_info.has_forced_break {
    return BreakAppeal::Perfect;
  }

  let (appeal, avoids_break_inside) = match hypothetical {
    Some((appeal, space, break_inside)) => {
      (appeal, is_avoid_break_inside_value(space, break_inside))
    }
    None => (
      result.break_info.break_appeal,
      is_break_inside(result.break_info.break_token.as_deref())
        && result.break_info.avoids_break_inside,
    ),
  };

  if avoids_break_inside && appeal > BreakAppeal::ViolatingBreakAvoid {
    BreakAppeal::ViolatingBreakAvoid
  } else {
    appeal
  }
}

/// Blink's `AdjustMarginsForFragmentation`: a box's block-start margin before its first fragment,
/// kept after a forced break alone, and its block-end margin once.
pub(crate) fn adjust_margins_for_fragmentation(
  break_token: Option<&BlockBreakToken>,
  is_floating: bool,
  margins: &mut LogicalBoxStrut,
) {
  let Some(break_token) = break_token else {
    return;
  };

  if !break_token.is_break_before || (!break_token.is_forced_break && !is_floating) {
    margins.block_start = LayoutUnit::ZERO;
  }
  if break_token.is_at_block_end {
    margins.block_end = LayoutUnit::ZERO;
  }
}

/// Blink's `BlockSizeForFragmentation`: how far a child reaches for breaking, its border box
/// where its layout gave none.
pub(crate) fn block_size_for_fragmentation(result: &LayoutResult) -> LayoutUnit {
  result
    .break_info
    .block_size_for_fragmentation
    .unwrap_or(result.size.block_size)
}

/// Blink's `EnterEarlyBreakInChild`: the breakpoint inside `child` that `early_break` names.
pub(crate) fn enter_early_break_in_child(
  child: NodeId,
  early_break: &EarlyBreak,
) -> Option<Rc<EarlyBreak>> {
  match &early_break.target {
    EarlyBreakTarget::Block {
      node,
      break_inside: Some(break_inside),
    } if *node == child => Some(Rc::clone(break_inside)),
    _ => None,
  }
}

/// Blink's `IsEarlyBreakTarget` for a block child, or for the next line as `None`.
pub(crate) fn is_early_break_target(
  early_break: &EarlyBreak,
  builder: &BoxFragmentBuilder,
  child: Option<NodeId>,
) -> bool {
  match (&early_break.target, child) {
    (EarlyBreakTarget::Line(line_number), None) => *line_number == builder.line_count,
    (EarlyBreakTarget::Block { node, .. }, Some(child)) => {
      early_break.is_break_before() && *node == child
    }
    _ => false,
  }
}

/// Blink's `SetupFragmentBuilderForFragmentation` for a box of `style` laid out `inline_size` wide
/// in `space`: whether the fragment leaves its block-start border and padding out.
pub(crate) fn setup_fragment_builder_for_fragmentation(
  space: &ConstraintSpace,
  style: &LayoutStyle,
  inline_size: LayoutUnit,
  builder: &mut BoxFragmentBuilder,
) -> bool {
  let previous_break_token = builder.previous_break_token.clone();

  if !space.has_block_fragmentation() && previous_break_token.is_none() {
    return false;
  }

  if let Some(token) = previous_break_token
    .as_deref()
    .filter(|token| !token.is_break_before)
  {
    builder.sequence_number = token.sequence_number + 1;
    builder.is_first_for_node = false;
  }

  let space_left = builder.fragmentainer_space_left(false);
  let clone_box_start_decorations = builder.should_clone_block_start_border_padding();
  let mut clone_box_end_decorations = clone_box_start_decorations;

  if space.has_block_fragmentation() {
    let mut requires_content_before_breaking = space.requires_content_before_breaking();
    let resolver = LengthResolver::new(space, style);
    let max_block_size = resolver
      .block_size_for_fragment(Some(LayoutUnit::MAX), Some(inline_size), None)
      .unwrap_or(LayoutUnit::MAX);

    if max_block_size != LayoutUnit::MAX
      && max_block_size - builder.previously_consumed_block_size() <= space_left
    {
      builder.is_known_to_fit_in_fragmentainer = true;
      clone_box_end_decorations = false;
      // Blink's `MustStayInCurrentFragmentainer`.
      if builder.is_first_for_node {
        requires_content_before_breaking = true;
      }
    }
    if clone_box_end_decorations {
      builder.should_clone_box_end_decorations = true;
      builder.should_prevent_break_before_block_end_decorations = true;
    }
    builder.requires_content_before_breaking = requires_content_before_breaking;
  }

  let leaves_block_start_out =
    is_break_inside(previous_break_token.as_deref()) && !clone_box_start_decorations;

  if leaves_block_start_out {
    builder.border_padding.block_start = LayoutUnit::ZERO;
  }
  leaves_block_start_out
}

/// Blink's `BlockChildIterator`: the `children` a fragment resuming at `break_token` resumes or
/// starts, with the tokens they resume at.
pub(crate) fn block_child_entries(
  children: Vec<NodeId>,
  break_token: Option<&BlockBreakToken>,
) -> Vec<(NodeId, Option<Rc<BlockBreakToken>>)> {
  let Some(container_break_token) = break_token else {
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

/// Blink's `GetFragmentationLayoutInputForChild`: `child` resumed at `token`, with the breakpoint
/// inside it that `early_break` names.
pub(crate) fn fragmentation_layout_input_for_child(
  child: NodeId,
  token: Option<Rc<BlockBreakToken>>,
  early_break: Option<&EarlyBreak>,
) -> FragmentationLayoutInput {
  FragmentationLayoutInput {
    break_token: token,
    early_break: early_break.and_then(|early_break| enter_early_break_in_child(child, early_break)),
  }
}

/// Blink's `SetupSpaceBuilderForFragmentation` for a child of the box `builder` builds in
/// `parent_space`, `fragmentainer_offset_delta` past its children's fragmentainer offset.
pub(crate) fn setup_space_builder_for_fragmentation(
  parent_space: &ConstraintSpace,
  builder: &BoxFragmentBuilder,
  child_is_monolithic: bool,
  fragmentainer_offset_delta: LayoutUnit,
  child_space: &mut ConstraintSpace,
) {
  let Some(parent) = parent_space.block_fragmentation else {
    return;
  };

  if child_is_monolithic {
    return;
  }

  child_space.block_fragmentation = Some(BlockFragmentation {
    fragmentainer_block_size: builder
      .fragmentainer_capacity(true)
      .unwrap_or(parent.fragmentainer_block_size),
    fragmentainer_offset: builder.fragmentainer_offset(true) + fragmentainer_offset_delta,
    requires_content_before_breaking: builder.requires_content_before_breaking,
    should_propagate_child_break_values: true,
    min_break_appeal: parent.min_break_appeal,
    min_block_size_should_encompass_intrinsic_size: false,
  });
}

/// Blink's `BreakBeforeChildIfNeeded` for a block-level `child` laid out as `result` at
/// `fragmentainer_block_offset`: whether the layout broke before it.
pub(crate) fn break_before_child_if_needed(
  space: &ConstraintSpace,
  builder: &mut BoxFragmentBuilder,
  child: (NodeId, &LayoutStyle),
  result: &LayoutResult,
  fragmentainer_block_offset: LayoutUnit,
  has_container_separation: bool,
) -> Result<bool, LayoutStatus> {
  if has_container_separation {
    let break_between = calculate_break_between_value(Some(child.1), result, builder);

    if is_forced_break_value(space, break_between) {
      break_before_child(
        space,
        builder,
        Some(child),
        result,
        BreakAppeal::Perfect,
        true,
      );
      return Ok(true);
    }
  }

  let appeal_before = calculate_break_appeal_before(
    space,
    Some(child.1),
    result,
    builder,
    has_container_separation,
  );

  if move_past_breakpoint(
    space,
    builder,
    Some(child),
    result,
    fragmentainer_block_offset,
    appeal_before,
  ) {
    return Ok(false);
  }
  if !attempt_soft_break(space, builder, Some(child), result, appeal_before) {
    return Err(LayoutStatus::NeedsEarlierBreak);
  }
  Ok(true)
}

/// Blink's `BreakBeforeChild`, for a block child or, as `None`, the next line.
pub(crate) fn break_before_child(
  space: &ConstraintSpace,
  builder: &mut BoxFragmentBuilder,
  child: Option<(NodeId, &LayoutStyle)>,
  result: &LayoutResult,
  appeal: BreakAppeal,
  is_forced_break: bool,
) {
  if space.should_propagate_child_break_values()
    && !is_forced_break
    && let Some((_, child_style)) = child
  {
    builder.propagate_child_break_values(result, child_style);
  }

  builder.add_break_before_child(child.map(|(node, _)| node), appeal, is_forced_break);
}

/// Blink's `MovePastBreakpoint`: whether a block child, or the next line as `None`, fits, or
/// breaking before it is no better than going on.
pub(crate) fn move_past_breakpoint(
  space: &ConstraintSpace,
  builder: &mut BoxFragmentBuilder,
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

  let Some(fragmentainer_block_size) = builder.fragmentainer_capacity(true) else {
    return true;
  };
  let break_token = result.break_info.break_token.as_deref();
  let space_left = fragmentainer_block_size - fragmentainer_block_offset;
  // Breaking at the page start makes no progress.
  let refuse_break_before = space_left >= fragmentainer_block_size
    && !is_breakable_at_start_of_resumed_container(
      space,
      builder,
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
          && builder
            .early_break_appeal()
            .is_none_or(|early_appeal| appeal_inside >= early_appeal)
        {
          move_past = true;
        }
      } else {
        move_past = true;
      }
    } else if appeal_before == BreakAppeal::LastResort && builder.requires_content_before_breaking {
      builder.is_block_size_for_fragmentation_clamped = true;
      move_past = true;
    }
  }

  if move_past && let Some(child) = child {
    update_early_break_at_block_child(space, builder, child, result, appeal_before);
  }
  move_past
}

/// Blink's `UpdateEarlyBreakAtBlockChild`: the breakpoint before or inside `child` the layout may
/// come back to.
fn update_early_break_at_block_child(
  space: &ConstraintSpace,
  builder: &mut BoxFragmentBuilder,
  (child, child_style): (NodeId, &LayoutStyle),
  result: &LayoutResult,
  appeal_before: BreakAppeal,
) {
  let mut appeal_inside = BreakAppeal::LastResort;

  if let Some(breakpoint) = &result.break_info.early_break {
    appeal_inside = calculate_break_appeal_inside(
      result,
      Some((
        breakpoint.break_appeal,
        space,
        child_style.breaks.break_inside,
      )),
    );
    if builder
      .early_break_appeal()
      .is_none_or(|early_appeal| early_appeal <= breakpoint.break_appeal)
    {
      builder.early_break = Some(Rc::new(EarlyBreak::block(
        child,
        appeal_inside,
        Some(Rc::clone(breakpoint)),
      )));
    }
  }
  if appeal_before <= appeal_inside {
    return;
  }
  if builder
    .early_break_appeal()
    .is_some_and(|early_appeal| early_appeal > appeal_before)
  {
    return;
  }
  builder.early_break = Some(Rc::new(EarlyBreak::block(child, appeal_before, None)));
}

/// Blink's `AttemptSoftBreak`: breaks before a block child, or the next line as `None`, unless a
/// breakpoint seen earlier is better.
pub(crate) fn attempt_soft_break(
  space: &ConstraintSpace,
  builder: &mut BoxFragmentBuilder,
  child: Option<(NodeId, &LayoutStyle)>,
  result: &LayoutResult,
  appeal_before: BreakAppeal,
) -> bool {
  if builder
    .early_break_appeal()
    .is_some_and(|early_appeal| early_appeal > appeal_before)
  {
    return false;
  }

  break_before_child(space, builder, child, result, appeal_before, false);
  true
}

/// Blink's `AdjustedMarginAfterFinalChildFragment`: the last child's block-end margin, cut where
/// the page ends.
pub(crate) fn adjusted_margin_after_final_child_fragment(
  space: &ConstraintSpace,
  builder: &BoxFragmentBuilder,
  block_offset: LayoutUnit,
  block_end_margin: LayoutUnit,
) -> LayoutUnit {
  if !space.has_block_fragmentation() {
    return block_end_margin;
  }

  let space_left = builder.fragmentainer_space_left(true) - block_offset;

  block_end_margin.min(space_left.clamp_negative_to_zero())
}

/// Blink's `FinishFragmentation`.
pub(crate) fn finish_fragmentation(builder: &mut BoxFragmentBuilder) -> Result<(), LayoutStatus> {
  let previously_consumed_block_size = builder.previously_consumed_block_size();
  let mut space_left = builder.fragmentainer_space_left(false);
  let previous_break_token = builder.previous_break_token.clone();
  let is_past_end = previous_break_token
    .as_deref()
    .is_some_and(|token| token.is_at_block_end);
  let desired_block_size =
    (builder.fragments_total_block_size - previously_consumed_block_size).clamp_negative_to_zero();
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

  if previously_consumed_block_size > LayoutUnit::ZERO
    && (builder.box_decoration_break == BoxDecorationBreak::Slice || is_past_end)
  {
    builder.includes_block_start = false;
  }
  if (builder.did_break_self && !builder.should_clone_box_end_decorations) || is_past_end {
    builder.includes_block_end = false;
  }
  builder.consumed_block_size = previously_consumed_block_size + final_block_size;
  builder.fragment_block_size = final_block_size;

  if !has_known_space {
    return Ok(());
  }

  let was_broken_by_child = builder.has_inflow_child_break_inside() || builder.has_float_break;

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
    } else if !builder.should_clone_box_end_decorations {
      builder.includes_block_end = false;
    }
    return Ok(());
  }

  if desired_block_size > space_left {
    if previously_consumed_block_size == LayoutUnit::ZERO
      && space_left < builder.border_padding.block_start
    {
      // The block-start border and padding do not fit: breaking here is a last resort.
      builder.clamp_break_appeal(BreakAppeal::LastResort);
    }
    if desired_block_size <= desired_intrinsic_block_size {
      if builder.early_break.is_some() {
        return Err(LayoutStatus::NeedsEarlierBreak);
      }
      builder.clamp_break_appeal(BreakAppeal::LastResort);
    }
    return Ok(());
  }

  builder.is_at_block_end = true;
  Ok(())
}
