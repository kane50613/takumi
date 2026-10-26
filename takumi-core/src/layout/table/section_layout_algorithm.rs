//! Blink's [`TableSectionLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/table/table_section_layout_algorithm.cc),
//! under the notice in LICENSE-CHROMIUM.

use std::rc::Rc;

#[cfg(feature = "fragmentation")]
use crate::layout::{
  box_fragment_builder::BoxFragmentBuilder,
  break_appeal::BreakAppeal,
  fragmentation_utils::{
    FlexItemBreak, IndexedBlockChildIterator, break_before_child_if_needed, finish_fragmentation,
    fragment_index, fragmentation_layout_input_for_child, is_early_break_target,
    setup_fragment_builder_for_fragmentation, setup_space_builder_for_fragmentation,
  },
  logical_rect::LogicalBoxStrut,
};
use crate::{
  geometry::{NodeId, Point},
  layout::{
    constraint_space::{CacheSlot, ConstraintSpace},
    fragmentation_layout_input::{ChildBreakToken, FragmentationLayoutInput},
    layout_result::{LayoutResult, LayoutStatus},
    logical_rect::LogicalSize,
    table::constraint_space_data::{TableConstraintSpaceData, TablePart},
    tree::LayoutTree,
  },
  layout_unit::LayoutUnit,
};

/// Stacks a section's rows at the block sizes its table gave them.
pub(super) struct TableSectionLayoutAlgorithm<'t, 'r> {
  tree: &'t mut LayoutTree<'r>,
  data: &'t Rc<TableConstraintSpaceData>,
  section: usize,
  #[cfg_attr(not(feature = "fragmentation"), allow(dead_code))]
  node: NodeId,
  space: &'t ConstraintSpace,
  #[cfg_attr(not(feature = "fragmentation"), allow(dead_code))]
  input: &'t FragmentationLayoutInput,
  #[cfg(feature = "fragmentation")]
  builder: BoxFragmentBuilder,
}

impl<'t, 'r> TableSectionLayoutAlgorithm<'t, 'r> {
  pub(super) fn new(
    tree: &'t mut LayoutTree<'r>,
    data: &'t Rc<TableConstraintSpaceData>,
    section: usize,
    space: &'t ConstraintSpace,
    input: &'t FragmentationLayoutInput,
  ) -> Self {
    let node = data.structure.sections[section].node;
    #[cfg(feature = "fragmentation")]
    let builder = {
      let style = Rc::clone(tree.layout_style_rc(node));
      let mut builder = BoxFragmentBuilder::new(
        space,
        &style,
        LogicalBoxStrut::default(),
        input.break_token.clone(),
      );

      setup_fragment_builder_for_fragmentation(space, &style, data.inline_size, &mut builder);
      builder
    };

    Self {
      tree,
      data,
      section,
      node,
      space,
      input,
      #[cfg(feature = "fragmentation")]
      builder,
    }
  }

  /// Blink's `Layout`, which lays the rows out only when the layout places them.
  pub(super) fn layout(mut self) -> LayoutResult {
    // A first row whose baseline a percentage block size moves finds it only by laying its cells
    // out, so the measure lays the rows out too, as Blink's always does.
    if self.space.cache_slot != CacheSlot::Layout && self.first_row_baseline_is_known() {
      return self.measure();
    }

    let data = Rc::clone(self.data);
    let section = &data.structure.sections[self.section];
    let start_row = data.rows.sections[self.section].start_row;
    let has_collapsed_borders = data.structure.borders.is_collapsed;
    let mut offset = LayoutUnit::ZERO;
    let mut intrinsic_block_size = LayoutUnit::ZERO;
    let mut is_first_non_collapsed_row = true;
    let mut first_baseline = None;
    let mut row_offsets = vec![LayoutUnit::ZERO];
    let mut actual_start_row = start_row;
    let mut out_of_flow = Vec::new();
    let mut entries = self.row_entries();
    let mut breaks_before_row = false;

    for (index, token) in entries.by_ref() {
      let row = &section.rows[index];
      let row_index = start_row + index;
      let is_row_collapsed = data.rows.rows[row_index].is_collapsed;

      if self.breaks_before_early_break_target(row.node) {
        breaks_before_row = true;
        break;
      }
      if !is_first_non_collapsed_row && !is_row_collapsed {
        offset += data.spacing.1;
      }

      let part = TablePart::Row {
        section: self.section,
        row: index,
      };
      #[cfg_attr(not(feature = "fragmentation"), allow(unused_mut))]
      let mut space = data.space(part, CacheSlot::Layout);

      self.setup_row_space_for_fragmentation(&mut space, offset);

      let result = self.lay_out_row(row.node, &space, &token);

      match self.break_before_row_if_needed(row.node, &result, offset, !is_first_non_collapsed_row)
      {
        Ok(true) => break,
        Ok(false) => {}
        Err(status) => return self.relayout_and_break_earlier(status),
      }
      if first_baseline.is_none() {
        first_baseline = result.first_baseline.map(|baseline| offset + baseline);
      }

      let location = Point::new(LayoutUnit::ZERO, offset);

      self.place_row(row.node, &token, location, &result);
      out_of_flow.extend(result.out_of_flow_moved_by(location));
      offset += result.size.block_size;
      is_first_non_collapsed_row &= is_row_collapsed;
      if has_collapsed_borders && !Self::resumes_past_block_end(&token) {
        if row_offsets.len() == 1 {
          actual_start_row = row_index;
        }
        row_offsets.push(offset);
      }
      intrinsic_block_size = offset;
      if self.has_inflow_child_break_inside() {
        break;
      }
    }
    if !breaks_before_row && entries.len() == 0 {
      self.set_has_seen_all_children();
    }

    let block_size = if section.rows.is_empty() {
      data.rows.sections[self.section].block_size
    } else {
      offset + self.previously_consumed_block_size()
    };
    let block_size = match self.finish_fragmentation(block_size, intrinsic_block_size) {
      Ok(block_size) => block_size,
      Err(status) => return self.relayout_and_break_earlier(status),
    };
    let mut result = LayoutResult::new(LogicalSize::new(data.inline_size, block_size));

    result.first_baseline = first_baseline;
    result.out_of_flow = out_of_flow;
    if row_offsets.len() > 1 {
      result.table_section_row_offsets = Some((actual_start_row, row_offsets));
    }
    self.finish_break_info(&mut result);
    result
  }

  fn first_row_baseline_is_known(&self) -> bool {
    let rows = &self.data.rows;

    rows
      .rows
      .get(rows.sections[self.section].start_row)
      .is_none_or(|row| row.baseline.is_some())
  }

  /// The section's block size and baseline from the rows its table measured, as a measure that
  /// places nothing reads them.
  fn measure(&self) -> LayoutResult {
    let data = self.data;
    let section_rows = &data.rows.sections[self.section];
    let extents = data.row_extents(self.section);
    let block_size = extents
      .last()
      .map_or(section_rows.block_size, |&(_, end)| end);
    let mut result = LayoutResult::new(LogicalSize::new(data.inline_size, block_size));

    result.first_baseline = extents.first().map(|&(start, _)| {
      start
        + data.rows.rows[section_rows.start_row]
          .baseline
          .unwrap_or(LayoutUnit::ZERO)
    });
    result
  }
}

#[cfg(feature = "fragmentation")]
impl TableSectionLayoutAlgorithm<'_, '_> {
  /// Blink's `BlockChildIterator` over the section's rows, by index in the section.
  fn row_entries(&self) -> IndexedBlockChildIterator {
    IndexedBlockChildIterator::new(
      Rc::clone(&self.data.structure.sections[self.section].row_nodes),
      self.input.break_token.as_deref(),
    )
  }

  fn breaks_before_early_break_target(&mut self, row: NodeId) -> bool {
    let Some(early_break) = self.input.early_break.as_deref() else {
      return false;
    };

    if !is_early_break_target(early_break, &self.builder, Some(row)) {
      return false;
    }
    self
      .builder
      .add_break_before_child(Some(row), BreakAppeal::Perfect, false);
    true
  }

  fn setup_row_space_for_fragmentation(&self, space: &mut ConstraintSpace, offset: LayoutUnit) {
    setup_space_builder_for_fragmentation(self.space, &self.builder, false, offset, space);
  }

  fn lay_out_row(
    &mut self,
    row: NodeId,
    space: &ConstraintSpace,
    token: &ChildBreakToken,
  ) -> Rc<LayoutResult> {
    let input =
      fragmentation_layout_input_for_child(row, token.clone(), self.input.early_break.as_deref());

    self.tree.layout_fragment(row, space, &input)
  }

  fn break_before_row_if_needed(
    &mut self,
    row: NodeId,
    result: &LayoutResult,
    offset: LayoutUnit,
    has_container_separation: bool,
  ) -> Result<bool, LayoutStatus> {
    if !self.space.has_block_fragmentation() {
      return Ok(false);
    }

    let style = Rc::clone(self.tree.layout_style_rc(row));
    let fragmentainer_block_offset = self.builder.fragmentainer_offset(true) + offset;

    break_before_child_if_needed(
      self.space,
      &mut self.builder,
      (row, &style),
      result,
      fragmentainer_block_offset,
      has_container_separation,
      FlexItemBreak::default(),
    )
  }

  fn place_row(
    &mut self,
    row: NodeId,
    token: &ChildBreakToken,
    location: Point<LayoutUnit>,
    result: &LayoutResult,
  ) {
    self.tree.place_child(
      row,
      fragment_index(token.as_deref()),
      fragment_index(self.input.break_token.as_deref()),
      location,
      result,
    );
    if !self.space.has_block_fragmentation() {
      return;
    }

    let style = Rc::clone(self.tree.layout_style_rc(row));

    self
      .builder
      .add_result(self.space, &style, result, location.y);
  }

  fn resumes_past_block_end(token: &ChildBreakToken) -> bool {
    token.as_deref().is_some_and(|token| token.is_at_block_end)
  }

  fn has_inflow_child_break_inside(&self) -> bool {
    self.space.has_block_fragmentation() && self.builder.has_inflow_child_break_inside
  }

  fn set_has_seen_all_children(&mut self) {
    self.builder.has_seen_all_children = true;
  }

  fn previously_consumed_block_size(&self) -> LayoutUnit {
    self.builder.previously_consumed_block_size()
  }

  /// Blink's `FinishFragmentation`: this fragment's block size, the section `block_size` tall
  /// across its fragments.
  fn finish_fragmentation(
    &mut self,
    block_size: LayoutUnit,
    intrinsic_block_size: LayoutUnit,
  ) -> Result<LayoutUnit, LayoutStatus> {
    if !self.builder.involved_in_block_fragmentation() {
      return Ok(block_size);
    }

    self.builder.fragments_total_block_size = block_size;
    self.builder.intrinsic_block_size = intrinsic_block_size;
    finish_fragmentation(&mut self.builder)?;
    Ok(self.builder.fragment_block_size)
  }

  fn finish_break_info(&mut self, result: &mut LayoutResult) {
    result.break_info = self.builder.finish_break_info(self.node);
  }

  /// Blink's `RelayoutAndBreakEarlier`: the section laid out again to break at the breakpoint it
  /// found.
  fn relayout_and_break_earlier(self, status: LayoutStatus) -> LayoutResult {
    let input = FragmentationLayoutInput {
      break_token: self.input.break_token.clone(),
      early_break: self.builder.early_break.clone(),
    };

    if status == LayoutStatus::NeedsEarlierBreak && input.early_break.is_some() {
      return TableSectionLayoutAlgorithm::new(
        self.tree,
        self.data,
        self.section,
        self.space,
        &input,
      )
      .layout();
    }

    let mut result = LayoutResult::new(LogicalSize::default());

    result.status = status;
    result.break_info.early_break = self.builder.early_break.clone();
    result
  }
}

/// The fragmentation hooks without the `fragmentation` feature, where nothing breaks.
#[cfg(not(feature = "fragmentation"))]
impl TableSectionLayoutAlgorithm<'_, '_> {
  fn row_entries(&self) -> impl ExactSizeIterator<Item = (usize, ChildBreakToken)> + use<> {
    (0..self.data.structure.sections[self.section].rows.len()).map(|index| (index, ()))
  }

  fn breaks_before_early_break_target(&mut self, _row: NodeId) -> bool {
    false
  }

  fn setup_row_space_for_fragmentation(&self, _space: &mut ConstraintSpace, _offset: LayoutUnit) {}

  fn lay_out_row(
    &mut self,
    row: NodeId,
    space: &ConstraintSpace,
    _token: &ChildBreakToken,
  ) -> Rc<LayoutResult> {
    self.tree.layout(row, space)
  }

  fn break_before_row_if_needed(
    &mut self,
    _row: NodeId,
    _result: &LayoutResult,
    _offset: LayoutUnit,
    _has_container_separation: bool,
  ) -> Result<bool, LayoutStatus> {
    Ok(false)
  }

  fn place_row(
    &mut self,
    row: NodeId,
    _token: &ChildBreakToken,
    location: Point<LayoutUnit>,
    result: &LayoutResult,
  ) {
    self.tree.place_child(row, 0, 0, location, result);
  }

  fn resumes_past_block_end(_token: &ChildBreakToken) -> bool {
    false
  }

  fn has_inflow_child_break_inside(&self) -> bool {
    false
  }

  fn set_has_seen_all_children(&mut self) {}

  fn previously_consumed_block_size(&self) -> LayoutUnit {
    LayoutUnit::ZERO
  }

  fn finish_fragmentation(
    &mut self,
    block_size: LayoutUnit,
    _intrinsic_block_size: LayoutUnit,
  ) -> Result<LayoutUnit, LayoutStatus> {
    Ok(block_size)
  }

  fn finish_break_info(&mut self, _result: &mut LayoutResult) {}

  fn relayout_and_break_earlier(self, status: LayoutStatus) -> LayoutResult {
    let mut result = LayoutResult::new(LogicalSize::default());

    result.status = status;
    result
  }
}
