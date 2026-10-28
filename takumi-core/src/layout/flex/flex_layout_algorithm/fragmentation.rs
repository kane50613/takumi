//! Blink's block fragmentation in
//! [`FlexLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/flex/flex_layout_algorithm.cc)
//! and its [`FlexBreakTokenData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/flex/flex_break_token_data.h),
//! for pages alone, without gap decorations.
//!
//! Follows Blink under the notice in LICENSE-CHROMIUM.

use std::{collections::BTreeMap, rc::Rc};

use super::{BaselineAccumulator, FlexLayoutAlgorithm, ItemPosition};
use crate::{
  geometry::NodeId,
  layout::{
    box_fragment_builder::BoxFragmentBuilder,
    break_appeal::BreakAppeal,
    break_token_algorithm_data::BreakTokenAlgorithmData,
    constraint_space::ConstraintSpace,
    early_break::{EarlyBreak, EarlyBreakTarget},
    flex::{
      flex_item::{FlexItem, FlexItemData, FlexLine},
      flex_item_iterator::FlexItemIterator,
    },
    fragmentation_layout_input::FragmentationLayoutInput,
    fragmentation_utils::{
      FlexColumnBreakInfo, FlexItemBreak, attempt_soft_break, break_before_child,
      break_before_child_if_needed, finish_fragmentation, fragment_index,
      fragmentation_layout_input_for_child, is_avoid_break_value, is_break_inside,
      is_breakable_at_start_of_resumed_container, is_early_break_target, is_forced_break_value,
      join_fragmentainer_break_values, setup_fragment_builder_for_fragmentation,
      setup_space_builder_for_fragmentation,
    },
    layout_result::{LayoutResult, LayoutStatus, OutOfFlowCandidate},
    layout_style::LayoutStyle,
    length_utils::LengthResolver,
    logical_rect::{LogicalBoxStrut, LogicalOffset},
    tree::LayoutTree,
  },
  layout_unit::LayoutUnit,
  style::{BoxDecorationBreak, BreakBetween},
};

/// Blink's `FlexBreakBeforeRow`: whether the container broke before a row, at its start or once
/// past it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum FlexBreakBeforeRow {
  #[default]
  None,
  AtStart,
  PastStart,
}

/// Blink's `FlexBreakTokenData`: the lines a container laid out in one piece, which each page lays
/// its items out from.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FlexBreakTokenData {
  pub(crate) flex_lines: Vec<FlexLine>,
  pub(crate) row_break_between: Vec<BreakBetween>,
  pub(crate) out_of_flow_children: Vec<NodeId>,
  pub(crate) intrinsic_block_size: LayoutUnit,
  pub(crate) break_before_row: FlexBreakBeforeRow,
}

/// The state of Blink's `FlexLayoutAlgorithm` that block fragmentation reads and writes.
#[derive(Default)]
pub(super) struct FlexFragmentation {
  pub(super) input: FragmentationLayoutInput,
  /// Blink's `early_break_`: where the column being laid out breaks.
  early_break: Option<Rc<EarlyBreak>>,
  /// Blink's `additional_early_breaks_`: where each column breaks, from an earlier layout.
  additional_early_breaks: Option<Vec<Option<Rc<EarlyBreak>>>>,
  /// Blink's `column_early_breaks_`: the breakpoint each column found to go back to.
  column_early_breaks: Vec<Option<Rc<EarlyBreak>>>,
  /// Blink's `cross_size_adjustments_`: how much each row grows, by line index plus one.
  cross_size_adjustments: Option<BTreeMap<usize, LayoutUnit>>,
  /// Blink's `row_cross_size_updates_`: the growth this layout found, for a layout again.
  row_cross_size_updates: BTreeMap<usize, LayoutUnit>,
  /// Blink's `row_break_between_outputs`: the joined break values between each row, or before and
  /// after every column.
  row_break_between: Vec<BreakBetween>,
  break_before_row: FlexBreakBeforeRow,
  has_processed_first_line: bool,
  last_line_index_to_process_first_child: Option<usize>,
  /// Blink's `intrinsic_block_size_` of this fragment.
  intrinsic_block_size: LayoutUnit,
}

impl FlexFragmentation {
  pub(super) fn new(input: &FragmentationLayoutInput) -> Self {
    Self {
      input: input.clone(),
      early_break: input.early_break.clone(),
      ..Self::default()
    }
  }
}

/// Sets up the fragment builder of the flex container `node`, `inline_size` wide in `space`.
pub(super) fn flex_fragment_builder(
  (tree, node): (&LayoutTree<'_>, NodeId),
  space: &ConstraintSpace,
  border_padding: LogicalBoxStrut,
  inline_size: Option<LayoutUnit>,
  input: &FragmentationLayoutInput,
) -> BoxFragmentBuilder {
  let style = tree.layout_style(node);
  let mut builder =
    BoxFragmentBuilder::new(space, style, border_padding, input.break_token.clone());

  builder.has_non_visible_block_overflow = tree.has_non_visible_block_overflow(node);
  setup_fragment_builder_for_fragmentation(
    space,
    style,
    inline_size.unwrap_or_default(),
    &mut builder,
  );
  builder
}

impl FlexLayoutAlgorithm<'_, '_> {
  pub(super) fn is_involved_in_block_fragmentation(&self) -> bool {
    self.builder.involved_in_block_fragmentation()
  }

  pub(super) fn is_resuming(&self) -> bool {
    is_break_inside(self.fragmentation.input.break_token.as_deref())
  }

  pub(super) fn fragment_index(&self) -> usize {
    fragment_index(self.fragmentation.input.break_token.as_deref())
  }

  pub(super) fn border_padding_for_size(&self, border_padding: LogicalBoxStrut) -> LogicalBoxStrut {
    self.builder.border_padding_for_size(border_padding)
  }

  pub(super) fn previously_consumed_block_size(&self) -> LayoutUnit {
    self.builder.previously_consumed_block_size()
  }

  /// The lines, out-of-flow children, and intrinsic block size an earlier page laid out, which
  /// this one resumes from.
  pub(super) fn resumed_lines(&mut self) -> Option<(Vec<FlexLine>, Vec<NodeId>, LayoutUnit)> {
    if !self.is_resuming() {
      return None;
    }

    let token = self.fragmentation.input.break_token.as_deref()?;
    let Some(BreakTokenAlgorithmData::Flex(data)) = &token.algorithm_data else {
      return None;
    };

    self.fragmentation.row_break_between = data.row_break_between.clone();
    self.fragmentation.break_before_row = data.break_before_row;
    Some((
      data.flex_lines.clone(),
      data.out_of_flow_children.clone(),
      data.intrinsic_block_size,
    ))
  }

  /// Blink's `row_break_between_outputs` before the items join theirs in: one value between each
  /// row, or one before and one after every column.
  pub(super) fn reset_row_break_between(&mut self, line_count: usize) {
    if !self.space.should_propagate_child_break_values() {
      return;
    }

    let count = if self.is_column { 2 } else { line_count + 1 };

    self.fragmentation.row_break_between = vec![BreakBetween::Auto; count];
  }

  /// Joins the break values of the item of `item_style` laid out as `result`, the `item_index`th
  /// item of `line`, into the break values between rows or columns.
  pub(super) fn join_row_break_between(
    &mut self,
    line: &FlexLine,
    line_index: usize,
    item_index: usize,
    item_style: &LayoutStyle,
    result: &LayoutResult,
  ) {
    if !self.space.should_propagate_child_break_values() {
      return;
    }

    let break_before = join_fragmentainer_break_values(
      item_style.breaks.break_before,
      result.break_info.initial_break_before.unwrap_or_default(),
    );
    let break_after = join_fragmentainer_break_values(
      item_style.breaks.break_after,
      result.break_info.final_break_after.unwrap_or_default(),
    );
    let row_break_between = &mut self.fragmentation.row_break_between;
    let (before, after) = if self.is_column {
      (
        (line.item_indices.first() == Some(&item_index)).then_some(0),
        (line.item_indices.last() == Some(&item_index)).then_some(1),
      )
    } else {
      (Some(line_index), Some(line_index + 1))
    };

    if let Some(index) = before {
      row_break_between[index] =
        join_fragmentainer_break_values(row_break_between[index], break_before);
    }
    if let Some(index) = after {
      row_break_between[index] =
        join_fragmentainer_break_values(row_break_between[index], break_after);
    }
  }

  /// Keeps what the item at `offset` laid out as `result` needs to lay out on each page, as Blink's
  /// `GiveItemsFinalPositionAndSize` does when it fragments.
  pub(super) fn keep_item_for_fragmentation(
    &self,
    flex_line: &mut FlexLine,
    item: &FlexItem,
    item_index: usize,
    offset: LogicalOffset,
    margin_block_end: LayoutUnit,
    result: &LayoutResult,
  ) {
    flex_line.line_items_data.push(FlexItemData {
      node: item.node,
      item_index,
      offset,
      alignment: item.alignment,
      main_axis_final_size: item.flexed_border_box_size(),
      margin_block_end,
      total_remaining_block_size: result.size.block_size,
      is_initial_block_size_indefinite: item.is_initial_block_size_indefinite,
      is_used_flex_basis_indefinite: item.is_used_flex_basis_indefinite,
      has_descendant_that_depends_on_percentage_block_size: self
        .tree
        .render_node(item.node)
        .has_descendant_that_depends_on_percentage_block_size(),
    });
  }

  /// The fragmentation part of Blink's `LayoutInternal`: the items laid out on this page, and the
  /// fragment's block size.
  pub(super) fn fragment_block_size(
    &mut self,
    flex_lines: &mut [FlexLine],
    total_intrinsic_block_size: &mut LayoutUnit,
  ) -> Result<LayoutUnit, LayoutStatus> {
    if !self.is_involved_in_block_fragmentation() {
      return Ok(self.total_block_size);
    }

    let previously_consumed_block_size = self.previously_consumed_block_size();

    self.fragmentation.intrinsic_block_size = self.builder.border_padding.block_start;
    self.give_items_final_position_and_size_for_fragmentation(
      flex_lines,
      total_intrinsic_block_size,
    )?;

    let intrinsic_block_size =
      self.fragmentation.intrinsic_block_size + self.builder.border_padding.block_end;
    let style = Rc::clone(&self.style);
    let mut resolver = LengthResolver::new(self.space, &style);

    resolver.border_padding = self.border_padding_for_size(resolver.border_padding);

    let stitched_intrinsic_block_size = previously_consumed_block_size + intrinsic_block_size;
    let block_size = resolver
      .block_size_for_fragment(Some(stitched_intrinsic_block_size), self.inline_size, None)
      .unwrap_or(stitched_intrinsic_block_size);

    self.builder.intrinsic_block_size = intrinsic_block_size;
    self.builder.fragments_total_block_size = block_size;
    finish_fragmentation(&mut self.builder)?;
    self.fragmentation.intrinsic_block_size = self.builder.intrinsic_block_size;

    if self.space.should_propagate_child_break_values()
      && let (Some(&first), Some(&last)) = (
        self.fragmentation.row_break_between.first(),
        self.fragmentation.row_break_between.last(),
      )
    {
      self.builder.initial_break_before = Some(first);
      self.builder.previous_break_after = last;
    }
    Ok(self.builder.fragment_block_size)
  }

  /// The intrinsic block size of this fragment, or of the container in one piece.
  pub(super) fn fragment_intrinsic_block_size(
    &self,
    total_intrinsic_block_size: LayoutUnit,
  ) -> LayoutUnit {
    if self.is_involved_in_block_fragmentation() {
      self.fragmentation.intrinsic_block_size
    } else {
      total_intrinsic_block_size
    }
  }

  /// Keeps the lines in the break token for the next page to resume from.
  pub(super) fn keep_break_token_data(
    &mut self,
    flex_lines: Vec<FlexLine>,
    out_of_flow_children: Vec<NodeId>,
    intrinsic_block_size: LayoutUnit,
  ) {
    if !self.space.has_block_fragmentation() {
      return;
    }

    self.builder.break_token_data =
      Some(BreakTokenAlgorithmData::Flex(Rc::new(FlexBreakTokenData {
        flex_lines,
        row_break_between: self.fragmentation.row_break_between.clone(),
        out_of_flow_children,
        intrinsic_block_size,
        break_before_row: self.fragmentation.break_before_row,
      })));
  }

  /// Blink's `HandleOutOfFlowPositionedItems` part for fragmentation: whether this fragment places
  /// the out-of-flow children at the block end and block center, which a later page holds when the
  /// container goes on there.
  pub(super) fn out_of_flow_block_edges(
    &mut self,
    total_intrinsic_block_size: LayoutUnit,
  ) -> (bool, bool) {
    if !self.is_involved_in_block_fragmentation() {
      return (true, true);
    }

    let should_process_block_end = !self.builder.should_break();

    if should_process_block_end {
      let style = Rc::clone(&self.style);
      let mut resolver = LengthResolver::new(self.space, &style);

      resolver.border_padding = self.border_padding_for_size(resolver.border_padding);
      self.total_block_size = resolver
        .block_size_for_fragment(Some(total_intrinsic_block_size), self.inline_size, None)
        .unwrap_or(total_intrinsic_block_size);
      return (true, true);
    }

    let center = self.total_block_size / 2;
    let capacity = self
      .builder
      .fragmentainer_capacity(true)
      .unwrap_or(LayoutUnit::MAX);

    (
      false,
      center - self.previously_consumed_block_size() <= capacity,
    )
  }

  /// The out-of-flow children an earlier fragment broke inside or pushed here, as candidates.
  pub(super) fn resumed_out_of_flow(&self) -> Vec<OutOfFlowCandidate> {
    self.tree.resumed_out_of_flow(&self.builder)
  }

  /// What the container's layout result tells its container about breaking.
  pub(super) fn finish_break_info(&mut self, result: &mut LayoutResult) {
    result.break_info = self.builder.finish_break_info(self.node);
    self.builder.exclude_broken_sides(result);
  }

  /// Blink's `Layout` after a layout that did not succeed: again to break at the breakpoints it
  /// found, with the rows grown to fit their items, or without breaking.
  pub(super) fn relayout(self, status: LayoutStatus) -> LayoutResult {
    let mut input = self.fragmentation.input.clone();
    let mut additional_early_breaks = self.fragmentation.additional_early_breaks.clone();
    let mut cross_size_adjustments = None;

    match status {
      LayoutStatus::NeedsEarlierBreak if self.builder.early_break.is_some() => {
        input.early_break = self.builder.early_break.clone();
        additional_early_breaks = Some(self.fragmentation.column_early_breaks.clone());
      }
      LayoutStatus::NeedsRelayoutWithRowCrossSizeChanges => {
        cross_size_adjustments = Some(self.fragmentation.row_cross_size_updates.clone());
      }
      LayoutStatus::DisableFragmentation => {
        let space = self.space.without_fragmentation();
        let input = self.fragmentation.input.without_early_break();

        return FlexLayoutAlgorithm::new(self.tree, self.node, &space, false, &input).layout();
      }
      _ => {
        let mut result = LayoutResult::new(Default::default());

        result.status = status;
        result.break_info.early_break = self.builder.early_break.clone();
        return result;
      }
    }

    let mut algorithm = FlexLayoutAlgorithm::new(self.tree, self.node, self.space, false, &input);

    algorithm.fragmentation.additional_early_breaks = additional_early_breaks;
    algorithm.fragmentation.cross_size_adjustments = cross_size_adjustments;
    algorithm.layout()
  }

  /// Blink's `FragmentainerSpaceAvailable`.
  fn fragmentainer_space_available(&self, block_offset: LayoutUnit) -> LayoutUnit {
    (self.builder.fragmentainer_space_left(true) - block_offset).clamp_negative_to_zero()
  }

  /// Blink's `ConsumeRemainingFragmentainerSpace`: the fragment fills the rest of the page after a
  /// break, and a forced break moves the line's later items up to the next page's start.
  fn consume_remaining_fragmentainer_space(
    &mut self,
    offset_in_stitched_container: LayoutUnit,
    flex_line: &mut FlexLine,
    column_break_info: Option<&FlexColumnBreakInfo>,
  ) {
    if self
      .builder
      .last_child_break_token()
      .is_some_and(|token| token.is_forced_break)
    {
      let intrinsic_block_size = column_break_info
        .map_or(self.fragmentation.intrinsic_block_size, |column| {
          column.column_intrinsic_block_size
        });

      flex_line.item_offset_adjustment -= intrinsic_block_size + offset_in_stitched_container
        - self.builder.cloned_block_start_decoration();
    }
    if !self.space.has_block_fragmentation() {
      return;
    }
    self.fragmentation.intrinsic_block_size +=
      self.fragmentainer_space_available(self.fragmentation.intrinsic_block_size);
  }

  /// Blink's `BreakBeforeRowIfNeeded`: whether the layout broke before the row `row` at
  /// `row_block_offset`, whose first item is `child`.
  #[allow(clippy::too_many_arguments)]
  fn break_before_row_if_needed(
    &mut self,
    row: &FlexLine,
    row_block_offset: LayoutUnit,
    row_break_between: BreakBetween,
    row_index: usize,
    child: NodeId,
    has_container_separation: bool,
    is_first_for_row: bool,
  ) -> Result<bool, LayoutStatus> {
    let fragmentainer_block_offset = self.builder.fragmentainer_offset(true) + row_block_offset;
    let child_style = Rc::clone(self.tree.layout_style_rc(child));

    if has_container_separation && is_forced_break_value(self.space, row_break_between) {
      break_before_child(
        self.space,
        &mut self.builder,
        Some((child, &child_style)),
        None,
        BreakAppeal::Perfect,
        true,
      );
      return Ok(true);
    }

    let breakable_at_start_of_container =
      is_breakable_at_start_of_resumed_container(self.space, &self.builder, is_first_for_row);
    // Blink's `CalculateBreakAppealBefore` for a row that laid out.
    let mut appeal_before = BreakAppeal::Perfect;

    if !has_container_separation {
      appeal_before = if breakable_at_start_of_container {
        self.space.min_break_appeal()
      } else {
        BreakAppeal::LastResort
      };
    }
    if appeal_before != BreakAppeal::LastResort
      && is_avoid_break_value(self.space, row_break_between)
    {
      appeal_before = appeal_before.min(BreakAppeal::ViolatingBreakAvoid);
    }

    if self.move_past_row_break_point(
      appeal_before,
      fragmentainer_block_offset,
      row.line_cross_size,
      row_index,
      has_container_separation,
      breakable_at_start_of_container,
    ) {
      return Ok(false);
    }
    if !attempt_soft_break(
      self.space,
      &mut self.builder,
      Some((child, &child_style)),
      None,
      appeal_before,
      &FlexItemBreak::default(),
    ) {
      return Err(LayoutStatus::NeedsEarlierBreak);
    }
    Ok(true)
  }

  /// Blink's `MovePastRowBreakPoint`: whether the row fits, keeping the break before it as one to
  /// come back to.
  fn move_past_row_break_point(
    &mut self,
    appeal_before: BreakAppeal,
    fragmentainer_block_offset: LayoutUnit,
    row_block_size: LayoutUnit,
    row_index: usize,
    has_container_separation: bool,
    breakable_at_start_of_container: bool,
  ) -> bool {
    let Some(capacity) = self.builder.fragmentainer_capacity(true) else {
      return true;
    };
    let space_left = capacity - fragmentainer_block_offset;

    if space_left < LayoutUnit::ZERO
      || (space_left == LayoutUnit::ZERO && row_block_size != LayoutUnit::ZERO)
    {
      return false;
    }
    if (has_container_separation || breakable_at_start_of_container)
      && self
        .builder
        .early_break_appeal()
        .is_none_or(|early_appeal| appeal_before >= early_appeal)
    {
      self.builder.early_break = Some(Rc::new(EarlyBreak::line(row_index, appeal_before)));
    }
    true
  }

  /// Blink's `AddColumnEarlyBreak`.
  fn add_column_early_break(&mut self, breakpoint: Option<Rc<EarlyBreak>>, index: usize) {
    let column_early_breaks = &mut self.fragmentation.column_early_breaks;

    if column_early_breaks.len() <= index {
      column_early_breaks.resize(index + 1, None);
    }
    column_early_breaks[index] = breakpoint;
  }

  /// Blink's `AdjustOffsetForNextLine`: the lines after `line_index` move by `item_expansion`.
  fn adjust_offset_for_next_line(
    flex_lines: &mut [FlexLine],
    line_index: usize,
    item_expansion: LayoutUnit,
  ) {
    if let Some(next_line) = flex_lines.get_mut(line_index + 1) {
      next_line.item_offset_adjustment += item_expansion;
    }
  }

  /// Blink's `MinBlockSizeShouldEncompassIntrinsicSize`: whether the item grows past its block
  /// size when a break pushes its content down.
  fn min_block_size_should_encompass_intrinsic_size(&self, item: &FlexItemData) -> bool {
    if item.has_descendant_that_depends_on_percentage_block_size
      || self.tree.is_monolithic(item.node)
    {
      return false;
    }

    let item_style = self.tree.layout_style(item.node);

    if self.is_column {
      let can_shrink =
        item_style.flex_shrink != 0.0 && self.child_available_size.block_size.is_some();
      let height = item_style.size.height;

      (item.is_used_flex_basis_indefinite && !can_shrink)
        || ((height.is_auto() || height.is_content_or_intrinsic())
          && (!can_shrink || self.should_apply_auto_min_size(item_style)))
    } else {
      let container_height = self.style.size.height;

      if self.will_child_cross_size_be_container_cross_size(item_style, item.alignment)
        && !(container_height.is_auto() || container_height.is_content_or_intrinsic())
      {
        return false;
      }
      self.does_item_computed_cross_size_have_auto(item_style)
    }
  }

  /// Blink's `IsContainerCrossSizeDefinite`.
  fn is_container_cross_size_definite(&self) -> bool {
    self.is_column || self.child_available_size.block_size.is_some()
  }

  /// Blink's `DoesItemComputedCrossSizeHaveAuto`.
  fn does_item_computed_cross_size_have_auto(&self, item_style: &LayoutStyle) -> bool {
    if self.is_horizontal_flow {
      item_style.size.height.is_auto()
    } else {
      item_style.size.width.is_auto()
    }
  }

  /// Blink's `WillChildCrossSizeBeContainerCrossSize`.
  fn will_child_cross_size_be_container_cross_size(
    &self,
    item_style: &LayoutStyle,
    alignment: ItemPosition,
  ) -> bool {
    !self.is_multi_line
      && self.is_cross_size_definite
      && alignment == ItemPosition::Stretch
      && self.does_item_computed_cross_size_have_auto(item_style)
  }

  /// Blink's `GiveItemsFinalPositionAndSizeForFragmentation`: the items laid out on this page from
  /// the lines laid out in one piece, broken where the page ends.
  fn give_items_final_position_and_size_for_fragmentation(
    &mut self,
    flex_lines: &mut [FlexLine],
    total_intrinsic_block_size: &mut LayoutUnit,
  ) -> Result<(), LayoutStatus> {
    let break_token = self.fragmentation.input.break_token.clone();
    let mut item_iterator = FlexItemIterator::new(
      flex_lines,
      self.tree.without_out_of_flow_tokens(break_token.as_ref()),
      self.is_column,
    );
    let mut has_inflow_child_break_inside_line = vec![false; flex_lines.len()];
    let mut needs_earlier_break_in_column = false;
    let fragmentainer_space = self.builder.fragmentainer_space_left(true);
    let mut column_break_info = if self.is_column {
      vec![FlexColumnBreakInfo::default(); flex_lines.len()]
    } else {
      Vec::new()
    };
    let mut previously_consumed_block_size = LayoutUnit::ZERO;
    let mut offset_in_stitched_container = LayoutUnit::ZERO;

    if let Some(token) = break_token
      .as_deref()
      .filter(|token| is_break_inside(Some(token)))
    {
      previously_consumed_block_size = token.consumed_block_size;
      offset_in_stitched_container = previously_consumed_block_size;

      // The cloned border and padding of the earlier fragments are not part of the content.
      if self.style.breaks.box_decoration_break == BoxDecorationBreak::Clone
        && offset_in_stitched_container != LayoutUnit::MAX
      {
        let preceding_fragment_count = token.sequence_number as i32 + 1;

        offset_in_stitched_container -= preceding_fragment_count
          * self.builder.border_padding.block_sum()
          - self.builder.border_padding.block_start;
      }
    }

    let mut baselines = BaselineAccumulator::default();
    let broke_before_row = self.fragmentation.break_before_row != FlexBreakBeforeRow::None;
    let mut status = Ok(());

    while let Some(entry) = item_iterator.next_item(flex_lines, broke_before_row) {
      let item_index = entry.item_index;
      let line_index = entry.line_index;
      let item_break_token = entry.token;
      let line_count = flex_lines.len();
      let is_last_item_in_line = item_index + 1 == flex_lines[line_index].line_items_data.len();
      let is_first_line = line_index == 0;
      let is_last_line = line_index + 1 == line_count;

      if !self.is_column {
        if line_index != 0 && has_inflow_child_break_inside_line[line_index - 1] {
          break;
        }
      } else {
        if let Some(early_break) = self
          .fragmentation
          .additional_early_breaks
          .as_ref()
          .and_then(|early_breaks| early_breaks.get(line_index))
        {
          self.fragmentation.early_break = early_break.clone();
        } else if self.fragmentation.early_break.is_some() && line_index != 0 {
          self.fragmentation.early_break = None;
        }

        if has_inflow_child_break_inside_line[line_index] {
          if !is_last_item_in_line {
            item_iterator.next_line(flex_lines);
          }
          continue;
        }
      }

      let mut row_block_offset = if self.is_column {
        LayoutUnit::ZERO
      } else {
        flex_lines[line_index].cross_axis_offset
      };
      let original_offset = flex_lines[line_index].line_items_data[item_index].offset;
      let mut offset = original_offset;
      let mut individual_item_adjustment = LayoutUnit::ZERO;

      if let Some(token) = item_break_token
        .as_deref()
        .filter(|token| token.is_break_before)
      {
        if token.is_forced_break {
          flex_lines[line_index].item_offset_adjustment += offset_in_stitched_container;
        } else if !self.is_column && item_index == 0 && broke_before_row {
          if self.fragmentation.break_before_row == FlexBreakBeforeRow::AtStart {
            let previous_row_end = if is_first_line {
              LayoutUnit::ZERO
            } else {
              flex_lines[line_index - 1].line_cross_end()
            };
            let previous_fragmentainer_unused_space =
              (offset_in_stitched_container - previous_row_end).clamp_negative_to_zero();
            let mut consumed_row_gap = LayoutUnit::ZERO;

            if previous_fragmentainer_unused_space != LayoutUnit::ZERO {
              let total_row_block_offset =
                row_block_offset + flex_lines[line_index].item_offset_adjustment;
              let row_gap = total_row_block_offset - previous_row_end;

              consumed_row_gap = row_gap.min(previous_fragmentainer_unused_space);
            }

            flex_lines[line_index].item_offset_adjustment +=
              offset_in_stitched_container - previous_row_end - consumed_row_gap;
          }
        } else {
          let total_item_block_offset =
            offset.block_offset + flex_lines[line_index].item_offset_adjustment;

          individual_item_adjustment =
            (offset_in_stitched_container - total_item_block_offset).clamp_negative_to_zero();
          if self.is_column {
            flex_lines[line_index].item_offset_adjustment += individual_item_adjustment;
          }
        }
      }

      if is_break_inside(item_break_token.as_deref()) {
        offset.block_offset = self.builder.border_padding.block_start;
      } else if self.is_resuming() {
        let offset_adjustment = offset_in_stitched_container
          - flex_lines[line_index].item_offset_adjustment
          - self.builder.border_padding.block_start;

        offset.block_offset -= offset_adjustment;
        if !self.is_column {
          offset.block_offset += individual_item_adjustment;
          row_block_offset -= offset_adjustment;
        }
      }

      let node = flex_lines[line_index].line_items_data[item_index].node;

      if let Some(early_break) = self.fragmentation.early_break.clone() {
        if !self.is_column {
          self.builder.line_count = line_index;
        }
        if is_early_break_target(&early_break, &self.builder, Some(node)) {
          self
            .builder
            .add_break_before_child(Some(node), BreakAppeal::Perfect, false);
          if matches!(early_break.target, EarlyBreakTarget::Line(_)) {
            self.fragmentation.break_before_row = FlexBreakBeforeRow::AtStart;
          }
          self.consume_remaining_fragmentainer_space(
            offset_in_stitched_container,
            &mut flex_lines[line_index],
            None,
          );
          has_inflow_child_break_inside_line[line_index] = true;
          if self.is_column {
            if !is_last_item_in_line {
              item_iterator.next_line(flex_lines);
            }
          } else if is_last_item_in_line {
            break;
          }
          self.fragmentation.last_line_index_to_process_first_child = Some(line_index);
          continue;
        }
      }

      if let Some(adjustment) = self
        .fragmentation
        .cross_size_adjustments
        .as_ref()
        .and_then(|adjustments| adjustments.get(&(line_index + 1)))
        .copied()
        && self
          .fragmentation
          .last_line_index_to_process_first_child
          .is_none_or(|last| last < line_index)
      {
        flex_lines[line_index].line_cross_size += adjustment;
        Self::adjust_offset_for_next_line(flex_lines, line_index, adjustment);
      }

      let mut line_cross_size = flex_lines[line_index].line_cross_size;

      if !self.is_column
        && let Some(token) = item_break_token.as_deref()
      {
        line_cross_size = (line_cross_size
          - (offset_in_stitched_container
            - (original_offset.block_offset + flex_lines[line_index].item_offset_adjustment)
            - token.consumed_block_size))
          .clamp_negative_to_zero();
      }

      let item = flex_lines[line_index].line_items_data[item_index].clone();
      let item_style = Rc::clone(self.tree.layout_style_rc(node));
      let mut child_space = self.space_for_layout(
        &item_style,
        item.alignment,
        item.is_initial_block_size_indefinite,
        None,
        Some(item.main_axis_final_size),
        Some(line_cross_size),
      );

      if self.space.has_block_fragmentation() {
        setup_space_builder_for_fragmentation(
          self.space,
          &self.builder,
          self.tree.is_monolithic(node),
          offset.block_offset,
          &mut child_space,
        );
        if self.min_block_size_should_encompass_intrinsic_size(&item)
          && let Some(fragmentation) = &mut child_space.block_fragmentation
        {
          fragmentation.min_block_size_should_encompass_intrinsic_size = true;
        }
      }

      let input = fragmentation_layout_input_for_child(
        node,
        item_break_token.clone(),
        self.fragmentation.early_break.as_deref(),
      );
      let result = self.tree.layout_fragment(node, &child_space, &input);
      let mut broke_before = false;
      let mut column_index = None;

      if self.fragmentation.early_break.is_none() && self.space.has_block_fragmentation() {
        let has_container_separation;

        if !self.is_column {
          has_container_separation = offset.block_offset > row_block_offset
            && item_break_token
              .as_deref()
              .is_none_or(|token| broke_before_row && item_index == 0 && token.is_break_before);

          if item_index == 0
            && item_break_token
              .as_deref()
              .is_none_or(|token| token.is_break_before && broke_before_row)
          {
            let row = flex_lines[line_index].clone();
            let row_break_between = self
              .fragmentation
              .row_break_between
              .get(line_index)
              .copied()
              .unwrap_or_default();
            let is_first_for_row = item_break_token.is_none() || broke_before_row;

            match self.break_before_row_if_needed(
              &row,
              row_block_offset,
              row_break_between,
              line_index,
              node,
              self.fragmentation.has_processed_first_line,
              is_first_for_row,
            ) {
              Ok(true) => {
                self.consume_remaining_fragmentainer_space(
                  offset_in_stitched_container,
                  &mut flex_lines[line_index],
                  None,
                );
                self.fragmentation.break_before_row = if broke_before_row {
                  FlexBreakBeforeRow::PastStart
                } else {
                  FlexBreakBeforeRow::AtStart
                };
                break;
              }
              Ok(false) => {
                self.fragmentation.break_before_row = FlexBreakBeforeRow::None;
              }
              Err(error) => {
                self.fragmentation.break_before_row = FlexBreakBeforeRow::None;
                status = Err(error);
                break;
              }
            }
          }
        } else {
          has_container_separation = item_break_token.is_none()
            && (self
              .fragmentation
              .last_line_index_to_process_first_child
              .is_some_and(|last| last >= line_index)
              || offset.block_offset > LayoutUnit::ZERO);

          if line_count > 1 {
            column_index = Some(line_index);
            self.builder.previous_break_after = column_break_info[line_index].break_after;
          }
        }

        let fragmentainer_block_offset =
          self.builder.fragmentainer_offset(true) + offset.block_offset;
        let breaks = break_before_child_if_needed(
          self.space,
          &mut self.builder,
          (node, &item_style),
          &result,
          fragmentainer_block_offset,
          has_container_separation,
          FlexItemBreak {
            is_row_item: !self.is_column,
            column: column_index.map(|index| &mut column_break_info[index]),
          },
        );

        if let Some(index) = column_index {
          column_break_info[index].break_after = self.builder.previous_break_after;
        }

        match breaks {
          Ok(broke) => broke_before = broke,
          Err(error) => {
            if let Some(index) = column_index {
              if !needs_earlier_break_in_column {
                needs_earlier_break_in_column = true;
                self.builder.early_break = column_break_info[index].early_break.clone();
              }
              self.add_column_early_break(column_break_info[index].early_break.clone(), index);
              if !is_last_item_in_line {
                item_iterator.next_line(flex_lines);
              }
              continue;
            }
            status = Err(error);
            break;
          }
        }
      }

      if broke_before {
        self.consume_remaining_fragmentainer_space(
          offset_in_stitched_container,
          &mut flex_lines[line_index],
          column_index.map(|index| &column_break_info[index]),
        );
        has_inflow_child_break_inside_line[line_index] = true;
        if self.is_column {
          if !is_last_item_in_line {
            item_iterator.next_line(flex_lines);
          }
        } else if is_last_item_in_line {
          break;
        }
        self.fragmentation.last_line_index_to_process_first_child = Some(line_index);
        continue;
      }

      let is_at_block_end = result
        .break_info
        .break_token
        .as_deref()
        .is_none_or(|token| token.is_at_block_end);
      let mut item_block_end = offset.block_offset + result.size.block_size;

      if is_at_block_end {
        item_block_end += item.margin_block_end;
        flex_lines[line_index].line_items_data[item_index].margin_block_end = LayoutUnit::ZERO;
      } else {
        has_inflow_child_break_inside_line[line_index] = true;
      }

      if self.is_column {
        let cloned_block_decorations = if !is_at_block_end
          && item_style.breaks.box_decoration_break == BoxDecorationBreak::Clone
        {
          LayoutUnit::from_float(
            result.border.top + result.border.bottom + result.padding.top + result.padding.bottom,
          )
        } else {
          LayoutUnit::ZERO
        };
        let line = &mut flex_lines[line_index];
        let data = &mut line.line_items_data[item_index];

        data.main_axis_final_size += cloned_block_decorations;
        data.total_remaining_block_size -= result.size.block_size - cloned_block_decorations;
        if data.total_remaining_block_size < LayoutUnit::ZERO
          && result.break_info.break_token.is_none()
        {
          let expansion = -data.total_remaining_block_size;

          line.item_offset_adjustment += expansion;
        }
      } else if self.fragmentation.cross_size_adjustments.is_none()
        && !item.has_descendant_that_depends_on_percentage_block_size
      {
        let line_block_end = flex_lines[line_index].line_cross_end() - offset_in_stitched_container
          + self.builder.cloned_block_start_decoration();

        if line_block_end <= fragmentainer_space
          && line_block_end >= LayoutUnit::ZERO
          && offset_in_stitched_container != LayoutUnit::MAX
        {
          let item_expansion = if is_at_block_end {
            item_block_end - line_block_end
          } else {
            (fragmentainer_space - line_block_end).add_epsilon()
          };

          if item_expansion > LayoutUnit::ZERO {
            match self
              .fragmentation
              .row_cross_size_updates
              .get(&(line_index + 1))
            {
              None => {
                self
                  .fragmentation
                  .row_cross_size_updates
                  .insert(line_index + 1, item_expansion);
                Self::adjust_offset_for_next_line(flex_lines, line_index, item_expansion);
              }
              Some(&previous) if item_expansion > previous => {
                Self::adjust_offset_for_next_line(
                  flex_lines,
                  line_index,
                  item_expansion - previous,
                );
                self
                  .fragmentation
                  .row_cross_size_updates
                  .insert(line_index + 1, item_expansion);
              }
              Some(_) => {}
            }
          }
        }
      }

      if let Some(index) = column_index {
        let column = &mut column_break_info[index];

        column.column_intrinsic_block_size = column.column_intrinsic_block_size.max(item_block_end);
      }

      self.fragmentation.intrinsic_block_size =
        self.fragmentation.intrinsic_block_size.max(item_block_end);
      self
        .builder
        .add_result(self.space, &item_style, &result, offset.block_offset);
      if let Some(index) = column_index {
        column_break_info[index].break_after = self.builder.previous_break_after;
      }
      baselines.accumulate_item(&result, offset.block_offset, is_first_line, is_last_line);
      self.add_result(node, Rc::clone(&result), offset);

      if is_last_item_in_line
        || (!self.is_column && !item_iterator.has_next_item_in_line(line_index))
      {
        if !has_inflow_child_break_inside_line[line_index] {
          flex_lines[line_index].has_seen_all_children = true;
        }
        self.fragmentation.has_processed_first_line = true;

        if (result.break_info.break_token.is_none() || flex_lines[line_index].has_seen_all_children)
          && line_index + 1 < line_count
          && !self.is_column
          && !item_iterator.has_more_break_tokens()
        {
          let adjustment = flex_lines[line_index].item_offset_adjustment;

          flex_lines[line_index + 1].item_offset_adjustment += adjustment;
        }
      }
      self.fragmentation.last_line_index_to_process_first_child = Some(line_index);
    }

    if needs_earlier_break_in_column {
      return Err(LayoutStatus::NeedsEarlierBreak);
    }
    status?;

    if !self.fragmentation.row_cross_size_updates.is_empty()
      && (self.is_multi_line || !self.is_container_cross_size_definite())
    {
      return Err(LayoutStatus::NeedsRelayoutWithRowCrossSizeChanges);
    }
    if !self.builder.has_inflow_child_break_inside
      && item_iterator
        .next_item(flex_lines, broke_before_row)
        .is_none()
    {
      self.builder.has_seen_all_children = true;
    }

    self.first_baseline = baselines.first_baseline();
    self.last_baseline = baselines.last_baseline();
    *total_intrinsic_block_size = (*total_intrinsic_block_size)
      .max(self.fragmentation.intrinsic_block_size + previously_consumed_block_size);
    Ok(())
  }
}
