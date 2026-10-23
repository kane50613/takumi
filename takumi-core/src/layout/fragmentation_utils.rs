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
    constraint_space::ConstraintSpace,
    early_break::{EarlyBreak, EarlyBreakTarget},
    layout_result::{LayoutResult, LayoutStatus},
    layout_style::LayoutStyle,
    logical_rect::LogicalBoxStrut,
  },
  layout_unit::LayoutUnit,
  style::{BreakBetween, BreakInside},
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

/// Blink's `IsBreakInside`: the token resumes a box that made a fragment.
pub(crate) fn is_break_inside(token: Option<&BlockBreakToken>) -> bool {
  token.is_some_and(|token| !token.is_break_before)
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
